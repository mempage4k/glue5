//! Android's thermal state (the NDK's `thermal.h`: the status since API 30,
//! the headroom forecast since API 31; looked up at run time, as the
//! performance hints are) and what to do about it.
//!
//! [`Governor`] turns readings into a level, 0 (no back-off) to 3, with
//! hysteresis: it rises after the heat has been asked for a few seconds and
//! falls one level at a time, slowly, so a phone near a threshold does not
//! flap between 40 and 30 fps. The system's own verdict (status moderate or
//! worse) is a floor the level jumps to at once.
//!
//! The numbers are a first guess (the headroom thresholds follow the Unreal
//! ADPF plugin's own steps): tune them from the `thermal:` log lines on the
//! S21 Plus. The logic is plain Rust and tested on any platform; only
//! [`read`] is Android's.

use std::time::{Duration, Instant};

/// How far ahead the headroom is forecast, in seconds (0 to 60).
pub const FORECAST_S: i32 = 10;

/// The highest level.
pub const MAX_LEVEL: u8 = 3;

/// Headroom (the forecast) that asks for level 1, 2, 3 (1.0 is the system's
/// severe threshold).
const UP: [f32; 3] = [0.80, 0.90, 0.97];
/// Headroom below which level 1, 2, 3 is let go: a gap under each `UP`.
const DOWN: [f32; 3] = [0.70, 0.80, 0.90];
/// How long a rise must be asked for, and a fall (one level each time).
const RAISE_AFTER: Duration = Duration::from_secs(6);
const LOWER_AFTER: Duration = Duration::from_secs(45);
/// Readings are this far apart at least: the system updates its thermal
/// state about once a second, and asks not to be read faster.
const POLL: Duration = Duration::from_secs(2);
/// The app logs the state this often, so a session's log holds the curve.
const LOG_EVERY: Duration = Duration::from_secs(10);

/// One reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// `AThermalStatus`: 0 none, 1 light, 2 moderate, 3 severe, 4 critical,
    /// 5 emergency, 6 shutdown; -1 when it cannot be read.
    pub status: i32,
    /// Forecast `FORECAST_S` ahead, 1.0 at the severe threshold; NaN when
    /// the device does not say (Android 11 has no headroom).
    pub headroom: f32,
}

impl Sample {
    /// The system's own state, in a word (for the log).
    pub fn status_name(&self) -> &'static str {
        match self.status {
            0 => "none",
            1 => "light",
            2 => "moderate",
            3 => "severe",
            4 => "critical",
            5 => "emergency",
            6 => "shutdown",
            _ => "unknown",
        }
    }
}

/// The level a reading asks for from `level`, and the floor the system's
/// status sets (the level is never below it).
fn wanted(level: u8, s: Sample) -> (u8, u8) {
    let floor = match s.status {
        i32::MIN..=1 => 0,
        2 => 1,
        3 => 2,
        _ => MAX_LEVEL,
    };
    let mut want = level;
    if s.headroom.is_finite() {
        while want < MAX_LEVEL && s.headroom >= UP[want as usize] {
            want += 1;
        }
        while want > 0 && s.headroom < DOWN[want as usize - 1] {
            want -= 1;
        }
    } else {
        // Status alone: the level follows it.
        want = floor;
    }
    (want.max(floor), floor)
}

/// Frames per second at most for `level` on a display of `refresh_hz`; 0:
/// no cap. On a 120 Hz panel 40 is three refreshes a frame, even; on 60 Hz
/// the only even step below 60 is 30.
pub fn cap(level: u8, refresh_hz: f32) -> u32 {
    match (level, refresh_hz >= 100.0) {
        (0, _) => 0,
        (1, true) => 40,
        _ => 30,
    }
}

/// How much of the render scale to keep at `level`: only the last level
/// asks (the engine rebuilds its render targets for each size, which
/// hitches).
pub fn scale(level: u8) -> f32 {
    if level >= MAX_LEVEL { 0.85 } else { 1.0 }
}

/// Reads the system. None off Android, or when it has no thermal manager.
#[cfg(target_os = "android")]
pub fn read() -> Option<Sample> {
    android::read()
}

#[cfg(not(target_os = "android"))]
pub fn read() -> Option<Sample> {
    None
}

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CStr, c_void};
    use std::sync::OnceLock;

    use super::{FORECAST_S, Sample};

    struct Api {
        manager: *mut c_void,
        status: unsafe extern "C" fn(*mut c_void) -> i32,
        /// API 31+.
        headroom: Option<unsafe extern "C" fn(*mut c_void, i32) -> f32>,
    }
    // SAFETY: the manager is a process-wide singleton, usable from any thread.
    unsafe impl Send for Api {}
    unsafe impl Sync for Api {}

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
            let acquire: unsafe extern "C" fn() -> *mut c_void =
                std::mem::transmute(sym(c"AThermal_acquireManager")?);
            // Kept for the life of the process, never released.
            let manager = acquire();
            if manager.is_null() {
                return None;
            }
            Some(Api {
                manager,
                status: std::mem::transmute(sym(c"AThermal_getCurrentThermalStatus")?),
                headroom: sym(c"AThermal_getThermalHeadroom").map(|p| std::mem::transmute(p)),
            })
        })
        .as_ref()
    }

    pub fn read() -> Option<Sample> {
        let api = api()?;
        let status = unsafe { (api.status)(api.manager) };
        let headroom = api.headroom.map_or(f32::NAN, |f| unsafe { f(api.manager, FORECAST_S) });
        Some(Sample { status, headroom })
    }
}

/// Reads the system now and then and holds the level.
#[derive(Default)]
pub struct Governor {
    level: u8,
    /// When the current rise (true) or fall (false) was first asked for.
    pending: Option<(Instant, bool)>,
    last_read: Option<Instant>,
    last: Option<Sample>,
    logged: Option<Instant>,
}

impl Governor {
    /// The level now.
    pub fn level(&self) -> u8 {
        self.level
    }

    /// The last reading, for the log.
    pub fn last(&self) -> Option<Sample> {
        self.last
    }

    /// True about every ten seconds: time to log the state.
    pub fn log_due(&mut self, now: Instant) -> bool {
        if self.logged.is_some_and(|t| now.saturating_duration_since(t) < LOG_EVERY) {
            return false;
        }
        self.logged = Some(now);
        true
    }

    /// Reads the system if it is time (see `POLL`); true if the level changed.
    pub fn poll(&mut self, now: Instant) -> bool {
        if self.last_read.is_some_and(|t| now.saturating_duration_since(t) < POLL) {
            return false;
        }
        self.last_read = Some(now);
        match read() {
            Some(sample) => self.step(now, sample),
            None => false,
        }
    }

    /// One reading; true if the level changed.
    pub fn step(&mut self, now: Instant, s: Sample) -> bool {
        self.last = Some(s);
        let before = self.level;
        let (want, floor) = wanted(self.level, s);
        if self.level < floor {
            // The system says so: at once.
            self.level = floor;
            self.pending = None;
        } else if want == self.level {
            self.pending = None;
        } else {
            let raising = want > self.level;
            let after = if raising { RAISE_AFTER } else { LOWER_AFTER };
            match self.pending {
                Some((since, r)) if r == raising => {
                    if now.saturating_duration_since(since) >= after {
                        self.level = if raising { self.level + 1 } else { self.level - 1 };
                        self.pending = None;
                    }
                }
                _ => self.pending = Some((now, raising)),
            }
        }
        self.level != before
    }

    /// Back to level 0 (the game ended, or the setting is off).
    pub fn reset(&mut self) {
        *self = Governor::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(status: i32, headroom: f32) -> Sample {
        Sample { status, headroom }
    }

    #[test]
    fn rises_only_after_the_heat_lasts() {
        let (mut g, t0) = (Governor::default(), Instant::now());
        assert!(!g.step(t0, s(1, 0.85)));
        assert!(!g.step(t0 + Duration::from_secs(4), s(1, 0.85)));
        assert!(g.step(t0 + Duration::from_secs(6), s(1, 0.85)));
        assert_eq!(g.level(), 1);
    }

    #[test]
    fn a_blip_resets_the_wait() {
        let (mut g, t0) = (Governor::default(), Instant::now());
        g.step(t0, s(1, 0.85));
        g.step(t0 + Duration::from_secs(4), s(1, 0.5));
        assert!(!g.step(t0 + Duration::from_secs(8), s(1, 0.85)));
        assert_eq!(g.level(), 0);
    }

    #[test]
    fn the_systems_status_is_a_floor_at_once() {
        let (mut g, t0) = (Governor::default(), Instant::now());
        assert!(g.step(t0, s(3, 0.2)));
        assert_eq!(g.level(), 2);
        assert!(g.step(t0 + POLL, s(4, f32::NAN)));
        assert_eq!(g.level(), MAX_LEVEL);
    }

    #[test]
    fn falls_one_level_at_a_time_and_slowly() {
        let (mut g, mut t) = (Governor::default(), Instant::now());
        g.level = 2;
        g.step(t, s(1, 0.6));
        t += Duration::from_secs(44);
        assert!(!g.step(t, s(1, 0.6)));
        t += Duration::from_secs(2);
        assert!(g.step(t, s(1, 0.6)));
        assert_eq!(g.level(), 1);
    }

    #[test]
    fn no_flapping_between_the_thresholds() {
        let (mut g, t0) = (Governor::default(), Instant::now());
        let mut changes = 0;
        for i in 0..100u64 {
            let h = if i % 2 == 0 { 0.81 } else { 0.75 };
            changes += g.step(t0 + POLL * i as u32, s(1, h)) as u32;
        }
        assert_eq!(changes, 0);
    }

    #[test]
    fn without_headroom_the_level_follows_the_status() {
        let (mut g, t0) = (Governor::default(), Instant::now());
        g.step(t0, s(2, f32::NAN));
        assert_eq!(g.level(), 1);
        g.step(t0 + POLL, s(0, f32::NAN));
        g.step(t0 + POLL * 30, s(0, f32::NAN));
        assert_eq!(g.level(), 0);
    }

    #[test]
    fn caps_follow_the_display() {
        assert_eq!((cap(0, 120.0), cap(1, 120.0), cap(2, 120.0), cap(3, 120.0)), (0, 40, 30, 30));
        assert_eq!((cap(1, 60.0), cap(2, 60.0)), (30, 30));
        assert_eq!((scale(2), scale(3)), (1.0, 0.85));
    }

    #[test]
    fn the_states_are_named() {
        assert_eq!(s(0, 0.0).status_name(), "none");
        assert_eq!(s(2, 0.0).status_name(), "moderate");
        assert_eq!(s(-1, 0.0).status_name(), "unknown");
    }
}
