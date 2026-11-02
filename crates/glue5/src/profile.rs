//! The game's profile, when the game was built instrumented for PGO
//! (`GLUE5_GAME_PGO=generate`): the file the compiler's profile runtime
//! writes when the process ends.
//!
//! The runtime reads `LLVM_PROFILE_FILE` when the library is loaded, so
//! the phone's activity sets it (see Glue5Activity): this module makes
//! sure the folder is there and says where the file goes.

use std::path::Path;

unsafe extern "C" {
    /// The profile runtime's own function: writes the profile file.
    ///
    /// The runtime would write it when the process ends, but the app ends
    /// the process without the exit handlers (the engine's threads never
    /// end; see `glue5_runtime::exit::process`), so the runtime is told
    /// to write it while it still can.
    fn __llvm_profile_write_file() -> core::ffi::c_int;
}

/// Writes the profile file.
pub fn write() {
    // SAFETY: the runtime's own function, in a build with the runtime
    // linked in (the game's build script says so: `GLUE5_GAME_PGO=generate`
    // links the profile runtime).
    let code = unsafe { __llvm_profile_write_file() };
    if code == 0 {
        log::info!("pgo: the profile was written");
    } else {
        log::warn!("pgo: the profile was not written ({code})");
    }
}

/// Makes sure the profile's folder is there, and says where the file goes.
///
/// The folder is the one the variable names, so this needs nothing from
/// the platform: the activity sets the variable before the library is
/// loaded, and the desktop task sets it before the run.
pub fn prepare() {
    match std::env::var("LLVM_PROFILE_FILE") {
        Ok(path) => {
            // The folder the file goes to: the runtime writes it when the
            // process ends, and makes no folder of its own.
            if let Some(folder) = Path::new(&path).parent()
                && !folder.as_os_str().is_empty()
            {
                if let Err(e) = std::fs::create_dir_all(folder) {
                    log::warn!("pgo: cannot make {}: {e}", folder.display());
                    return;
                }
            }
            log::info!("pgo: the profile goes to {path}");
        }
        // The runtime reads the variable when the library is loaded; the
        // activity sets it (GLUE5_PROFILE). Without it the file goes to
        // the current folder, which on a phone is not writable.
        Err(_) => log::warn!("pgo: LLVM_PROFILE_FILE is not set; the profile would go to the current folder"),
    }
}
