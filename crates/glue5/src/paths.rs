//! Where things are, per platform.
//!
//! - The user directory (settings, saves, caches) is SDL's per-user
//!   application directory: `~/Library/Application Support/glue5` on macOS,
//!   `~/.local/share/glue5` on Linux, `%APPDATA%\glue5` on Windows, the app's
//!   sandbox on iOS and Android.
//! - The game (`b/<build>/` and `data/`, ~21 GB) is never part of the app: it
//!   is wherever the user put it. Mobile defaults are user-visible places
//!   (iOS: the app's Documents, through Finder or Files; Android: the app's
//!   external files directory). The Apple TV has none: it keeps only caches
//!   the system may empty, and the game always streams from a server.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Phones, tablets and the Apple TV: apps the system runs full screen and
/// starts without arguments, which cannot quit or start themselves again.
pub const MOBILE: bool = cfg!(any(target_os = "ios", target_os = "android", target_os = "tvos"));

/// A touch screen is the main input (phones and tablets): the touch
/// controls and their settings button.
pub const TOUCH: bool = cfg!(any(target_os = "ios", target_os = "android"));

/// The Apple TV: no game folder (what it keeps, the system may delete); the
/// game streams from a server.
pub const TV: bool = cfg!(target_os = "tvos");

pub fn user_dir() -> Result<PathBuf> {
    sdl3::filesystem::get_pref_path("", "glue5").map_err(|e| anyhow::anyhow!("no user directory: {e}"))
}

/// Where the game is looked for when nothing says otherwise.
pub fn default_game_dirs() -> Vec<PathBuf> {
    if TV {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    if cfg!(target_os = "ios") {
        if let Ok(docs) = sdl3::filesystem::get_user_folder(sdl3::filesystem::Folder::DOCUMENTS) {
            dirs.push(docs.join("game"));
        }
    } else if cfg!(target_os = "android") {
        #[cfg(target_os = "android")]
        if let Some(ext) = android_external_dir() {
            dirs.push(ext.join("game"));
        }
    } else {
        // A distributed Mac app can sit beside an external game directory.
        // SDL's base path is inside the bundle, not beside the .app.
        #[cfg(target_os = "macos")]
        if let Ok(exe) = std::env::current_exe()
            && let Some(bundle) = exe.parent().and_then(Path::parent).and_then(Path::parent)
            && bundle.extension().is_some_and(|ext| ext == "app")
            && let Some(parent) = bundle.parent()
        {
            dirs.push(parent.join("game"));
        }
        // Likewise beside an AppImage: the executable runs from where it is
        // mounted, a temporary directory; its runtime says where it is.
        #[cfg(target_os = "linux")]
        if let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from)
            && let Some(parent) = appimage.parent()
        {
            dirs.push(parent.join("game"));
        }
        // Next to the executable, the repository's ./game (target/<profile>/glue5),
        // and the working directory.
        if let Ok(base) = sdl3::filesystem::get_base_path() {
            dirs.push(base.join("game"));
            dirs.push(base.join("../../game"));
        }
        dirs.push(PathBuf::from("game"));
    }
    dirs
}

#[cfg(target_os = "android")]
pub(crate) fn android_external_dir() -> Option<PathBuf> {
    let p = unsafe { sdl3::sys::system::SDL_GetAndroidExternalStoragePath() };
    (!p.is_null()).then(|| PathBuf::from(unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned()))
}

/// Files first copied by Android's document picker.  It deliberately lives
/// beside the game, where Java and Rust agree on the path; the picker never
/// writes into the live save or game tree.
pub fn imports_dir(user_dir: &Path) -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some(external) = android_external_dir() {
        return external.join("imports");
    }
    user_dir.join("imports")
}

/// The mods' folder. On phones it is beside the game's (`Documents/mods`
/// on iOS, where the Files app and the Finder reach it; the external
/// folder on Android), elsewhere in the user directory. A folder from
/// before (in the user directory) is moved there once.
pub fn mods_dir(user_dir: &Path) -> PathBuf {
    let old = user_dir.join("mods");
    let dir = beside_game().map_or_else(|| old.clone(), |d| d.join("mods"));
    if dir != old && old.is_dir() && !dir.exists() {
        // The same volume on iOS: a rename; on Android a copy.
        if std::fs::rename(&old, &dir).is_err() {
            match copy_dir(&old, &dir) {
                Ok(()) => {
                    let _ = std::fs::remove_dir_all(&old);
                }
                Err(e) => {
                    log::warn!("mods: moving {} to {}: {e}", old.display(), dir.display());
                    // A partial copy would pass for the moved folder next time.
                    let _ = std::fs::remove_dir_all(&dir);
                    return old;
                }
            }
        }
        log::info!("mods: moved to {}", dir.display());
    }
    dir
}

/// The name used when scenes are placed beside or inside the game folder.
pub const SCENES_FOLDER: &str = "glue5-scenes";

/// The name used when the interface's sounds are placed beside or inside the
/// game folder (see `docs/menu-sfx.md`).
pub const SFX_FOLDER: &str = "glue5-sfx";

/// The app's own folder for the menus' own sounds: `sfx/`, beside scenes.
pub fn sfx_dir(user_dir: &Path) -> PathBuf {
    beside_game().map_or_else(|| user_dir.join("sfx"), |dir| dir.join("sfx"))
}

/// The app's own fallback folder for optional start-page scenes.
pub fn scenes_dir(user_dir: &Path) -> PathBuf {
    beside_game().map_or_else(|| user_dir.join("scenes"), |dir| dir.join("scenes"))
}

/// The app's own fallback folder for optional menu fonts (the game's own
/// cuts, supplied by the player; never bundled — see
/// `docs/gta5-visual-parity.md`).
pub fn fonts_dir(user_dir: &Path) -> PathBuf {
    beside_game().map_or_else(|| user_dir.join("fonts"), |dir| dir.join("fonts"))
}

/// Where a phone keeps the game's folder (its parent).
fn beside_game() -> Option<PathBuf> {
    #[cfg(target_os = "ios")]
    return sdl3::filesystem::get_user_folder(sdl3::filesystem::Folder::DOCUMENTS).ok().map(Path::to_path_buf);
    #[cfg(target_os = "android")]
    return android_external_dir();
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    None
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The app's cache folder: what can be fetched again (the stream's copy of
/// the game, the packs this computer shares). iOS and tvOS may empty it
/// while the app is not running; the stream then fetches again.
pub fn cache_dir(user_dir: &Path) -> PathBuf {
    // SDL's user directory there is Library/Application Support/glue5 (on
    // tvOS Library/Caches/glue5, which is this one).
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    if let Some(library) = user_dir.parent().and_then(Path::parent) {
        return library.join("Caches").join("glue5");
    }
    #[cfg(target_os = "android")]
    if let Some(ext) = android_external_dir() {
        return ext.join("cache");
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Caches/glue5");
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local).join("glue5").join("cache");
    }
    #[cfg(target_os = "linux")]
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
    {
        return cache.join("glue5");
    }
    user_dir.join("cache")
}

/// The stream's copy of the game (a game folder's layout, partial).
pub fn stream_copy(user_dir: &Path) -> PathBuf {
    cache_dir(user_dir).join("stream").join(glue5_runtime::GAME_BUILD)
}

/// Where the whole game goes when it is downloaded from a server: where a
/// copied game would be (`Documents/game`, the external files' `game`, the
/// user directory's `game` on desktop). None on tvOS, which keeps nothing.
pub fn download_dir(user_dir: &Path) -> Option<PathBuf> {
    if !can_download_all() {
        None
    } else if MOBILE {
        default_game_dirs().into_iter().next()
    } else {
        Some(user_dir.join("game"))
    }
}

/// The device keeps files for good (tvOS keeps only caches).
pub fn can_download_all() -> bool {
    !TV
}

/// Where the game's files go, for the start page when they are missing.
pub fn where_the_game_goes() -> &'static str {
    if cfg!(target_os = "ios") {
        "Copy the game folder into the app's files, named game: Finder (iPhone › Files › Glue 5) \
         or the Files app (On My iPhone › Glue 5). This page looks again when the app comes back."
    } else if cfg!(target_os = "android") {
        "Use Find files to choose the game folder wherever it is (recommended). Advanced: from a \
         computer, run adb push game /sdcard/Android/data/local.glue5/files/."
    } else {
        "Choose the game folder, or start glue5 with its path."
    }
}

/// How the game streams instead, for the start page.
pub fn or_stream() -> &'static str {
    if MOBILE {
        "Or play it from a device that has it, over the network."
    } else {
        "Or play it from another device that has it, over the network."
    }
}

/// What the game folder holds.
pub fn game_layout() -> String {
    format!("game/\n  b/{}/shaders/\n  data/", glue5_runtime::GAME_BUILD)
}

/// Keeps `dir` (the game's folder, tens of GB) out of the device's iCloud
/// and computer backups, which take Documents whole by default. Set again
/// each time: the mark goes with the folder when it is copied in anew (one
/// not made yet is marked the next time).
pub fn keep_out_of_backups(dir: &Path) {
    #[cfg(target_os = "ios")]
    if dir.is_dir() {
        use objc2_foundation::{NSNumber, NSString, NSURL, NSURLIsExcludedFromBackupKey};
        let url = NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()));
        // SAFETY: the key takes an NSNumber holding a BOOL.
        if let Err(e) = unsafe { url.setResourceValue_forKey_error(Some(&NSNumber::numberWithBool(true)), NSURLIsExcludedFromBackupKey) } {
            log::warn!("{}: still in the backups: {e}", dir.display());
        }
    }
    #[cfg(not(target_os = "ios"))]
    let _ = dir;
}

pub fn canonical(dir: &Path) -> Result<PathBuf> {
    dir.canonicalize().with_context(|| format!("{}", dir.display()))
}
