use std::{path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=GLUE5_BUILD_ID");
    // The game built instrumented for PGO (`GLUE5_GAME_PGO=generate`): the
    // app then tells the profile runtime where to write its file.
    println!("cargo:rerun-if-env-changed=GLUE5_GAME_PGO");
    println!("cargo:rustc-check-cfg=cfg(glue5_game_pgo)");
    if std::env::var("GLUE5_GAME_PGO").as_deref() == Ok("generate") {
        println!("cargo:rustc-cfg=glue5_game_pgo");
    }

    let repo_root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets the manifest directory"))
        .join("../..");
    println!("cargo:rerun-if-changed={}", repo_root.join("VERSION_CODENAME").display());

    let from_env = std::env::var("GLUE5_BUILD_ID")
        .or_else(|_| std::env::var("GITHUB_SHA"))
        .ok();
    let from_git = || {
        let output = Command::new("git")
            .args(["rev-parse", "--short=7", "HEAD"])
            .current_dir(&repo_root)
            .output()
            .ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let build_id = from_env
        .or_else(from_git)
        .map(|value| value.trim().chars().take(7).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local".to_owned());

    let codename = std::fs::read_to_string(repo_root.join("VERSION_CODENAME")).unwrap_or_else(|_| "unknown".to_owned());
    let codename = codename.trim();
    println!("cargo:rustc-env=GLUE5_BUILD_ID={build_id}");
    println!("cargo:rustc-env=GLUE5_VERSION_CODENAME={codename}");
}
