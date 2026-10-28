//! Fading peds. An entity fading in or out (its alpha below 255: a ped
//! coming or going with the distance, a script's SET_ENTITY_ALPHA) is drawn
//! in the deferred fade pass, which forces the "deferredalphaclip"
//! technique group (stipple fades, without MSAA) or a "deferredsubsample*"
//! one (with MSAA). Each shader draws with its technique for that group,
//! and draws nothing when it has none: `grmShader::BeginDraw` gives no
//! passes. This game's `ped.fxc` has neither (its hair shaders have both),
//! so a fading ped was its hair alone.
//!
//! A shader with no technique for a fade group draws with its "deferred"
//! one instead, stippled as the fade pass wants it: the entity's alpha
//! (`CShaderLib::SetStippleAlpha`, before its draws) goes to the renderer
//! as `op::STIPPLE`, the share of each 4x4 block of pixels drawn, and back
//! to none after the draw. Shaders that have the group are unchanged.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use glue5_game::{Env, exports, orig};

use crate::gpu::op;
use crate::memory::Mem;

/// The technique group forced on every shader's draws (-1: none), as
/// `grmShader::SetForcedTechniqueGroupId` keeps it.
const FORCED_GROUP: u64 = 6634168;
/// The technique groups' ids, as `DeferredLighting::InitShaders` looks them
/// up: "deferred", then the fade pass's ("deferredalphaclip",
/// "deferredsubsamplealphaclip", "deferredsubsamplealpha").
const DEFERRED_GROUP: u64 = 21490232;
const FADE_GROUPS: [u64; 3] = [7248300, 7248304, 7248320];

/// The fading entity's alpha (0 to 255), as the fade pass last set it.
static ALPHA: AtomicU32 = AtomicU32::new(255);
/// A stipple was sent to the renderer for the draw in progress.
static STIPPLING: AtomicBool = AtomicBool::new(false);
static LOGGED: AtomicBool = AtomicBool::new(false);

/// Sends `op::STIPPLE`: `level` of 16 pixels drawn (0: all).
unsafe fn stipple(env: *mut Env, level: u32) {
    let m = unsafe { Mem::of(env) };
    let Ok(top) = (unsafe { exports::emscripten_stack_get_current(env) }) else { return };
    if let Ok(at) = unsafe { exports::_emscripten_stack_alloc(env, 16) } {
        m.set_u32(at, level);
        unsafe { orig::gpu_emit(env, op::STIPPLE, at, 1) };
    }
    let _ = unsafe { exports::_emscripten_stack_restore(env, top) };
}

/// Hook for `rage::grmShader::BeginDraw(eDrawType, bool, grcEffectTechnique) const`:
/// the passes to draw (0: none).
unsafe extern "C" fn shader_begin_draw(env: *mut Env, shader: u64, draw_type: u32, restore: u32, technique: u32) -> u32 {
    // A new shader must not inherit a previous fallback's stipple if an
    // engine path bypasses the EndDraw hook.
    if STIPPLING.swap(false, Ordering::Relaxed) {
        unsafe { stipple(env, 0) };
    }
    let passes = unsafe { orig::shader_begin_draw(env, shader, draw_type, restore, technique) };
    // An explicit technique, or one found: as it is.
    if passes != 0 || technique != 0 {
        return passes;
    }
    let m = unsafe { Mem::of(env) };
    let forced = m.i32(FORCED_GROUP);
    if forced < 0 || !FADE_GROUPS.iter().any(|&g| m.i32(g) == forced) {
        return passes;
    }
    m.set_i32(FORCED_GROUP, m.i32(DEFERRED_GROUP));
    let passes = unsafe { orig::shader_begin_draw(env, shader, draw_type, restore, technique) };
    m.set_i32(FORCED_GROUP, forced);
    if passes != 0 {
        // Of 16, rounded; 16 (or more) is all of them.
        let level = (ALPHA.load(Ordering::Relaxed).min(255) * 16 + 127) / 255;
        if level < 16 {
            unsafe { stipple(env, level.max(1)) };
            STIPPLING.store(true, Ordering::Relaxed);
        }
        if !LOGGED.swap(true, Ordering::Relaxed) {
            log::info!("fades: a shader with no fade technique draws with its deferred one, stippled (fading peds)");
        }
    }
    passes
}

/// Hook for `rage::grmShader::EndDraw() const` (`this` is gone from the
/// build: it ends the draw in progress): the stipple ends with the draw.
unsafe extern "C" fn shader_end_draw(env: *mut Env) {
    unsafe { orig::shader_end_draw(env) };
    if STIPPLING.swap(false, Ordering::Relaxed) {
        unsafe { stipple(env, 0) };
    }
}

/// Hook for `CShaderLib::SetStippleAlpha(unsigned int, bool)`.
unsafe extern "C" fn set_stipple_alpha(env: *mut Env, alpha: u32, invert: u32) {
    ALPHA.store(alpha, Ordering::Relaxed);
    unsafe { orig::set_stipple_alpha(env, alpha, invert) };
}

/// Hook for `CShaderLib::ResetStippleAlpha()`.
unsafe extern "C" fn reset_stipple_alpha(env: *mut Env) {
    ALPHA.store(255, Ordering::Relaxed);
    unsafe { orig::reset_stipple_alpha(env) };
    // This is the engine's explicit end of a fade scope, including paths
    // which bypass grmShader::EndDraw. Reset the host-side state as well.
    STIPPLING.store(false, Ordering::Relaxed);
    unsafe { stipple(env, 0) };
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.shader_begin_draw = Some(shader_begin_draw);
    hooks.shader_end_draw = Some(shader_end_draw);
    hooks.set_stipple_alpha = Some(set_stipple_alpha);
    hooks.reset_stipple_alpha = Some(reset_stipple_alpha);
}
