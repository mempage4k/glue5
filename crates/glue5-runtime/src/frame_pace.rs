//! The engine's frame limiter (`-frameLimit`, at the end of
//! `CSystem::EndRender` on the render thread) waits for 95% of the frame's
//! time since the last frame: in sleeps of whole milliseconds, less its
//! worst oversleep so far (1.5 ms at least, up to 16.5, forgotten 0.1% a
//! frame), then spinning on the clock for the rest. Apple's and Android's
//! sleeps run up to a fifth long, so that worst sits at 2-6 ms and the
//! render thread spun 0.5-3.5 ms a frame (a tenth of a core at 60 or 30
//! fps, which a phone's GPU and loading screens want).
//!
//! The engine's limiter now finds every frame late and lets it go; the
//! host then waits for the same deadline, in sleeps of half what is left
//! (none too long however late each comes back), and stores the frame's
//! end as the engine would have.
//!
//! A phone that heats up holds the game to fewer frames a second
//! ([`set_thermal_cap`]): the host waits for the slower of the two.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use glue5_game::{Env, futex, orig};

use crate::memory::Mem;

/// When the limiter last let a frame go (`sysTimer` ticks, u64), and the
/// seconds a tick lasts (f32). Addresses in this build.
const LAST_END: u64 = 25038496;
const TICK_SECONDS: u64 = 9779848;
/// The display's refresh rate (numerator, denominator) and the refresh
/// intervals a frame lasts (the VSync setting `-frameLimit` seeds; 0: off).
const REFRESH: u64 = 12622824;
const INTERVALS: u64 = 19447080;
/// What the limiter lets a frame take when it is off: 1/256 s.
const UNLIMITED: f32 = 1.0 / 256.0;

/// 1 while the app is in the background (phones): the render thread stops
/// at the end of its frame, and with it the game. Android lets the app run
/// there; iOS runs it for a while before suspending it, failing its GPU
/// work meanwhile.
static BACKGROUND: AtomicU32 = AtomicU32::new(0);

/// The app went to the background, or came back.
pub fn set_background(on: bool) {
    BACKGROUND.store(on as u32, Ordering::Release);
    if !on {
        futex::notify(&BACKGROUND, u32::MAX);
    }
}

/// A frame rate the host holds the game to when the phone is hot (0: none;
/// see glue5-qos's `thermal`), on top of the engine's own limit.
static THERMAL_CAP: AtomicU32 = AtomicU32::new(0);

/// Holds frames to at most `fps` a second, slower than the engine's own
/// limiter if need be (0: lifts it). Takes effect at the next frame.
pub fn set_thermal_cap(fps: u32) {
    THERMAL_CAP.store(fps, Ordering::Relaxed);
}

/// Hook for `CSystem::EndRender()`.
pub(crate) unsafe extern "C" fn end_render(env: *mut Env) {
    while BACKGROUND.load(Ordering::Acquire) == 1 {
        futex::wait32(&BACKGROUND, 1, None);
    }
    let mem = unsafe { Mem::of(env) };
    crate::memory::report_heaps(mem);
    let last = mem.atomic_u64(LAST_END).swap(0, Ordering::AcqRel);
    unsafe { orig::end_render(env) };
    // The engine's computation, as it would have made it (f32 included).
    let now = mem.atomic_u64(LAST_END).load(Ordering::Acquire);
    if last == 0 || now < last {
        return;
    }
    let tick = f32::from_bits(mem.u32(TICK_SECONDS));
    let hz = (mem.u32(REFRESH) / mem.u32(REFRESH + 4).max(1)) as i32;
    let intervals = mem.i32(INTERVALS);
    let target = if hz > 0 && intervals > 0 { intervals as f32 / hz as f32 } else { UNLIMITED };
    // The thermal cap is only ever slower than the engine's own.
    let target = {
        let cap = THERMAL_CAP.load(Ordering::Relaxed);
        if cap > 0 { target.max(1.0 / cap as f32) } else { target }
    };
    let elapsed = (now - last) as f32 * tick;
    let wait = 0.95 * target - elapsed;
    if wait.is_nan() || wait <= 0.0 || tick.is_nan() || tick <= 0.0 {
        return;
    }
    let started = Instant::now();
    sleep_until(started + Duration::from_secs_f32(wait));
    let waited = started.elapsed().as_secs_f32() / tick;
    mem.atomic_u64(LAST_END).store(now + waited as u64, Ordering::Release);
}

/// Sleeps until `deadline`: half of what is left at a time, as the system's
/// sleeps can run a fifth long.
fn sleep_until(deadline: Instant) {
    while let Some(left) = deadline.checked_duration_since(Instant::now()).filter(|l| *l > Duration::from_micros(50)) {
        std::thread::sleep(left / 2);
    }
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.end_render = Some(end_render);
}
