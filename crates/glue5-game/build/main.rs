//! Turns game.wasm into a native static library:
//!
//! 1. find game.wasm under $GLUE5_GAME_DIR and check it is a supported build
//! 2. translate it to C with the patched wasm2c (`mise run setup`)
//! 3. apply hooks (hooks.rs) and generate the glue C and Rust bindings
//! 4. compile everything through an object cache and archive it

mod codegen;
mod compile;
mod formats;
mod hot;
mod mangle;
mod module;
mod natives;
mod optimise;
mod ordering;
mod page_map;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};

include!("../hooks.rs");

/// Game builds the hooks and host code were written against.
const BUILDS: &[(&str, &str)] = &[("8b0b5899ed", "11ca8d2c04c5e843d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0")];

/// wasm2c output files; ~15 MB of C each, so they compile in parallel.
const OUTPUTS: usize = 64;

const DEFINES: &[(&str, &str)] = &[
    ("WASM_RT_MEMCHECK_BOUNDS_CHECK", "1"),
    // The game is trusted code: no per-access bounds checks, plain accesses to
    // the shared memory, unchecked indirect calls, no stack depth counting.
    ("WASM_RT_NONCONFORMING_SKIP_BOUNDS_CHECKS", "1"),
    ("WASM_RT_NONCONFORMING_PLAIN_SHARED_ACCESS", "1"),
    ("WASM_RT_NONCONFORMING_UNCHECKED_CALL_INDIRECT", "1"),
    ("WASM_RT_NONCONFORMING_UNCHECKED_STACK_EXHAUSTION", "1"),
    // Indirect calls pass the caller's instance: every thread's instance
    // shares one function table (`glue5_instance_new`), ~2.9 MB each saved.
    ("WASM_RT_NONCONFORMING_CALLER_INSTANCE", "1"),
    // The memory never moves: its base is a constant for the generated code
    // (set by `Memory::new`), not two loads after every store and call.
    ("WASM_RT_NONCONFORMING_FIXED_MEMORY_BASE", "1"),
];

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let root = manifest.parent().and_then(Path::parent).context("workspace root")?.to_path_buf();
    let out = PathBuf::from(env::var("OUT_DIR")?);
    // (A relative target directory is cargo's, from where it runs: the
    // workspace's root as the tasks run it; build scripts run in their
    // crate's directory.)
    let tools = env::var_os("CARGO_TARGET_DIR")
        .map(|t| root.join(t))
        .unwrap_or_else(|| root.join("target"))
        .join("glue5-tools");

    for path in ["build", "hooks.rs", "hooks", "csrc"] {
        println!("cargo:rerun-if-changed={}", manifest.join(path).display());
    }
    println!("cargo:rerun-if-env-changed=GLUE5_GAME_DIR");
    println!("cargo:rerun-if-env-changed=GLUE5_ALLOW_UNKNOWN_BUILD");

    // 1. The game.
    let game_dir = PathBuf::from(env::var("GLUE5_GAME_DIR").context("GLUE5_GAME_DIR is not set")?);
    let (build_id, wasm_path) = find_wasm(&game_dir)?;
    println!("cargo:rerun-if-changed={}", wasm_path.display());
    let wasm = fs::read(&wasm_path).with_context(|| format!("reading {}", wasm_path.display()))?;
    let sha = hex(&Sha256::digest(&wasm));
    if !BUILDS.iter().any(|&(id, sum)| id == build_id && sum == sha) {
        let msg = format!(
            "{} is not a supported build (id {build_id}, sha256 {sha}); hooks and host code may not match it",
            wasm_path.display()
        );
        ensure!(
            env::var_os("GLUE5_ALLOW_UNKNOWN_BUILD").is_some(),
            "{msg}. Set GLUE5_ALLOW_UNKNOWN_BUILD=1 to try anyway"
        );
        println!("cargo:warning={msg}");
    }
    println!("cargo:rustc-env=GLUE5_GAME_BUILD={build_id}");

    // 2. wasm2c, once per (game.wasm, wasm2c) pair. Generated sources live outside
    // OUT_DIR so every profile and target shares them and their objects.
    let wasm2c = tools.join("bin").join(format!("wasm2c{}", env::consts::EXE_SUFFIX));
    ensure!(wasm2c.exists(), "{} not found: run `mise run setup` first", wasm2c.display());
    let tool_stamp = fs::read_to_string(tools.join("wasm2c.stamp"))
        .with_context(|| format!("{}: run `mise run setup` first", tools.join("wasm2c.stamp").display()))?;
    println!("cargo:rerun-if-changed={}", wasm2c.display());
    let gen_dir = tools.join("gen").join(&sha[..16]);
    let stamp_path = gen_dir.join("wasm2c.stamp");
    if fs::read_to_string(&stamp_path).ok().as_deref() != Some(tool_stamp.as_str()) {
        let tmp = tools.join("gen").join(format!("{}.tmp{}", &sha[..16], std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;
        let status = Command::new(&wasm2c)
            .args(["--enable-threads", "-n", "game", &format!("--num-outputs={OUTPUTS}"), "-o"])
            .arg(tmp.join("game.c"))
            .arg(&wasm_path)
            .status();
        if !status.as_ref().is_ok_and(|s| s.success()) {
            // Not left behind (a GB or more each time, a full disk likely).
            let _ = fs::remove_dir_all(&tmp);
            bail!("wasm2c failed: {status:?}");
        }
        fs::write(tmp.join("wasm2c.stamp"), &tool_stamp)?;
        if fs::read_to_string(&stamp_path).ok().as_deref() == Some(tool_stamp.as_str()) {
            // Another build put the same there meanwhile (and may be reading it).
            let _ = fs::remove_dir_all(&tmp);
        } else {
            let _ = fs::remove_dir_all(&gen_dir);
            fs::rename(&tmp, &gen_dir)?;
        }
    }

    // 3. Hooks and glue.
    let module = module::parse(&wasm)?;
    let hooks = codegen::resolve_hooks(&module, &HOOKS.concat(), CALLED_ONLY)?;
    let mut sources: Vec<PathBuf> = (0..OUTPUTS).map(|i| gen_dir.join(format!("game_{i}.c"))).collect();
    let mut texts: Vec<Option<String>> = vec![None; sources.len()];
    apply_hooks(&hooks, &sources, &mut texts)?;
    ordering::apply(&sources, &mut texts)?;
    page_map::apply(&sources, &mut texts)?;
    formats::apply(&sources, &mut texts)?;
    let optimise = optimise::Options::from_env(env::var("CARGO_CFG_TARGET_ENV")? == "msvc")?;
    let hot = match optimise.hot_list()? {
        Some(list) => hot::gather(&list, &sources, &mut texts, &gen_dir)?,
        None => Vec::new(),
    };
    // (Not over the shared `.hooked.c` files with functions moved out: a
    // build without them may read those meanwhile.)
    let patched = if hot.is_empty() { "hooked.c" } else { "hot.c" };
    for (source, text) in sources.iter_mut().zip(texts) {
        if let Some(text) = text {
            let patched = source.with_extension(patched);
            write_if_changed(&patched, &text)?;
            *source = patched;
        }
    }
    sources.extend(hot);
    write_if_changed(&gen_dir.join("glue.c"), &codegen::glue_c(&module, &hooks)?)?;
    fs::write(out.join("bindings.rs"), codegen::bindings_rs(&module, &hooks))?;
    // The natives, for the crates that call them (DEP_GLUE5_GAME_NATIVES).
    let natives = out.join("natives.tsv");
    write_if_changed(&natives, &natives::table(&wasm)?)?;
    println!("cargo:natives={}", natives.display());

    // 4. Compile.
    let wabt = tools.join("wabt-src");
    let simde = root.join("third_party/wabt/third_party/simde");
    ensure!(simde.join("simde").exists(), "{} is missing: run `mise run setup`", simde.display());
    let mut build = cc::Build::new();
    build
        .include(&gen_dir)
        .include(manifest.join("csrc"))
        .include(wabt.join("wasm2c"))
        .include(&simde)
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .flag_if_supported("-w")
        .flag_if_supported("-fno-strict-aliasing")
        // wasm2c passes the shared memory to memory.copy/fill helpers typed
        // for unshared ones (the same leading fields): Clang 16+ makes the
        // warning an error, which -w does not silence.
        .flag_if_supported("-Wno-error=incompatible-pointer-types");
    for (k, v) in DEFINES {
        build.define(k, *v);
    }
    // iOS devices default to the Apple A7's ARMv8.0: no LSE atomics (the
    // engine synchronises a lot) and old scheduling. iOS 16.4 runs on A11
    // and later.
    if env::var("TARGET")? == "aarch64-apple-ios" {
        build.flag("-mcpu=apple-a11");
    }
    // Apple TV: the A12 and later (the earlier ones lack the memory).
    if env::var("TARGET")? == "aarch64-apple-tvos" {
        build.flag("-mcpu=apple-a12");
    }
    // The CPU the game's C is tuned for, if any (Android only; the tune is
    // read once, here, so the flag and the objects' key agree).
    let tune = android_tune();
    // Android likewise defaults to ARMv8.0, with atomics as calls that pick
    // LSE at run time. Phones that can run the game have ARMv8.2 cores
    // (Cortex-A55/A75 and later), none of them a Cortex-A53.
    if env::var("TARGET")? == "aarch64-linux-android" {
        build.flag("-march=armv8.2-a").flag("-mno-fix-cortex-a53-835769");
        // The big cores the game runs on: the Exynos 2100's Cortex-A78 (the
        // Galaxy S21+). -mtune only tells the compiler which core it is
        // scheduling for; the code generated is still ARMv8.2 (what -march
        // says), so it runs on any of them. GLUE5_ANDROID_TUNE picks
        // another (cortex-x1) or turns it off (none).
        if let Some(tune) = &tune {
            build.flag(format!("-mtune={tune}"));
        }
    }
    if env::var("CARGO_CFG_TARGET_ARCH")? == "x86_64" {
        build.flag_if_supported("-msse4.1"); // SIMDe maps wasm SIMD much better with SSE4.1
    }
    // Windows: clang-cl, Clang with MSVC's command line, headers and ABI
    // (link.exe links it with the Rust side). cl.exe truncates identifiers
    // past 2047 characters, which the engine's mangled names reach; its C11
    // atomics are experimental. C11 is what turns on wasm2c's threads, and
    // wasm-rt.h includes windows.h in every file: its lean version, without
    // the min/max macros.
    let msvc = env::var("CARGO_CFG_TARGET_ENV")? == "msvc";
    if msvc {
        let target = env::var("TARGET")?;
        if env::var_os("CC").is_none() && env::var_os(format!("CC_{}", target.replace('-', "_"))).is_none() {
            build.compiler("clang-cl");
        }
        build.flag("/std:c17").define("WIN32_LEAN_AND_MEAN", None).define("NOMINMAX", None);
    }
    // Linux: Clang too, as everywhere else (Apple's, the NDK's, clang-cl),
    // rather than the system's cc, usually GCC. `CC` overrides it.
    if env::var("CARGO_CFG_TARGET_OS")? == "linux" {
        let target = env::var("TARGET")?;
        if env::var_os("CC").is_none() && env::var_os(format!("CC_{}", target.replace('-', "_"))).is_none() {
            build.compiler("clang");
        }
    }
    // Three groups, each keyed by the headers it includes (sources are not
    // scanned for includes): the game, the glue, the wasm2c runtime.
    let glue = vec![gen_dir.join("glue.c"), manifest.join("csrc/glue5.c")];
    let runtime: Vec<PathBuf> = ["wasm-rt-impl.c", "wasm-rt-mem-impl.c", "wasm-rt-exceptions-impl.c"]
        .iter()
        .map(|rt| wabt.join("wasm2c").join(rt))
        .collect();
    let key = |files: &[PathBuf]| -> Result<String> {
        let mut h = Sha256::new();
        h.update(tool_stamp.as_bytes());
        for f in files {
            h.update(fs::read(f).with_context(|| format!("reading {}", f.display()))?);
        }
        Ok(hex(&h.finalize()))
    };
    let game_key = key(&[gen_dir.join("game.h"), gen_dir.join("game-impl.h")])?;
    let glue_key = key(&[gen_dir.join("game.h"), manifest.join("csrc/glue5.h")])?;
    let runtime_key = key(&[])?;
    // The tune is part of the compiler's command line, so the objects of a
    // build with another one are not the same: it belongs to their key.
    let tune_key = tune.clone().unwrap_or_default();

    // Cargo's -j, but at most 8: each job holds a 15 MB C file's compilation.
    let jobs = env::var("NUM_JOBS").ok().and_then(|j| j.parse().ok()).unwrap_or(4).min(8);
    let cache = tools.join("objcache").join(env::var("TARGET")?);
    let mut game_build = build.clone();
    optimise.apply(&mut game_build);
    let game_key = format!("{game_key}{}{tune_key}", optimise.key());
    let objects = compile::compile(&game_build, &sources, &game_key, &cache, jobs)?;
    let ltocache = tools.join("ltocache").join(env::var("TARGET")?);
    let mut objects = optimise.link(&game_build, objects, &ltocache, &out)?;
    optimise.runtime(&build)?;
    objects.extend(compile::compile(&build, &glue, &glue_key, &cache, jobs)?);
    objects.extend(compile::compile(&build, &runtime, &runtime_key, &cache, jobs)?);
    let lib = if msvc { "glue5_game.lib" } else { "libglue5_game.a" };
    compile::archive(&build, &objects, &out.join(lib))?;
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=glue5_game");
    Ok(())
}

/// The CPU the game's C is tuned for on Android (`GLUE5_ANDROID_TUNE`):
/// the phone's big cores, the ones the game runs on. `none` (or an empty
/// value) turns the tune off.
fn android_tune() -> Option<String> {
    println!("cargo:rerun-if-env-changed=GLUE5_ANDROID_TUNE");
    let tune = env::var("GLUE5_ANDROID_TUNE").unwrap_or_else(|_| "cortex-a78".to_string());
    let tune = tune.trim();
    if tune.is_empty() || tune == "none" {
        None
    } else {
        Some(tune.to_string())
    }
}

/// The single `b/<build>/game.wasm` under the game directory.
fn find_wasm(game_dir: &Path) -> Result<(String, PathBuf)> {
    let b = game_dir.join("b");
    let mut found = Vec::new();
    for entry in fs::read_dir(&b).with_context(|| {
        format!("{} not found: point GLUE5_GAME_DIR at the game directory (or symlink ./game)", b.display())
    })? {
        let entry = entry?;
        let wasm = entry.path().join("game.wasm");
        if wasm.exists() {
            found.push((entry.file_name().to_string_lossy().into_owned(), wasm));
        }
    }
    match found.len() {
        1 => Ok(found.pop().unwrap()),
        0 => bail!("no b/<build>/game.wasm under {}", game_dir.display()),
        _ => bail!("several builds under {}; keep one", b.display()),
    }
}

/// Renames each hooked function's definition to `<symbol>_orig` in its
/// file's text (`texts`, written as `game_N.hooked.c`, which replaces the
/// original in the build); those only called are left as they are, once
/// found (a definition the glue can call). One pass over each file: its
/// functions' names looked up among the hooks (a search per hook took
/// minutes).
fn apply_hooks(hooks: &[codegen::Hook], sources: &[PathBuf], texts: &mut [Option<String>]) -> Result<()> {
    const PARAMS: &str = "(w2c_game* instance";
    let mut pending: std::collections::HashMap<&str, &codegen::Hook> =
        hooks.iter().map(|h| (h.symbol.as_str(), h)).collect();
    for (source, patched) in sources.iter().zip(texts.iter_mut()) {
        if pending.is_empty() {
            break;
        }
        let text = fs::read_to_string(source)?;
        // Where each hooked definition's name ends (`_orig` goes there).
        let mut renames = Vec::new();
        for (at, _) in text.match_indices(PARAMS) {
            let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
            let Some(space) = text[line_start..at].rfind(' ') else { continue };
            let symbol = &text[line_start + space + 1..at];
            if !pending.contains_key(symbol) {
                continue;
            }
            // A definition, not a declaration: its signature (on one line,
            // or wrapped over several) ends with `{`.
            let rest = &text[line_start..];
            let end = rest.find(['{', ';']).unwrap_or(0);
            if rest[end..].starts_with('{')
                && !rest.starts_with("static")
                && let Some(hook) = pending.remove(symbol)
                && !hook.called_only
            {
                renames.push(at);
            }
        }
        if renames.is_empty() {
            continue;
        }
        let mut out = String::with_capacity(text.len() + renames.len() * 5);
        let mut from = 0;
        for at in renames {
            out.push_str(&text[from..at]);
            out.push_str("_orig");
            from = at;
        }
        out.push_str(&text[from..]);
        *patched = Some(out);
    }
    if let Some(hook) = hooks.iter().find(|h| pending.contains_key(h.symbol.as_str())) {
        bail!("hook `{}`: definition of {} not found in the wasm2c output", hook.field, hook.symbol);
    }
    Ok(())
}

fn write_if_changed(path: &Path, text: &str) -> Result<()> {
    if fs::read_to_string(path).ok().as_deref() != Some(text) {
        fs::write(path, text)?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
