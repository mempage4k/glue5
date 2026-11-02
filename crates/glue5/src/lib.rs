//! Glue 5: the game as one application. Android is the only officially
//! supported target (Galaxy S21 Plus); the desktop and iOS entry points
//! below are historical build paths, unsupported and unvalidated.
//!
//! The engine runs on its own threads ([`glue5_runtime`]) and renders with
//! [`glue5_gpu`]; this crate is the platform side, through SDL: window,
//! input, audio and the application's own settings. One entry point serves
//! every platform:
//!
//! - desktop: the `glue5` binary calls [`main`];
//! - iOS: the Xcode project's `main` calls `glue5_main`, which hands the
//!   process to SDL (`SDL_RunApp` / `UIApplicationMain`);
//! - Android: SDL's Java activity (`platform/android`) loads SDL's library
//!   and this one (built as `libglue5.so`), then calls `SDL_main`.

mod app;
mod audio;
mod benchmark;
mod device;
mod drivers;
mod haptics;
#[cfg(target_os = "android")]
mod jni_util;
#[cfg(target_os = "android")]
mod lan;
mod imports;
mod keyboard;
mod keymap;
mod pads;
mod paths;
mod settings;
mod sfx;
mod sharing;
mod touch;
mod ui;
mod version;

pub use device::name as device_name;

use sdl3::messagebox::{MessageBoxFlag, show_simple_message_box};

/// Runs the application; returns the process exit code.
pub fn main() -> i32 {
    #[cfg(not(target_os = "android"))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    #[cfg(target_os = "android")]
    {
        // RUST_LOG as on desktop (the launch intent's extras, see Glue5Activity).
        let spec = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
        let filter = android_logger::FilterBuilder::new().parse(&spec).build();
        let level = filter.filter();
        android_logger::init_once(android_logger::Config::default().with_tag("glue5").with_max_level(level).with_filter(filter));
    }
    // A host thread serving the engine (the renderer's, ...) that panics
    // leaves it waiting forever, as an engine thread's trap does.
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        if glue5_runtime::exit::is_panic_recoverable() {
            return;
        }
        let thread = std::thread::current();
        let what = info.payload_as_str().unwrap_or("panic");
        glue5_runtime::exit::fail(format!("its {} thread stopped ({what})", thread.name().unwrap_or("unnamed")));
    }));
    match app::run() {
        Ok(()) => 0,
        Err(e) => {
            log::error!("{e:#}");
            let _ = show_simple_message_box(MessageBoxFlag::ERROR, "Glue 5", &format!("{e:#}"), None);
            1
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "tvos"))]
#[unsafe(no_mangle)]
pub extern "C" fn glue5_main(argc: std::ffi::c_int, argv: *mut *mut std::ffi::c_char) -> std::ffi::c_int {
    // SDL's UIKit backend keeps the process running once its main returns
    // (for apps that leave the rest to the run loop), and the engine's
    // threads never end: the window would stay black. Ends it, as desktop does.
    unsafe extern "C" fn sdl_main(_: std::ffi::c_int, _: *mut *mut std::ffi::c_char) -> std::ffi::c_int {
        glue5_runtime::exit::process(main())
    }
    unsafe { sdl3::sys::main::SDL_RunApp(argc, argv, Some(sdl_main), std::ptr::null_mut()) }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn SDL_main(_: std::ffi::c_int, _: *mut *mut std::ffi::c_char) -> std::ffi::c_int {
    // The process ends with main, as on desktop and iOS: the engine's threads
    // never end, and SDL's activity only finishes when main returns, leaving
    // them running in the process Android keeps cached.
    glue5_runtime::exit::process(main())
}
