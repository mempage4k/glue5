//! Thread priorities. On Apple platforms they are quality-of-service classes,
//! which also decide between performance and efficiency cores: the threads a
//! frame waits on are interactive, the engine's others just below. On
//! Android the interactive threads form a performance hint session told how
//! long frames take ([`frame`]), from which the system sets the CPU clocks;
//! there and on Linux each class is also a nice value, Android's own for
//! such a thread, and interactive threads' timers have no slack.

/// Android's thermal state, and how far to back off for it.
pub mod thermal;

#[derive(Clone, Copy, Debug)]
pub enum Qos {
    /// Sound for the device, which plays it as it comes (the engine's mixer
    /// and the audio thread feeding it).
    Audio,
    /// A frame waits on it (the engine's main thread, the renderer).
    Interactive,
    /// Work the game needs soon (engine workers, shaders being prepared).
    Initiated,
    /// Background work (logs, pipelines compiled while playing).
    Utility,
}

/// Sets the calling thread's class.
pub fn set(qos: Qos) {
    #[cfg(target_vendor = "apple")]
    unsafe {
        use libc::qos_class_t::*;
        let class = match qos {
            Qos::Audio | Qos::Interactive => QOS_CLASS_USER_INTERACTIVE,
            Qos::Initiated => QOS_CLASS_USER_INITIATED,
            Qos::Utility => QOS_CLASS_UTILITY,
        };
        libc::pthread_set_qos_class_self_np(class, 0);
    }
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        nice::set(qos);
        slack::set(qos);
    }
    #[cfg(target_os = "android")]
    if let Qos::Interactive = qos {
        hint::join();
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "linux")))]
    let _ = qos;
}

/// Starts a host thread named `name` in class `qos`. Without one a thread
/// keeps its creator's: on Linux and Android a frame's thread's nice value
/// and timer slack (or the display's, SDL's main thread on Android) for
/// work that is not a frame's.
pub fn spawn<T: Send + 'static>(
    name: &str,
    qos: Qos,
    f: impl FnOnce() -> T + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<T>> {
    std::thread::Builder::new().name(name.into()).spawn(move || {
        set(qos);
        f()
    })
}

/// Linux's priorities: nice values, which weigh each thread's share of a
/// busy core (not a strict order, as Apple's classes are). Android's own
/// for each class: its audio threads' (SDL's included), the display's
/// (SDL's main thread), the default, background work's.
#[cfg(any(target_os = "android", target_os = "linux"))]
mod nice {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::Qos;

    const AUDIO: i32 = -16;
    const DISPLAY: i32 = -4;
    const BACKGROUND: i32 = 10;

    pub fn set(qos: Qos) {
        let nice = match qos {
            Qos::Audio => AUDIO,
            Qos::Interactive => DISPLAY,
            Qos::Initiated => 0,
            Qos::Utility => BACKGROUND,
        };
        if !allowed() {
            return;
        }
        // (On Linux the nice value is the thread's, not the process's.)
        if unsafe { libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, nice) } != 0 {
            static TOLD: AtomicBool = AtomicBool::new(false);
            if !TOLD.swap(true, Ordering::Relaxed) {
                log::warn!("threads: nice {nice}: {}", std::io::Error::last_os_error());
            }
        }
    }

    /// Whether the classes can be set: a value below the thread's needs
    /// RLIMIT_NICE (Android's apps have 40: -20 to 19; a desktop's users
    /// usually none), and without it a thread lowered for background work
    /// would not come back either.
    fn allowed() -> bool {
        static ALLOWED: OnceLock<bool> = OnceLock::new();
        *ALLOWED.get_or_init(|| {
            let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            let ok = unsafe { libc::getrlimit(libc::RLIMIT_NICE, &mut limit) } == 0;
            let allowed = ok && limit.rlim_cur >= (20 - AUDIO) as libc::rlim_t;
            if !allowed {
                log::info!("threads: one priority (RLIMIT_NICE {})", limit.rlim_cur);
            }
            allowed
        })
    }
}

/// Timer slack: how late Linux may end a thread's timed sleeps and waits,
/// to wake it with others' (50 µs by default, as Android's in the
/// foreground). The engine's waits on a frame's path are short:
/// `sysIpcYield` sleeps 20 µs at a time in its loops (the main thread's
/// render lists, among others), 70 with the slack. Interactive threads get
/// none; threads inherit it, so the others get back the process's. (Android
/// 10-12 set every thread's back to 50 µs as the app comes back to the
/// foreground; from 13 on the system's profiles go to the whole process,
/// which leaves it.)
#[cfg(any(target_os = "android", target_os = "linux"))]
mod slack {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::Qos;

    pub fn set(qos: Qos) {
        // The process's, as the first thread to get a class has it (no
        // thread's changed yet).
        static NORMAL: OnceLock<i32> = OnceLock::new();
        let normal = *NORMAL.get_or_init(|| unsafe { libc::prctl(libc::PR_GET_TIMERSLACK) });
        if normal <= 0 {
            return;
        }
        let slack = match qos {
            Qos::Interactive => 1,
            _ => normal,
        };
        if unsafe { libc::prctl(libc::PR_SET_TIMERSLACK, slack as libc::c_ulong) } != 0 {
            static TOLD: AtomicBool = AtomicBool::new(false);
            if !TOLD.swap(true, Ordering::Relaxed) {
                log::warn!("threads: timer slack {slack} ns: {}", std::io::Error::last_os_error());
            }
        }
    }
}

/// A frame was shown `took` after the one before, for `target`.
pub fn frame(took: std::time::Duration, target: std::time::Duration) {
    #[cfg(target_os = "android")]
    hint::frame(took, target);
    #[cfg(not(target_os = "android"))]
    let _ = (took, target);
}

/// Asks Android to refresh the window `native` (an `ANativeWindow`) at the
/// frame cap `fps` (0: no preference; Android 11+, looked up at run time):
/// a 120 Hz panel otherwise stays at 120 for a game capped at 60, its frames
/// going out 1 or 2 refreshes apart, and nothing holding them to 60.
/// Compatibility "default", which also keeps touch from raising the rate.
#[cfg(target_os = "android")]
pub fn ask_frame_rate(native: *mut std::ffi::c_void, fps: u32) {
    type SetFrameRate = unsafe extern "C" fn(*mut std::ffi::c_void, f32, i8) -> i32;
    static SET: std::sync::OnceLock<Option<SetFrameRate>> = std::sync::OnceLock::new();
    let set = SET.get_or_init(|| unsafe {
        let lib = libc::dlopen(c"libandroid.so".as_ptr(), libc::RTLD_NOW);
        let p = if lib.is_null() { std::ptr::null_mut() } else { libc::dlsym(lib, c"ANativeWindow_setFrameRate".as_ptr()) };
        (!p.is_null()).then(|| std::mem::transmute::<*mut libc::c_void, SetFrameRate>(p))
    });
    if let Some(set) = set
        && !native.is_null()
    {
        let status = unsafe { set(native, fps as f32, 0) };
        log::debug!("display: {fps} fps asked ({status})");
    }
}

/// Android's performance hints (ADPF, Android 13+; looked up at run time).
/// Without them the system's defaults keep the big cores' clocks low for a
/// game that never fully loads one thread, as the engine's pipeline does.
#[cfg(target_os = "android")]
mod hint {
    use std::ffi::{CStr, c_void};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    struct Api {
        manager: *mut c_void,
        create: unsafe extern "C" fn(*mut c_void, *const i32, usize, i64) -> *mut c_void,
        close: unsafe extern "C" fn(*mut c_void),
        update_target: unsafe extern "C" fn(*mut c_void, i64) -> i32,
        report: unsafe extern "C" fn(*mut c_void, i64) -> i32,
        /// Android 14+; before, the session is made again with the new thread.
        set_threads: Option<unsafe extern "C" fn(*mut c_void, *const i32, usize) -> i32>,
    }
    // SAFETY: the manager is a process-wide singleton, usable from any thread.
    unsafe impl Send for Api {}
    unsafe impl Sync for Api {}

    struct Session {
        threads: Vec<i32>,
        session: *mut c_void,
        target: i64,
    }
    // SAFETY: the session is only used under the mutex.
    unsafe impl Send for Session {}

    static SESSION: Mutex<Session> =
        Mutex::new(Session { threads: Vec::new(), session: std::ptr::null_mut(), target: 0 });

    fn api() -> Option<&'static Api> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(|| unsafe {
            let lib = libc::dlopen(c"libandroid.so".as_ptr(), libc::RTLD_NOW);
            if lib.is_null() {
                return None;
            }
            let sym = |name: &CStr| {
                let p = libc::dlsym(lib, name.as_ptr());
                (!p.is_null()).then_some(p)
            };
            let get_manager: unsafe extern "C" fn() -> *mut c_void =
                std::mem::transmute(sym(c"APerformanceHint_getManager")?);
            let manager = get_manager();
            if manager.is_null() {
                return None;
            }
            Some(Api {
                manager,
                create: std::mem::transmute(sym(c"APerformanceHint_createSession")?),
                close: std::mem::transmute(sym(c"APerformanceHint_closeSession")?),
                update_target: std::mem::transmute(sym(c"APerformanceHint_updateTargetWorkDuration")?),
                report: std::mem::transmute(sym(c"APerformanceHint_reportActualWorkDuration")?),
                set_threads: sym(c"APerformanceHint_setThreads").map(|p| std::mem::transmute(p)),
            })
        })
        .as_ref()
    }

    /// Adds the calling thread to the session.
    pub fn join() {
        let Some(api) = api() else { return };
        let mut s = SESSION.lock().unwrap();
        s.threads.push(unsafe { libc::gettid() });
        if s.target == 0 {
            s.target = Duration::from_secs(1).as_nanos() as i64 / 60;
        }
        unsafe {
            if !s.session.is_null()
                && let Some(set_threads) = api.set_threads
            {
                set_threads(s.session, s.threads.as_ptr(), s.threads.len());
                return;
            }
            if !s.session.is_null() {
                (api.close)(s.session);
            }
            s.session = (api.create)(api.manager, s.threads.as_ptr(), s.threads.len(), s.target);
        }
        if s.session.is_null() {
            log::warn!("no performance hint session");
        }
    }

    pub fn frame(took: Duration, target: Duration) {
        let Some(api) = api() else { return };
        let mut s = SESSION.lock().unwrap();
        if s.session.is_null() {
            return;
        }
        let target = target.as_nanos() as i64;
        unsafe {
            if target > 0 && target != s.target {
                s.target = target;
                (api.update_target)(s.session, target);
            }
            (api.report)(s.session, took.as_nanos().max(1) as i64);
        }
    }
}
