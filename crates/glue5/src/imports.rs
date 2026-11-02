//! Files brought in through Android's document picker.
//!
//! Java owns the picker and copies a selected document tree into
//! `Android/data/<app>/files/imports/ready`.  That is intentional: opening a
//! picker backgrounds the renderer, and Android may recreate the activity.
//! A completed tree survives either outcome.  This module then installs it
//! while the game is stopped, with an atomic file-by-file replacement.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::paths;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Save,
    Resources,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Resources => "resources",
        }
    }
}

/// Ask the Java activity to show Android's folder picker.  SDL exposes the
/// activity's JNI environment on the same thread that draws the app.
#[cfg(target_os = "android")]
pub fn pick(kind: Kind) -> Result<()> {
    ask(kind.name())
}

/// Ask for the folder the game is played from.  It is not an import: nothing
/// is copied, the engine reads the game where the player keeps it (which
/// takes the access the activity asks for first).
#[cfg(target_os = "android")]
pub fn pick_game() -> Result<()> {
    ask("game")
}

#[cfg(target_os = "android")]
fn ask(what: &str) -> Result<()> {
    use jni::objects::{JObject, JValue};

    let started = crate::jni_util::with_activity(|env, class| {
        let value = JObject::from(env.new_string(what).context("encoding import type")?);
        env.call_static_method(
            class,
            "startImport",
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&value)],
        )
        .context("asking Android to open the folder picker")?
        .z()
        .context("reading Android's folder-picker result")
    })?;
    if !started {
        bail!("Android's activity is not ready to open the folder picker");
    }
    Ok(())
}

/// The folder the activity's picker chose for the game, or why it did not.
#[derive(Clone, Debug)]
pub enum GamePick {
    /// A path on the device's storage; whether it has the game is the app's
    /// call, which knows this build's layout.
    Found(PathBuf),
    /// What went wrong, for the start page.
    Refused(String),
}

/// The names the activity writes its answer under, in the imports folder.
const GAME_PATH: &str = "game-path";
const GAME_ERROR: &str = "game-error";

/// Takes the picker's answer for the game's folder, once the activity has
/// left one.  It travels as a file, so an answer already left survives the
/// activity being recreated before the app comes to read it.
pub fn take_game_pick(user_dir: &Path) -> Option<GamePick> {
    let root = paths::imports_dir(user_dir);
    if let Some(path) = take_file(&root.join(GAME_PATH)) {
        // An answer replaces whatever the last try said.
        let _ = fs::remove_file(root.join(GAME_ERROR));
        return Some(GamePick::Found(PathBuf::from(path)));
    }
    take_file(&root.join(GAME_ERROR)).map(GamePick::Refused)
}

/// A small text file's contents, read once: the file goes with them.
fn take_file(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let _ = fs::remove_file(path);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// Install every completed Android import.  A directory moves into
/// `processing` before we touch it, so it is either retried on the next app
/// launch or retained under `failed` with its error.
pub fn install_ready(user_dir: &Path, game_dir: Option<&Path>) -> Vec<Result<String>> {
    let root = paths::imports_dir(user_dir);
    let mut results = Vec::new();
    for state in ["ready", "processing"] {
        let dir = root.join(state);
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let source = entry.path();
            let processing = root.join("processing").join(&name);
            let stage = if state == "ready" {
                match fs::create_dir_all(root.join("processing")).and_then(|()| fs::rename(&source, &processing)) {
                    Ok(()) => processing,
                    Err(error) => {
                        record_event(&root, &format!("Could not stage {name}: {error}"));
                        results.push(Err(error).context("moving the selected import into place"));
                        continue;
                    }
                }
            } else {
                source
            };
            let outcome = install_one(&stage, user_dir, game_dir);
            match outcome {
                Ok(message) => {
                    if let Err(error) = fs::remove_dir_all(&stage) {
                        record_event(&root, &format!("Installed {name}, but could not remove staging files: {error}"));
                        results.push(Err(error).context("removing the completed import staging folder"));
                    } else {
                        record_event(&root, &format!("Installed {name}: {message}"));
                        results.push(Ok(message));
                    }
                }
                Err(error) => {
                    record_event(&root, &format!("Import {name} failed: {error:#}"));
                    let failed = root.join("failed").join(&name);
                    let moved = fs::create_dir_all(root.join("failed")).and_then(|()| fs::rename(&stage, &failed));
                    if moved.is_ok() {
                        let _ = fs::write(failed.join("error.txt"), format!("{error:#}\n"));
                    }
                    results.push(Err(error));
                }
            }
        }
    }
    results.extend(take_copy_errors(&root));
    sync_legacy_saves_to_default(user_dir);
    results
}

/// Java records failures while copying a SAF tree here. Surface each one in
/// the settings note once instead of leaving it in an invisible side folder.
fn take_copy_errors(root: &Path) -> Option<Result<String>> {
    let dir = root.join("errors");
    let entries = fs::read_dir(&dir).ok()?;
    let mut paths = entries.flatten().map(|entry| entry.path()).collect::<Vec<_>>();
    paths.sort();
    let mut errors = Vec::new();
    for path in paths {
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("txt") {
            continue;
        }
        let name = path.file_stem().and_then(|name| name.to_str()).unwrap_or("unknown");
        let what = if name.starts_with("save-") {
            "save folder"
        } else if name.starts_with("resources-") {
            "game resources folder"
        } else {
            "selected folder"
        };
        let message = match fs::read_to_string(&path) {
            Ok(message) if !message.trim().is_empty() => message.trim().replace('\n', " "),
            Ok(_) => "Android returned no error details".to_owned(),
            Err(error) => format!("could not read Android's error report: {error}"),
        };
        let _ = fs::remove_file(&path);
        errors.push(format!("{what}: {message}"));
    }
    if errors.is_empty() {
        None
    } else {
        let message = format!("Android could not copy the selected import: {}", errors.join("; "));
        record_event(root, &message);
        Some(Err(anyhow::anyhow!("{message}")))
    }
}

pub(crate) fn record_game_picker_issue(user_dir: &Path, message: &str) {
    record_event(
        &paths::imports_dir(user_dir),
        &format!("Find files: {message}"),
    );
}

fn record_event(root: &Path, event: &str) {
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let event = event.replace('\n', " ").replace('\r', " ");
    let result = fs::create_dir_all(root).and_then(|()| {
        let mut file = fs::OpenOptions::new().create(true).append(true).open(root.join("events.log"))?;
        writeln!(file, "{at} {event}")
    });
    if let Err(error) = result {
        log::warn!("imports: cannot record diagnostic event: {error}");
    } else {
        log::info!("imports: {event}");
    }
}

/// A bounded support snapshot of import/save probes, runtime settings,
/// virtual gamepad transitions, performance data, engine output, and the GPU
/// census. It never reads save-file contents or records raw touch coordinates.
pub fn diagnostics_report(
    user_dir: &Path,
    source: crate::settings::Source,
    game_dir: Option<&Path>,
    settings: Option<&crate::settings::Settings>,
    touch_events: &[String],
) -> String {
    use std::fmt::Write as _;

    let imports = paths::imports_dir(user_dir);
    let saves = user_dir.join("Documents/Rockstar Games/GTA V");
    let mut report = String::new();
    let _ = writeln!(report, "GLUE 5 Android support diagnostics");
    report.push_str("Bounded runtime snapshot; paths and file names may appear. Review before sharing. No save contents or raw touch coordinates are included.\n");
    let _ = writeln!(report, "App build: {}", crate::version::LABEL);
    let _ = writeln!(report, "Platform: {} / {}", std::env::consts::OS, std::env::consts::ARCH);
    if let Some(adapter) = glue5_gpu::adapter_diagnostics() {
        let _ = writeln!(report, "GPU adapter: {adapter}");
    } else {
        report.push_str("GPU adapter: not initialized\n");
    }
    let _ = writeln!(report, "Runtime failure: {}", glue5_runtime::exit::failed().unwrap_or("none recorded"));
    let _ = writeln!(report, "Engine exit request: {:?}", glue5_runtime::exit::requested());
    if let Some(trap) = glue5_runtime::last_trap_diagnostics() {
        let _ = writeln!(report, "WASM trap detail: {trap}");
    }
    let _ = writeln!(report, "Game source: {source:?}");
    let _ = writeln!(
        report,
        "Game directory: {}",
        game_dir.map_or_else(|| "(not set)".to_owned(), |path| path.display().to_string())
    );
    let _ = writeln!(report, "User directory: {}", user_dir.display());
    let _ = writeln!(report, "Engine VFS mapping: /userdata -> {}", user_dir.display());
    let _ = writeln!(report, "Imports directory: {}", imports.display());
    let _ = writeln!(report, "Save root: {}", saves.display());
    append_settings(&mut report, settings);
    append_touch_diagnostics(&mut report, touch_events);
    append_input_diagnostics(&mut report, &glue5_runtime::input::diagnostics());
    append_perf_diagnostics(&mut report, &glue5_gpu::perf::diagnostics_snapshot());
    let gpu_diagnostics = glue5_gpu::census::diagnostics_snapshot();
    append_gpu_census(&mut report, gpu_diagnostics.as_ref());

    let events = imports.join("events.log");
    let _ = writeln!(report, "\nRecent import events:");
    match fs::read(&events) {
        Ok(bytes) => {
            let start = bytes.len().saturating_sub(32 * 1024);
            report.push_str(&String::from_utf8_lossy(&bytes[start..]));
            if !report.ends_with('\n') {
                report.push('\n');
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.push_str("(no recorded imports)\n");
        }
        Err(error) => {
            let _ = writeln!(report, "(cannot read event log: {error})");
        }
    }

    let mut remaining = 300;
    let _ = writeln!(report, "\nImport staging files (names and sizes only):");
    append_matching_files(&mut report, &imports, &mut remaining, 8, |path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        is_save(name)
            || name == "pc_settings.bin"
            || name == ".glue5-source-name"
            || name == ".glue5-source-document-id"
            || name == "error.txt"
            || (path.parent().and_then(Path::file_name).is_some_and(|n| n == "errors")
                && path.extension().and_then(|e| e.to_str()) == Some("txt"))
    });

    remaining = 300;
    let _ = writeln!(report, "\nInstalled GTA V save files (names and sizes only):");
    append_matching_files(&mut report, &saves, &mut remaining, 8, |path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        is_save(name) || name == "pc_settings.bin"
    });

    let trace = user_dir.join("save-vfs.log");
    let _ = writeln!(report, "\nRecent runtime save-filesystem trace (paths, operations, results only):");
    let _ = writeln!(report, "getdents-list shows relevant directory names; open/stat/fstat/read results show discovery and access. Read entries contain byte counts only.");
    match fs::read(&trace) {
        Ok(bytes) if bytes.is_empty() => {
            report.push_str("(trace file is empty; no matching filesystem operations were observed)\n");
        }
        Ok(bytes) => {
            let start = bytes.len().saturating_sub(64 * 1024);
            report.push_str(&String::from_utf8_lossy(&bytes[start..]));
            if !report.ends_with('\n') {
                report.push('\n');
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.push_str("(no runtime trace recorded; open Load Game, then copy this report)\n");
        }
        Err(error) => {
            let _ = writeln!(report, "(cannot read runtime trace: {error})");
        }
    }
    append_recent_engine_save_log(&mut report, user_dir);
    append_recent_engine_log_tail(&mut report, user_dir);
    report
}

fn append_settings(report: &mut String, settings: Option<&crate::settings::Settings>) {
    use std::fmt::Write as _;

    let _ = writeln!(report, "\nRuntime settings:");
    let Some(settings) = settings else {
        report.push_str("(not available)\n");
        return;
    };
    let _ = writeln!(
        report,
        "Graphics: scale={:?}, upscaler={:?}, sharpness={:.2}, max_fps={}, vsync={}, frame_generation={}",
        settings.render_scale,
        settings.upscaler,
        settings.sharpness,
        settings.max_fps,
        settings.vsync,
        settings.frame_generation,
    );
    let _ = writeln!(
        report,
        "Overlay: {:?}/{:?}; touch: {:?}, layout={:?}, vehicle_steer={:?}, aim_fire={:?}, aim_assist={:?}, camera_speed={:.2}, size={:.2}, opacity={:.0}%, haptics={}, phone_touchscreen={}",
        settings.overlay,
        settings.overlay_detail,
        settings.touch,
        settings.touch_layout,
        settings.touch_steer,
        settings.touch_fire,
        settings.touch_aim,
        settings.touch_camera,
        settings.touch_size,
        settings.touch_opacity * 100.0,
        settings.haptics,
        settings.phone_touchscreen,
    );
    let _ = writeln!(report, "Phone: touchscreen={}, bigger={}", settings.phone_touchscreen, settings.phone_enlarge);
}

fn append_touch_diagnostics(report: &mut String, events: &[String]) {
    use std::fmt::Write as _;

    let _ = writeln!(report, "\nRecent virtual touch/gamepad output (no finger coordinates):");
    if events.is_empty() {
        report.push_str("(no touch gamepad transitions recorded)\n");
        return;
    }
    for event in events {
        let _ = writeln!(report, "{event}");
    }
}

fn append_input_diagnostics(report: &mut String, events: &[String]) {
    use std::fmt::Write as _;

    let _ = writeln!(report, "\nRecent phone/input-device transitions (from the game):");
    if events.is_empty() {
        report.push_str("(no phone or input-device transitions recorded)\n");
        return;
    }
    for event in events {
        let _ = writeln!(report, "{event}");
    }
}

fn append_perf_diagnostics(report: &mut String, snapshot: &glue5_gpu::perf::Snapshot) {
    use std::fmt::Write as _;

    fn distribution(values: &[f32]) -> Option<String> {
        let mut values = values.iter().copied().filter(|value| value.is_finite()).collect::<Vec<_>>();
        if values.is_empty() {
            return None;
        }
        values.sort_by(f32::total_cmp);
        let average = values.iter().sum::<f32>() / values.len() as f32;
        let p95 = values[(values.len() * 95 + 99) / 100 - 1];
        Some(format!("n={} avg={average:.2}ms p95={p95:.2}ms max={:.2}ms", values.len(), values[values.len() - 1]))
    }

    let _ = writeln!(report, "\nPerformance snapshot (last overlay session):");
    let _ = writeln!(report, "Frame interval: {}", distribution(&snapshot.frame_ms).unwrap_or_else(|| "not sampled".to_owned()));
    let _ = writeln!(report, "Renderer time: {}", distribution(&snapshot.render_ms).unwrap_or_else(|| "not sampled".to_owned()));
    let _ = writeln!(report, "GPU time: {}", distribution(&snapshot.gpu_ms).unwrap_or_else(|| "not available on this backend".to_owned()));
    let _ = writeln!(
        report,
        "Last renderer counts: draws={} passes={} submits={}; tracked GPU memory={:.1} MiB",
        snapshot.draws,
        snapshot.passes,
        snapshot.submits,
        snapshot.gpu_bytes as f64 / (1024.0 * 1024.0),
    );
}

/// Adds the renderer's bounded HUD census to the same clipboard report as
/// import/save diagnostics. The census contains render metadata only, not
/// game or save-file contents.
fn append_gpu_census(report: &mut String, diagnostics: Option<&glue5_gpu::census::DiagnosticsSnapshot>) {
    use std::fmt::Write as _;

    const DETAILS: usize = 12;
    let _ = writeln!(report, "\nGPU HUD census (Full performance overlay):");
    let Some(diagnostics) = diagnostics else {
        report.push_str("(no Full-overlay session captured; enable Glue 5 overlay at Full detail while playing)\n");
        return;
    };
    let census = &diagnostics.summary;
    if census.frames == 0 {
        report.push_str("(no rendered frames captured)\n");
        return;
    }

    let _ = writeln!(report, "Frames: {} total; {} with SCENE_DONE", census.frames, census.with_hud);
    let _ = writeln!(
        report,
        "HUD draws/frame: min {} / mean {:.1} / max {}; target-set changes/frame: mean {:.1} / max {}; BB target sets/frame: min {} / mean {:.1} / max {}",
        census.hud_draws_min,
        census.hud_draws_mean,
        census.hud_draws_max,
        census.target_set_changes_mean,
        census.target_set_changes_max,
        census.bb_passes_min,
        census.bb_passes_mean,
        census.bb_passes_max,
    );
    let _ = writeln!(
        report,
        "Frames reading BB through SRV: {}; frames with depth-stencil: {}",
        census.frames_reading_bb, census.frames_with_ds
    );
    let sizes = census
        .sizes
        .iter()
        .map(|(width, height)| format!("{width}x{height}"))
        .collect::<Vec<_>>();
    let _ = writeln!(report, "HUD target sizes: {}", if sizes.is_empty() { "none".to_owned() } else { sizes.join(", ") });
    let _ = writeln!(
        report,
        "Pixel programs reading SV_Position: {}",
        if census.reads_position.is_empty() { "none".to_owned() } else { census.reads_position.join(", ") }
    );

    let _ = writeln!(report, "BROKE scene-sized HUD targets: {}", census.scene_sized_targets.len());
    for issue in census.scene_sized_targets.iter().take(DETAILS) {
        let _ = writeln!(
            report,
            "  RT#{} {}x{} F{} mip{} slice{} samples{}; frames {}/{}; draws {}; first..last {}..{}; pre-SCENE_DONE {}/{} frames; BB-reading HUD draws {}",
            issue.id,
            issue.width,
            issue.height,
            issue.format,
            issue.mip,
            issue.slice,
            issue.samples,
            issue.frames,
            census.with_hud,
            issue.draws,
            issue.first_draw,
            issue.last_draw,
            issue.written_before_scene_done_frames,
            census.with_hud,
            issue.reads_into_bb_draws,
        );
        let programs = issue.pixel_programs.iter().take(DETAILS).cloned().collect::<Vec<_>>();
        let _ = writeln!(report, "    pixel programs: {}", if programs.is_empty() { "none".to_owned() } else { programs.join(", ") });
        if issue.pixel_programs.len() > DETAILS {
            let _ = writeln!(report, "    ({} more pixel programs omitted)", issue.pixel_programs.len() - DETAILS);
        }
    }
    if census.scene_sized_targets.len() > DETAILS {
        let _ = writeln!(report, "  ({} more scene-sized targets omitted)", census.scene_sized_targets.len() - DETAILS);
    }

    let _ = writeln!(report, "Scene targets sampled by BB-writing HUD draws: {}", census.scene_targets_read_by_hud.len());
    for source in census.scene_targets_read_by_hud.iter().take(DETAILS) {
        let _ = writeln!(
            report,
            "  SRC RT#{} {}x{} F{} mip{} slice{} samples{}; frames {}; HUD draws {}; via {}",
            source.id,
            source.width,
            source.height,
            source.format,
            source.mip,
            source.slice,
            source.samples,
            source.frames,
            source.bb_read_draws,
            if source.bb_reader_programs.is_empty() { "unknown".to_owned() } else { source.bb_reader_programs.iter().take(DETAILS).cloned().collect::<Vec<_>>().join(", ") },
        );
    }
    if census.scene_targets_read_by_hud.len() > DETAILS {
        let _ = writeln!(report, "  ({} more source targets omitted)", census.scene_targets_read_by_hud.len() - DETAILS);
    }

    let _ = writeln!(report, "Other census violations: {}", census.violations.len());
    for (violation, frames) in census.violations.iter().take(DETAILS) {
        let _ = writeln!(report, "  {frames} frames: {}", violation.replace('\n', " ").replace('\r', " "));
    }
    if census.violations.len() > DETAILS {
        let _ = writeln!(report, "  ({} more violations omitted)", census.violations.len() - DETAILS);
    }
    append_frame_samples(report, "Recent violating frame records (JSON)", &diagnostics.incident_frames, 48 * 1024);
    append_frame_samples(report, "Recent frame records (JSON)", &diagnostics.recent_frames, 24 * 1024);
}

fn append_frame_samples(report: &mut String, title: &str, frames: &[String], max_bytes: usize) {
    use std::fmt::Write as _;

    let _ = writeln!(report, "\n{title}: {} retained", frames.len());
    if frames.is_empty() {
        report.push_str("(none)\n");
        return;
    }
    let selected = frames.iter().rev().take(8).collect::<Vec<_>>();
    let mut output = 0;
    for line in selected.into_iter().rev() {
        let needed = line.len() + 1;
        if output + needed > max_bytes {
            report.push_str("(frame JSON excerpt capped)\n");
            break;
        }
        report.push_str(line);
        report.push('\n');
        output += needed;
    }
}

fn append_recent_engine_save_log(report: &mut String, user_dir: &Path) {
    use std::fmt::Write as _;
    use std::io::{Read, Seek, SeekFrom};

    const WINDOW: u64 = 256 * 1024;
    const MAX_OUTPUT: usize = 16 * 1024;
    let path = user_dir.join("game.log");
    let _ = writeln!(report, "\nRelevant engine log lines (filtered; log text only, not save contents):");
    let result = (|| {
        let mut file = fs::File::open(path)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(WINDOW)))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok::<_, std::io::Error>(bytes)
    })();
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.push_str("(engine log not present)\n");
            return;
        }
        Err(error) => {
            let _ = writeln!(report, "(cannot read engine log: {error})");
            return;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("save")
                || lower.contains("profile")
                || lower.contains("sgta5")
                || lower.contains("rockstar games")
                || lower.contains("userdata")
        })
        .rev()
        .take(80)
        .collect::<Vec<_>>();
    lines.reverse();
    let mut written = 0;
    for line in lines {
        let needed = line.len() + 1;
        if written + needed > MAX_OUTPUT {
            report.push_str("(engine log excerpt capped)\n");
            break;
        }
        report.push_str(line);
        report.push('\n');
        written += needed;
    }
    if written == 0 {
        report.push_str("(no matching lines in the recent engine log window)\n");
    }
}

/// A bounded unfiltered tail complements the focused save trace above; it can
/// include game-generated names and paths, so the player can inspect the copy
/// before sharing it.
fn append_recent_engine_log_tail(report: &mut String, user_dir: &Path) {
    use std::fmt::Write as _;
    use std::io::{Read, Seek, SeekFrom};

    const WINDOW: u64 = 512 * 1024;
    const MAX_OUTPUT: usize = 32 * 1024;
    const MAX_LINES: usize = 400;
    let path = user_dir.join("game.log");
    let _ = writeln!(report, "\nRecent engine output (last {MAX_LINES} lines, capped at 32 KiB; inspect paths/names before sharing):");
    let result = (|| {
        let mut file = fs::File::open(path)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(WINDOW)))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok::<_, std::io::Error>(bytes)
    })();
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.push_str("(engine log not present)\n");
            return;
        }
        Err(error) => {
            let _ = writeln!(report, "(cannot read engine log: {error})");
            return;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines().rev().take(MAX_LINES).collect::<Vec<_>>();
    lines.reverse();
    let mut written = 0;
    for line in lines {
        let needed = line.len() + 1;
        if written + needed > MAX_OUTPUT {
            report.push_str("(engine output tail capped)\n");
            break;
        }
        report.push_str(line);
        report.push('\n');
        written += needed;
    }
    if written == 0 {
        report.push_str("(engine log is empty)\n");
    }
}

fn append_matching_files(
    report: &mut String,
    root: &Path,
    remaining: &mut usize,
    depth: usize,
    include: impl Fn(&Path) -> bool + Copy,
) {
    use std::fmt::Write as _;

    fn walk(
        report: &mut String,
        root: &Path,
        dir: &Path,
        remaining: &mut usize,
        visited: &mut usize,
        depth: usize,
        include: impl Fn(&Path) -> bool + Copy,
    ) {
        if *remaining == 0 || *visited >= 10_000 {
            return;
        }
        let mut entries = match fs::read_dir(dir) {
            Ok(entries) => entries.flatten().collect::<Vec<_>>(),
            Err(error) => {
                let _ = writeln!(report, "{}: cannot list: {error}", dir.display());
                return;
            }
        };
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if *remaining == 0 || *visited >= 10_000 {
                return;
            }
            *visited += 1;
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() && depth > 0 {
                walk(report, root, &path, remaining, visited, depth - 1, include);
            } else if kind.is_file() && include(&path) {
                let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
                let relative = path.strip_prefix(root).unwrap_or(&path).display();
                let _ = writeln!(report, "{relative} ({size} bytes)");
                let file_name = entry.file_name();
                if file_name == ".glue5-source-name" {
                    if let Ok(name) = fs::read_to_string(&path) {
                        let _ = writeln!(report, "  selected tree name: {}", name.trim());
                    }
                } else if file_name == ".glue5-source-document-id" {
                    if let Ok(id) = fs::read_to_string(&path) {
                        let _ = writeln!(report, "  SAF document id: {}", id.trim());
                    }
                } else if file_name == "error.txt"
                    || (path.parent().and_then(Path::file_name).is_some_and(|n| n == "errors")
                        && path.extension().and_then(|e| e.to_str()) == Some("txt"))
                {
                    if let Ok(message) = fs::read_to_string(&path) {
                        let _ = writeln!(report, "  error: {}", message.trim().replace('\n', " "));
                    }
                }
                *remaining -= 1;
            }
        }
    }

    if !root.is_dir() {
        let _ = writeln!(report, "{} (missing)", root.display());
        return;
    }
    let mut visited = 0;
    walk(report, root, root, remaining, &mut visited, depth, include);
    if *remaining == 0 || visited >= 10_000 {
        report.push_str("(listing capped)\n");
    }
}

#[cfg(target_os = "android")]
pub fn copy_diagnostics(
    user_dir: &Path,
    source: crate::settings::Source,
    game_dir: Option<&Path>,
    settings: &crate::settings::Settings,
    touch_events: &[String],
) -> Result<()> {
    use jni::objects::{JObject, JValue};

    glue5_runtime::flush_save_diagnostics();
    let report = diagnostics_report(user_dir, source, game_dir, Some(settings), touch_events);
    crate::jni_util::with_activity(|env, class| {
        let report = JObject::from(env.new_string(report).context("encoding support report")?);
        env.call_static_method(
            class,
            "copyImportDiagnostics",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&report)],
        )
        .context("asking Android to copy the import report")?;
        Ok(())
    })
}

#[cfg(target_os = "android")]
pub fn request_logcat_capture() -> Result<()> {
    call_logcat_activity("requestLogcatCapture", "asking Android for previous-session logcat capture")
}

#[cfg(target_os = "android")]
pub fn start_logcat_capture() -> Result<()> {
    call_logcat_activity("startLogcatCapture", "asking Android to start live logcat capture")
}

#[cfg(target_os = "android")]
pub fn stop_logcat_capture() -> Result<()> {
    call_logcat_activity("stopLogcatCapture", "asking Android to finish live logcat capture")
}

#[cfg(target_os = "android")]
pub fn mark_debug_event(tag: &'static str) -> Result<()> {
    use jni::objects::{JObject, JValue};

    crate::jni_util::with_activity(|env, class| {
        let marker = JObject::from(env.new_string(tag).context("encoding debug marker")?);
        env.call_static_method(
            class,
            "markDebugEvent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&marker)],
        )
        .context("writing timestamped debug marker to Android log")?;
        Ok(())
    })
}

#[cfg(target_os = "android")]
fn call_logcat_activity(method: &str, context: &'static str) -> Result<()> {
    crate::jni_util::with_activity(|env, class| {
        env.call_static_method(class, method, "()V", &[]).context(context)?;
        Ok(())
    })
}

fn install_one(stage: &Path, user_dir: &Path, game_dir: Option<&Path>) -> Result<String> {
    let label = stage.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    if label.starts_with("save-") {
        let source = best_profile(stage)?.ok_or_else(|| anyhow::anyhow!("the selected folder has no SGTA5 save file"))?;
        let name = profile_name(&source, stage)?;
        let root = user_dir.join("Documents/Rockstar Games/GTA V");
        // Next to the saves the engine wrote itself is the only destination
        // that certainly matches this build's layout (PC builds use
        // `Profiles/<id>`, console-derived ones do not).  Before the first
        // save exists, the PC layout is the best guess.
        let destination = existing_save_dir(&root).unwrap_or_else(|| root.join("Profiles").join(&name));
        let backup = paths::imports_dir(user_dir).join("backups").join(label).join(&name);
        let files = copy_tree(&source, &destination, Some(&backup))?;
        let default_dir = root.join("Default");
        if destination != default_dir {
            let _ = copy_tree(&source, &default_dir, None);
        }
        let shown = destination.strip_prefix(&root).unwrap_or(&destination).display();
        return Ok(format!(
            "Imported {files} save file(s) from profile {name} into \
             Documents/Rockstar Games/GTA V/{shown}. Restart Story Mode before loading."
        ));
    }
    if label.starts_with("resources-") {
        let game = game_dir.ok_or_else(|| anyhow::anyhow!("the local game folder is not available"))?;
        let files = copy_tree(stage, game, None)?;
        return Ok(format!("Imported {files} game resource file(s). Restart the game to load them."));
    }
    bail!("unknown import kind in {label:?}")
}

fn source_name(stage: &Path) -> Result<String> {
    let raw = fs::read_to_string(stage.join(".glue5-source-name")).unwrap_or_default();
    let name = raw.trim();
    if !name.is_empty() && name != "imported-profile" && valid_profile_name(name) {
        Ok(name.to_owned())
    } else {
        bail!("could not determine the selected GTA V profile folder name")
    }
}

/// The folder of the profile in a picked tree.  A profile can be picked as
/// is, inside a "GTA V" folder, or beside a backup copy; the folder with the
/// most save slots is the live one (a backup usually has a single slot).
fn best_profile(dir: &Path) -> Result<Option<PathBuf>> {
    best_profile_except(dir, None)
}

fn best_profile_except(dir: &Path, excluded_name: Option<&str>) -> Result<Option<PathBuf>> {
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn walk(dir: &Path, seen: &mut HashMap<PathBuf, (usize, SystemTime)>) -> Result<()> {
        for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                walk(&entry.path(), seen)?;
            } else if ty.is_file() && is_save(&entry.file_name().to_string_lossy()) {
                let Some(parent) = entry.path().parent().map(Path::to_path_buf) else { continue };
                let time = entry.metadata().and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH);
                let found = seen.entry(parent).or_insert((0, UNIX_EPOCH));
                found.0 += 1;
                found.1 = found.1.max(time);
            }
        }
        Ok(())
    }

    let mut seen = HashMap::new();
    walk(dir, &mut seen)?;
    Ok(seen
        .into_iter()
        .filter(|(path, _)| excluded_name != path.file_name().and_then(|name| name.to_str()))
        .max_by_key(|(_, found)| *found)
        .map(|(path, _)| path))
}

/// `SGTA50000` … `SGTA50015`, with or without the backup suffix.
fn is_save(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("SGTA5") else { return false };
    let rest = rest.strip_suffix(".bak").unwrap_or(rest);
    rest.len() == 4 && rest.chars().all(|c| c.is_ascii_digit())
}

/// Prefer the folder containing existing saves; before the first save, use
/// the engine's empty `Default` directory when it has created one.
fn existing_save_dir(root: &Path) -> Option<PathBuf> {
    best_profile_except(root, Some("imported-profile")).ok().flatten().or_else(|| {
        let default = root.join("Default");
        default.is_dir().then_some(default)
    })
}

/// If the engine's `Default` save directory is missing or empty, but an imported profile
/// exists under `Profiles/`, copy those saves into `Default` so the game's load menu finds them.
pub fn sync_legacy_saves_to_default(user_dir: &Path) {
    let root = user_dir.join("Documents/Rockstar Games/GTA V");
    let default = root.join("Default");
    let has_default_saves = fs::read_dir(&default)
        .ok()
        .map(|entries| entries.flatten().any(|e| is_save(&e.file_name().to_string_lossy())))
        .unwrap_or(false);
    if !has_default_saves {
        if let Ok(Some(existing)) = best_profile_except(&root, Some("imported-profile")) {
            if existing != default {
                let _ = copy_tree(&existing, &default, None);
            }
        }
    }
}

fn profile_name(source: &Path, stage: &Path) -> Result<String> {
    let name = if source == stage {
        source_name(stage)?
    } else {
        source.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_owned()
    };
    if valid_profile_name(&name) {
        Ok(name)
    } else {
        bail!("could not determine the selected GTA V profile folder name")
    }
}

fn valid_profile_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Copy only ordinary directory entries and files.  The source is already an
/// app-owned staging tree, but retaining this restriction keeps an import
/// from manufacturing links outside either destination.
fn copy_tree(from: &Path, to: &Path, backup: Option<&Path>) -> Result<usize> {
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    let mut copied = 0;
    for entry in fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".glue5-source-name" || name == ".glue5-source-document-id" || name == "error.txt" {
            continue;
        }
        let source = entry.path();
        let destination = to.join(&name);
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copied += copy_tree(&source, &destination, backup.map(|b| b.join(&name)).as_deref())?;
        } else if ty.is_file() {
            if let Some(backup) = backup.filter(|_| destination.exists()) {
                if let Some(parent) = backup.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&destination, backup).with_context(|| format!("backing up {}", destination.display()))?;
            }
            copy_file(&source, &destination)?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn copy_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    let extension = to.extension().and_then(|e| e.to_str()).unwrap_or("file");
    let temp = to.with_extension(format!("{extension}.glue5-import"));
    fs::copy(from, &temp).with_context(|| format!("copying {}", from.display()))?;
    if let Err(error) = fs::rename(&temp, to) {
        // Android/Linux replace directly.  The fallback covers filesystems
        // that do not replace an existing file during rename.
        if to.exists() {
            fs::remove_file(to)?;
            fs::rename(&temp, to)?;
        } else {
            return Err(error).with_context(|| format!("installing {}", to.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new() -> Self {
            let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("glue5-import-test-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn imports_profile_selected_directly_with_real_save_names() {
        let temp = TestDir::new();
        let user = &temp.0;
        let stage = paths::imports_dir(user).join("ready/save-fixture");
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join(".glue5-source-name"), "0F74F4C4").unwrap();

        let mut expected_sizes: Vec<(String, u64)> = Vec::new();
        for slot in (0..=12).chain(std::iter::once(15)) {
            let name = format!("SGTA5{slot:04}");
            let backup_name = format!("{name}.bak");
            for (filename, contents) in [(&name, format!("slot-{slot}")), (&backup_name, format!("backup-{slot}"))] {
                let path = stage.join(filename);
                fs::write(&path, contents).unwrap();
                expected_sizes.push((filename.to_string(), fs::metadata(path).unwrap().len()));
            }
        }
        let settings = stage.join("pc_settings.bin");
        fs::write(&settings, b"settings").unwrap();
        let settings_size = fs::metadata(&settings).unwrap().len();

        assert_eq!(best_profile(&stage).unwrap().as_deref(), Some(stage.as_path()));
        let results = install_ready(user, None);
        assert_eq!(results.len(), 1);
        let result = results.into_iter().next().unwrap().unwrap();
        let destination = user.join("Documents/Rockstar Games/GTA V/Profiles/0F74F4C4");
        assert!(result.contains("Imported 29 save file(s)"), "{result}");
        assert!(result.contains("Profiles/0F74F4C4"), "{result}");
        for (filename, expected_size) in expected_sizes {
            let imported = destination.join(&filename);
            assert!(imported.is_file(), "missing {filename}");
            assert_eq!(fs::metadata(imported).unwrap().len(), expected_size);
        }
        let imported_settings = destination.join("pc_settings.bin");
        assert!(imported_settings.is_file());
        assert_eq!(fs::metadata(imported_settings).unwrap().len(), settings_size);
    }

    #[test]
    fn ignores_legacy_placeholder_profile_when_importing_known_profile() {
        let temp = TestDir::new();
        let user = &temp.0;
        let stage = paths::imports_dir(user).join("processing/save-fixture");
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join(".glue5-source-name"), "0F74F4C4").unwrap();
        fs::write(stage.join("SGTA50000"), b"correct save").unwrap();

        let placeholder = user.join("Documents/Rockstar Games/GTA V/Profiles/imported-profile");
        fs::create_dir_all(&placeholder).unwrap();
        fs::write(placeholder.join("SGTA50000"), b"previously misfiled").unwrap();
        let placeholder_save = placeholder.join("SGTA50000");
        let placeholder_size = fs::metadata(&placeholder_save).unwrap().len();
        let staged_size = fs::metadata(stage.join("SGTA50000")).unwrap().len();
        let result = install_one(&stage, user, None).unwrap();

        assert!(result.contains("Profiles/0F74F4C4"), "{result}");
        assert_eq!(fs::metadata(placeholder_save).unwrap().len(), placeholder_size);
        let imported_save = user.join("Documents/Rockstar Games/GTA V/Profiles/0F74F4C4/SGTA50000");
        assert!(imported_save.is_file());
        assert_eq!(fs::metadata(imported_save).unwrap().len(), staged_size);
    }

    #[test]
    fn refuses_to_misfile_direct_profile_when_its_name_is_unavailable() {
        let temp = TestDir::new();
        let user = &temp.0;
        let stage = paths::imports_dir(user).join("processing/save-fixture");
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join(".glue5-source-name"), "imported-profile").unwrap();
        fs::write(stage.join("SGTA50000"), b"save").unwrap();

        let error = install_one(&stage, user, None).unwrap_err();
        assert!(error.to_string().contains("could not determine the selected GTA V profile folder name"));
        assert!(!user.join("Documents/Rockstar Games/GTA V/Profiles/imported-profile/SGTA50000").exists());
    }

    #[test]
    fn imports_into_empty_default_directory_when_it_is_the_runtime_destination() {
        let temp = TestDir::new();
        let user = &temp.0;
        let stage = paths::imports_dir(user).join("processing/save-fixture");
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join(".glue5-source-name"), "0F74F4C4").unwrap();
        let staged_save = stage.join("SGTA50000");
        fs::write(&staged_save, b"synthetic-save-fixture").unwrap();

        let root = user.join("Documents/Rockstar Games/GTA V");
        let default = root.join("Default");
        fs::create_dir_all(&default).unwrap();
        assert!(fs::read_dir(&default).unwrap().next().is_none());

        let result = install_one(&stage, user, None).unwrap();
        let imported_save = default.join("SGTA50000");
        assert!(result.contains("GTA V/Default"), "{result}");
        assert!(imported_save.is_file());
        assert_eq!(fs::metadata(&imported_save).unwrap().len(), fs::metadata(&staged_save).unwrap().len());
        assert!(!root.join("Profiles/0F74F4C4/SGTA50000").exists());
    }

    #[test]
    fn imports_into_the_profile_the_game_already_uses() {
        let temp = TestDir::new();
        let user = &temp.0;
        let stage = paths::imports_dir(user).join("processing/save-fixture");
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join(".glue5-source-name"), "0F74F4C4").unwrap();
        fs::write(stage.join("SGTA50000"), b"import").unwrap();

        let root = user.join("Documents/Rockstar Games/GTA V");
        fs::create_dir_all(root.join("Default")).unwrap();
        let placeholder = root.join("Profiles/imported-profile");
        fs::create_dir_all(&placeholder).unwrap();
        for slot in 0..12 {
            fs::write(placeholder.join(format!("SGTA5{slot:04}")), b"misfiled").unwrap();
        }
        let game_profile = root.join("runtime-profile");
        fs::create_dir_all(&game_profile).unwrap();
        fs::write(game_profile.join("SGTA50000"), b"existing").unwrap();
        let result = install_one(&stage, user, None).unwrap();

        assert!(result.contains("GTA V/runtime-profile"), "{result}");
        let imported = game_profile.join("SGTA50000");
        assert!(imported.is_file());
        assert_eq!(fs::metadata(&imported).unwrap().len(), fs::metadata(stage.join("SGTA50000")).unwrap().len());
        assert!(!user.join("Documents/Rockstar Games/GTA V/Profiles/0F74F4C4/SGTA50000").exists());
    }

    #[test]
    fn android_copy_errors_are_reported_once() {
        let temp = TestDir::new();
        let errors = paths::imports_dir(&temp.0).join("errors");
        fs::create_dir_all(&errors).unwrap();
        fs::write(errors.join("save-123.txt"), "IOException: cannot open selected file").unwrap();

        let results = install_ready(&temp.0, None);
        assert_eq!(results.len(), 1);
        assert!(results[0].as_ref().unwrap_err().to_string().contains("cannot open selected file"));
        assert!(!errors.join("save-123.txt").exists());
        assert!(
            diagnostics_report(&temp.0, crate::settings::Source::Server, None, None, &[])
                .contains("cannot open selected file")
        );
        assert!(install_ready(&temp.0, None).is_empty());
    }

    #[test]
    fn gpu_census_is_formatted_for_the_support_report() {
        use glue5_gpu::census::{DiagnosticsSnapshot, SceneSizedTargetIssue, SceneTargetReadSummary, Summary};

        let census = Summary {
            frames: 5927,
            with_hud: 5927,
            hud_draws_min: 0,
            hud_draws_max: 182,
            hud_draws_mean: 74.0,
            target_set_changes_max: 7,
            target_set_changes_mean: 1.8,
            bb_passes_min: 1,
            bb_passes_max: 2,
            bb_passes_mean: 1.6,
            frames_reading_bb: 4233,
            scene_sized_targets: vec![SceneSizedTargetIssue {
                id: 1212,
                width: 960,
                height: 432,
                format: 87,
                mip: 0,
                slice: 0,
                samples: 1,
                frames: 5927,
                draws: 256599,
                first_draw: 1,
                last_draw: 145,
                written_before_scene_done_frames: 0,
                reads_into_bb_draws: 0,
                pixel_program_ids: vec![1],
                pixel_programs: vec!["cpv_only::P...xturedUnlit".to_owned()],
            }],
            scene_targets_read_by_hud: vec![SceneTargetReadSummary {
                id: 1195,
                width: 960,
                height: 432,
                format: 87,
                mip: 0,
                slice: 0,
                samples: 1,
                frames: 5927,
                bb_read_draws: 50,
                bb_reader_program_ids: vec![2],
                bb_reader_programs: vec!["im::PS_Blit".to_owned()],
            }],
            ..Summary::default()
        };
        let diagnostics = DiagnosticsSnapshot {
            summary: census,
            recent_frames: vec!["{\"frame\":5927,\"draws_hud\":74}".to_owned()],
            incident_frames: vec!["{\"frame\":5927,\"violations\":[\"scene-sized target\"]}".to_owned()],
        };
        let mut report = String::new();
        append_gpu_census(&mut report, Some(&diagnostics));

        assert!(report.contains("GPU HUD census (Full performance overlay)"));
        assert!(report.contains("RT#1212 960x432 F87"));
        assert!(report.contains("pre-SCENE_DONE 0/5927 frames"));
        assert!(report.contains("SRC RT#1195 960x432 F87"));
        assert!(report.contains("via im::PS_Blit"));
        assert!(report.contains("Recent violating frame records (JSON)"));
        assert!(report.contains("\"frame\":5927,\"violations\""));
        assert!(report.contains("Recent frame records (JSON)"));
    }

    #[test]
    fn support_report_captures_settings_touch_output_and_performance() {
        let settings = crate::settings::Settings::default();
        let mut report = String::new();
        append_settings(&mut report, Some(&settings));
        append_touch_diagnostics(&mut report, &["pad buttons=[L2,R2] triggers=(1.00,1.00)".to_owned()]);
        append_perf_diagnostics(
            &mut report,
            &glue5_gpu::perf::Snapshot {
                frame_ms: vec![10.0, 20.0, 30.0],
                render_ms: vec![2.0, 4.0],
                gpu_ms: vec![3.0, 6.0],
                draws: 100,
                passes: 20,
                submits: 4,
                gpu_bytes: 1024 * 1024,
            },
        );

        assert!(report.contains("vehicle_steer=Stick"));
        assert!(report.contains("aim_fire=Auto"));
        assert!(report.contains("pad buttons=[L2,R2]"));
        assert!(report.contains("p95=30.00ms"));
        assert!(report.contains("tracked GPU memory=1.0 MiB"));
    }

    #[test]
    fn missing_gpu_census_explains_how_to_capture_one() {
        let mut report = String::new();
        append_gpu_census(&mut report, None);
        assert!(report.contains("enable Glue 5 overlay at Full detail while playing"));
    }

    #[test]
    fn diagnostics_include_save_paths_and_events_but_not_save_contents() {
        let temp = TestDir::new();
        let user = &temp.0;
        let profile = user.join("Documents/Rockstar Games/GTA V/Profiles/0F74F4C4");
        fs::create_dir_all(&profile).unwrap();
        let staged = paths::imports_dir(user).join("failed/save-previous");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join(".glue5-source-name"), "0F74F4C4").unwrap();
        fs::write(profile.join("SGTA50000"), b"private save data").unwrap();
        record_event(&paths::imports_dir(user), "Installed save-test into Profiles/0F74F4C4");
        fs::write(
            user.join("save-vfs.log"),
            "save-vfs op=getdents-list path=/userdata/Documents/Rockstar Games/GTA V/Profiles result=entries:1 relevant_names=[0F74F4C4]\n",
        )
        .unwrap();
        fs::write(
            user.join("game.log"),
            "Loading profile SGTA50000 failed validation\nUnrelated renderer detail\n",
        )
        .unwrap();

        let game = user.join("Downloads/game");
        let report = diagnostics_report(user, crate::settings::Source::Folder, Some(&game), None, &[]);
        assert!(report.contains(&format!("App build: {}", crate::version::LABEL)));
        assert!(report.contains("Game source: Folder"));
        assert!(report.contains(&format!("Game directory: {}", game.display())));
        assert!(report.contains(&format!("Engine VFS mapping: /userdata -> {}", user.display())));
        assert!(report.contains("Profiles/0F74F4C4/SGTA50000"));
        assert!(report.contains("selected tree name: 0F74F4C4"));
        assert!(report.contains("Installed save-test into Profiles/0F74F4C4"));
        assert!(report.contains("save-vfs op=getdents-list"));
        assert!(report.contains("relevant_names=[0F74F4C4]"));
        assert!(report.contains("Loading profile SGTA50000 failed validation"));
        assert!(report.contains("Recent engine output"));
        assert!(report.contains("No save contents or raw touch coordinates are included"));
        assert!(report.contains("Unrelated renderer detail"));
        assert!(!report.contains("private save data"));
    }

    #[test]
    fn syncs_legacy_profile_saves_to_default_when_default_is_empty() {
        let temp = TestDir::new();
        let user = &temp.0;
        let root = user.join("Documents/Rockstar Games/GTA V");
        let profile = root.join("Profiles/0F74F4C4");
        fs::create_dir_all(&profile).unwrap();
        let staged_save = profile.join("SGTA50000");
        fs::write(&staged_save, b"synthetic-save-fixture").unwrap();
        let staged_settings = profile.join("pc_settings.bin");
        fs::write(&staged_settings, b"synthetic-settings").unwrap();

        sync_legacy_saves_to_default(user);

        let default_save = root.join("Default/SGTA50000");
        assert!(default_save.is_file());
        assert_eq!(fs::metadata(&default_save).unwrap().len(), fs::metadata(&staged_save).unwrap().len());
        let default_settings = root.join("Default/pc_settings.bin");
        assert!(default_settings.is_file());
        assert_eq!(fs::metadata(&default_settings).unwrap().len(), fs::metadata(&staged_settings).unwrap().len());
    }
}
