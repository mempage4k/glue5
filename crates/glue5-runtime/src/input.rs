//! Input, platform independent.
//!
//! - Keyboard and mouse go to the engine's input block, which the browser page
//!   used to fill (layout from index.html / browser_input_wasm.cpp).
//! - Gamepads (physical or virtual, e.g. touch controls) feed the engine's own
//!   pad code: the `pad_update` hook writes [`Gamepad`] into `rage::ioPad`
//!   after the original update, exactly where XInput would on PC.

use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use glue5_game::{Env, orig};

use crate::memory::Mem;

/// The frames' waits for the game's control update (on a thread of its
/// own, see audio_update.rs) since the last call: how many, how long in
/// all, the longest.
pub fn take_control_waits() -> (u64, Duration, Duration) {
    crate::audio_update::take_control_waits()
}

// ---- gamepads ----------------------------------------------------------------

/// Pad buttons, with RAGE's bit values.
pub mod button {
    pub const L2: u16 = 0x0001;
    pub const R2: u16 = 0x0002;
    pub const L1: u16 = 0x0004;
    pub const R1: u16 = 0x0008;
    /// Y / Triangle
    pub const NORTH: u16 = 0x0010;
    /// B / Circle
    pub const EAST: u16 = 0x0020;
    /// A / Cross
    pub const SOUTH: u16 = 0x0040;
    /// X / Square
    pub const WEST: u16 = 0x0080;
    /// Back / Select
    pub const SELECT: u16 = 0x0100;
    pub const L3: u16 = 0x0200;
    pub const R3: u16 = 0x0400;
    pub const START: u16 = 0x0800;
    pub const UP: u16 = 0x1000;
    pub const RIGHT: u16 = 0x2000;
    pub const DOWN: u16 = 0x4000;
    pub const LEFT: u16 = 0x8000;
}

/// State of one pad. Sticks are -1..1 with +y down (screen convention);
/// triggers 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gamepad {
    pub connected: bool,
    pub buttons: u16,
    pub left: [f32; 2],
    pub right: [f32; 2],
    pub l2: f32,
    pub r2: f32,
}

pub const MAX_PADS: usize = 4;

/// Rumble motor strengths the engine asked for, per pad: (low, high frequency).
static RUMBLE: Mutex<[(u16, u16); MAX_PADS]> = Mutex::new([(0, 0); MAX_PADS]);

/// The engine's current rumble request for pad `slot` (low, high frequency).
pub fn rumble(slot: usize) -> (u16, u16) {
    RUMBLE.lock().unwrap().get(slot).copied().unwrap_or_default()
}

static PADS: Mutex<[Gamepad; MAX_PADS]> =
    Mutex::new([Gamepad { connected: false, buttons: 0, left: [0.0; 2], right: [0.0; 2], l2: 0.0, r2: 0.0 }; MAX_PADS]);

/// Buttons pressed since the engine last read each pad. The engine reads
/// the pads once a frame; a press that begins and ends in between (a quick
/// tap on a touch button: START, a face button) would never reach it, so
/// it counts as held until the engine's next read.
static PRESSED: Mutex<[u16; MAX_PADS]> = Mutex::new([0; MAX_PADS]);

/// Buttons pressed and released since [`take_gamepad_changes`] last read
/// each pad: its reader (the mods, once a frame) sees a tap that began and
/// ended in between, as the engine does.
static CHANGES: Mutex<[(u16, u16); MAX_PADS]> = Mutex::new([(0, 0); MAX_PADS]);

/// Sets what pad `slot` reports to the engine from its next update on.
pub fn set_gamepad(slot: usize, pad: Gamepad) {
    if slot < MAX_PADS {
        let mut pads = PADS.lock().unwrap();
        let (down, up) = (pad.buttons & !pads[slot].buttons, pads[slot].buttons & !pad.buttons);
        PRESSED.lock().unwrap()[slot] |= down;
        let mut changes = CHANGES.lock().unwrap();
        changes[slot].0 |= down;
        changes[slot].1 |= up;
        pads[slot] = pad;
    }
}

/// Pad `slot`'s buttons: held now, and pressed and released since the last
/// call (for one reader besides the engine: the mods).
pub fn take_gamepad_changes(slot: usize) -> (u16, u16, u16) {
    if slot >= MAX_PADS {
        return (0, 0, 0);
    }
    let pads = PADS.lock().unwrap();
    let (pressed, released) = std::mem::take(&mut CHANGES.lock().unwrap()[slot]);
    (pads[slot].buttons, pressed, released)
}

// rage::ioPad (136 bytes per pad, 4 pads).
const PAD_INDEX: u64 = 8;
const PAD_BUTTONS: u64 = 12;
const PAD_AXES: u64 = 20; // LX, LY, RX, RY: one byte each, 0x80 = centre
const PAD_ANALOG: u64 = 24; // analog button values; [0] = L2, [1] = R2
const PAD_HAS_ANALOG: u64 = 55; // 1: CPad reads the analog trigger values
const PAD_MOTORS: u64 = 66; // u16 low, u16 high (ioPad::SetActuatorValue)

/// Hook for `rage::ioPad::Update(bool)`: the original runs first (it resets the
/// pad and merges the engine's own injected input), then the host's state is
/// merged on top.
///
/// (START and Back as any button: the engine's debug pad, which took them,
/// is off; see `Config::engine_args`.)
pub(crate) unsafe extern "C" fn pad_update(env: *mut Env, pad: u64) {
    unsafe { orig::pad_update(env, pad) };
    let mem = unsafe { Mem::of(env) };
    let slot = mem.u32(pad + PAD_INDEX) as usize;
    if let Some(r) = RUMBLE.lock().unwrap().get_mut(slot) {
        *r = (mem.u16(pad + PAD_MOTORS), mem.u16(pad + PAD_MOTORS + 2));
    }
    let Some(mut state) = PADS.lock().unwrap().get(slot).copied() else { return };
    state.buttons |= std::mem::take(&mut PRESSED.lock().unwrap()[slot]);
    if !state.connected {
        return;
    }
    mem.set_u32(pad + PAD_BUTTONS, mem.u32(pad + PAD_BUTTONS) | state.buttons as u32);
    let axis = |v: f32| ((v.clamp(-1.0, 1.0) + 1.0) * 127.5).round() as u8;
    for (i, v) in [state.left[0], state.left[1], state.right[0], state.right[1]].into_iter().enumerate() {
        // Keep the original value when the host reports a centred stick.
        if v.abs() > 1.0 / 128.0 {
            mem.set_u8(pad + PAD_AXES + i as u64, axis(v));
        }
    }
    let trigger = |v: f32, bit: u16| {
        if state.buttons & bit != 0 { 255 } else { (v.clamp(0.0, 1.0) * 255.0).round() as u8 }
    };
    mem.set_u8(pad + PAD_ANALOG, trigger(state.l2, button::L2));
    mem.set_u8(pad + PAD_ANALOG + 1, trigger(state.r2, button::R2));
    mem.set_u8(pad + PAD_HAS_ANALOG, 1);
}

// ---- web browser ---------------------------------------------------------------

/// The browser's input events (`SET_INPUT_EVENT`): a mouse click, accept.
const MOUSE_ACCEPT: f64 = 1001.0;
const ACCEPT: f64 = 16.0;

thread_local! {
    /// Whether this thread is building a `SET_INPUT_EVENT` call.
    static INPUT_EVENT: Cell<bool> = const { Cell::new(false) };
}

/// Hook for `CScaleformMgr::BeginMethod`, how scripts start a method call.
pub(crate) unsafe extern "C" fn scaleform_begin_method(env: *mut Env, movie: u32, base: u32, name: u64, index: u32) -> u32 {
    let mem = unsafe { Mem::of(env) };
    INPUT_EVENT.set(mem.bytes(name, 16) == b"SET_INPUT_EVENT\0");
    unsafe { orig::scaleform_begin_method(env, movie, base, name, index) }
}

/// Hook for `CScaleformMgr::AddParamFloat`. The phone's internet app sends a
/// left click as `SET_INPUT_EVENT(1001)`, which the browser movie of this
/// game data does not handle: links opened only with Enter, accept (16),
/// which opens what the pointer is on. A click is sent as accept.
pub(crate) unsafe extern "C" fn scaleform_add_param_float(env: *mut Env, value: f64) {
    let value = if INPUT_EVENT.get() && value == MOUSE_ACCEPT { ACCEPT } else { value };
    unsafe { orig::scaleform_add_param_float(env, value) }
}

// ---- text entry ----------------------------------------------------------------

static TEXT_BOX: AtomicBool = AtomicBool::new(false);
/// The open text box's `rage::ioVirtualKeyboard::eTextType`.
static TEXT_BOX_TYPE: AtomicU32 = AtomicU32::new(0);

/// What the game's text box takes, as it asks the platform's keyboard for it
/// (`rage::ioVirtualKeyboard::eTextType`; the box itself keeps the
/// characters each allows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextType {
    Text,
    Email,
    Password,
    /// Digits: the phone's keypad (cheats are numbers dialled on it).
    Numeric,
    Alphabet,
    /// A player's name.
    Gamertag,
    /// A save's or a clip's name.
    Filename,
}

/// What the game's text box takes, while it is open (it wants typed text):
/// touch devices show the system keyboard meanwhile.
pub fn text_box() -> Option<TextType> {
    TEXT_BOX.load(Ordering::Relaxed).then(|| match TEXT_BOX_TYPE.load(Ordering::Relaxed) {
        1 => TextType::Email,
        2 => TextType::Password,
        3 => TextType::Numeric,
        4 => TextType::Alphabet,
        5 => TextType::Gamertag,
        6 => TextType::Filename,
        _ => TextType::Text,
    })
}

/// Hook for `CTextInputBox::Open`: its parameters start with the text type.
pub(crate) unsafe extern "C" fn text_box_open_hook(env: *mut Env, this: u64, params: u64) {
    unsafe { orig::text_box_open(env, this, params) };
    TEXT_BOX_TYPE.store(unsafe { Mem::of(env) }.u32(params), Ordering::Relaxed);
    TEXT_BOX.store(true, Ordering::Relaxed);
}

/// Hook for `CTextInputBox::RemoveTextInputBoxMovie`: the text box closes
/// (accepted, cancelled or closed by the game).
pub(crate) unsafe extern "C" fn text_box_close_hook(env: *mut Env, this: u64) {
    unsafe { orig::text_box_close(env, this) };
    TEXT_BOX.store(false, Ordering::Relaxed);
}

// ---- touch play ------------------------------------------------------------------

/// The device the engine saw used last (an `ioSource`'s device index): a
/// pad's index, or -4 for keyboard and mouse. It picks the prompts' icons,
/// aim assist, the menus' behaviour.
const LAST_DEVICE: u64 = 24627136;
const KEYBOARD_MOUSE: i32 = -4;
/// The keyboard and mouse scheme a script loaded for its minigame (golf,
/// darts, yoga, the arcade's: `INIT_PC_SCRIPTED_CONTROLS`), its name's
/// hash: "default"'s when none (`SHUTDOWN_PC_SCRIPTED_CONTROLS`).
const PC_SCHEME: u64 = 24627144;
/// joaat("default").
const DEFAULT_SCHEME: u32 = 3839837909;
/// A task's ped, a ped's position.
const TASK_PED: u64 = 16;
const PED_POSITION: u64 = 112;
/// A vehicle's type (`VEHICLE_TYPE_*`), and a bicycle's.
const VEHICLE_TYPE: u64 = 3088;
const BICYCLE: i32 = 12;
/// The player's surroundings are scanned for a vehicle to enter every this
/// many frames on foot.
const SCAN_EVERY: u32 = 6;
/// The engine's per-frame signals below count as now for this long.
const FRESH: Duration = Duration::from_millis(250);

static TOUCH_PLAY: AtomicBool = AtomicBool::new(false);
/// A script's instructional buttons' bar shows: its menu or scene asks for
/// buttons.
static BAR: AtomicBool = AtomicBool::new(false);
/// When a script last showed its menu's cursor.
static CURSOR_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// A screen of the game's own shows its pointer (the browser, a hacking
/// screen: their movies draw it).
static POINTER: AtomicBool = AtomicBool::new(false);
/// The pause menu shows its map.
static MAP: AtomicBool = AtomicBool::new(false);
/// A warning of the engine's asks something; the screen is black or
/// loading.
static ALERT: AtomicBool = AtomicBool::new(false);
static HIDDEN: AtomicBool = AtomicBool::new(false);
static SIGNALS: Mutex<Signals> = Mutex::new(Signals {
    on_foot: None,
    in_vehicle: None,
    driving: None,
    parachute: None,
    cutscene: None,
    activity: Activity::OnFoot,
});
static VEHICLE_NEAR: AtomicBool = AtomicBool::new(false);
static SCAN_FRAME: AtomicU32 = AtomicU32::new(0);
static PHONE_OUT: AtomicBool = AtomicBool::new(false);

/// `PHONE_OUT` with hysteresis. The raw bytes dip while the phone remakes
/// itself (its opening animation runs with `PHONE_GOING` set, and switching
/// the input device between pad and keyboard/mouse — which this module does
/// *because* the phone is out — can make the game rebuild the phone). Feeding
/// those dips straight through made the published state, the device and the
/// touch layout flip back and forth, and the phone cycled open/closed on its
/// own after a single tap until the player backed out of it. Coming out is
/// believed quickly; going away must hold, so a dip during the phone's own
/// remake never flips anything back.
#[derive(Clone, Copy)]
struct PhoneOut {
    published: bool,
    flip_since: Option<Instant>,
}

impl PhoneOut {
    const fn new() -> Self {
        Self { published: false, flip_since: None }
    }

    fn update(&mut self, raw: bool, now: Instant) -> bool {
        if raw == self.published {
            self.flip_since = None;
            return self.published;
        }
        let started = *self.flip_since.get_or_insert(now);
        let wait = if raw { Duration::from_millis(60) } else { Duration::from_millis(600) };
        if now.saturating_duration_since(started) >= wait {
            self.published = raw;
            self.flip_since = None;
        }
        self.published
    }
}

static PHONE_STATE: Mutex<PhoneOut> = Mutex::new(PhoneOut::new());

/// While the phone is out, whether it is driven as a touchscreen (the input
/// device switches to keyboard/mouse so taps on the phone's screen land) or
/// left on the pad (D-pad and buttons). A settings switch: if the phone
/// cycles open/closed on a build, this tells the device switch apart from
/// the game cycling on its own.
static PHONE_TOUCHSCREEN: AtomicBool = AtomicBool::new(true);

/// The phone's raw bytes, the state published from them and the device
/// written, whenever any of them changes: for the support report, so the
/// phone opening/closing on its own can be told from input this app sent.
/// Bounded; no coordinates or contents.
static PHONE_TRACE: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static PHONE_TRACE_LAST: Mutex<Option<(bool, bool, bool, bool, bool, bool, i32)>> = Mutex::new(None);
static PHONE_TRACE_START: OnceLock<Instant> = OnceLock::new();

fn trace_phone(created: bool, camera: bool, going: bool, raw: bool, published: bool, mouse: bool, z: f32) {
    // The phone's depth in half-steps: steady while it shows, so a phone
    // jumping nearer and farther (its enlarged place against its own)
    // stands out as lines of their own.
    let state = (created, camera, going, raw, published, mouse, (z * 2.0) as i32);
    let mut last = PHONE_TRACE_LAST.lock().unwrap();
    if *last == Some(state) {
        return;
    }
    *last = Some(state);
    drop(last);
    let start = PHONE_TRACE_START.get_or_init(Instant::now);
    let mut trace = PHONE_TRACE.lock().unwrap();
    if trace.len() == 64 {
        trace.pop_front();
    }
    trace.push_back(format!(
        "+{}ms phone created={created} camera={camera} going={going} raw_out={raw} out={published} device={} pos_z={z:.1}",
        start.elapsed().as_millis(),
        if mouse { "keyboard/mouse" } else { "pad" },
    ));
}

/// The recent phone/device transitions, oldest first.
pub fn diagnostics() -> Vec<String> {
    PHONE_TRACE.lock().unwrap().iter().cloned().collect()
}
static PAUSE_MENU: AtomicBool = AtomicBool::new(false);
/// The pause menu's frontend is up, with a script's menu in it or not.
static FRONTEND: AtomicBool = AtomicBool::new(false);
/// A minigame's keyboard and mouse scheme is loaded.
static MINIGAME: AtomicBool = AtomicBool::new(false);
static SCRIPTED: AtomicBool = AtomicBool::new(false);
static CHARACTER: AtomicU32 = AtomicU32::new(Character::Other as u32);
static GUN: AtomicBool = AtomicBool::new(false);

/// A ped's weapons (`CPedWeaponManager`), and the one in hand's hash.
const PED_WEAPONS: u64 = 4344;
const WEAPON_SELECTED: u64 = 24;
/// What the player can hold that does not reload (their B is a blow): bare
/// hands, melee weapons, thrown ones, cans.
const NO_RELOAD: &[&str] = &[
    "WEAPON_UNARMED", "WEAPON_KNIFE", "WEAPON_NIGHTSTICK", "WEAPON_HAMMER", "WEAPON_BAT", "WEAPON_GOLFCLUB",
    "WEAPON_CROWBAR", "WEAPON_BOTTLE", "WEAPON_DAGGER", "WEAPON_HATCHET", "WEAPON_KNUCKLE", "WEAPON_MACHETE",
    "WEAPON_FLASHLIGHT", "WEAPON_SWITCHBLADE", "WEAPON_POOLCUE", "WEAPON_WRENCH", "WEAPON_BATTLEAXE",
    "WEAPON_STONE_HATCHET", "WEAPON_CANDYCANE", "WEAPON_GRENADE", "WEAPON_STICKYBOMB", "WEAPON_PROXMINE",
    "WEAPON_BZGAS", "WEAPON_SMOKEGRENADE", "WEAPON_MOLOTOV", "WEAPON_PIPEBOMB", "WEAPON_SNOWBALL", "WEAPON_BALL",
    "WEAPON_FLARE", "WEAPON_ACIDPACKAGE", "WEAPON_PETROLCAN", "WEAPON_HAZARDCAN", "WEAPON_FERTILIZERCAN",
    "WEAPON_FIREEXTINGUISHER", "GADGET_PARACHUTE",
];

/// A ped's archetype (its model), and the model's name hash.
const PED_ARCHETYPE: u64 = 32;
const ARCHETYPE_HASH: u64 = 24;
/// The story's characters' models (`player_zero`, `_one`, `_two`).
const MICHAEL: u32 = 0x0D71_14C9;
const FRANKLIN: u32 = 0x9B22_DBAF;
const TREVOR: u32 = 0x9B81_0FA2;

/// `CPhoneMgr`'s state (bytes, bit 0): the phone created (the scripts'
/// `CREATE_MOBILE_PHONE`; the code deletes it too, on a load or a new
/// session), being moved off the screen, its camera on.
const PHONE_CREATED: u64 = 19505653;
const PHONE_GOING: u64 = 19505660;
const PHONE_CAMERA: u64 = 19505840;
/// Where the phone is (`SET_MOBILE_PHONE_POSITION`): x, y, z in front of
/// its camera (z negative).
const PHONE_POSITION: u64 = 19505776;
static PHONE_AT: Mutex<[f32; 3]> = Mutex::new([0.0; 3]);
/// On touchscreens, bring the phone 20% closer so it appears 25% larger.
const TOUCH_PHONE_SCALE: f32 = 1.25;
#[derive(Clone, Copy)]
struct PhonePositionOverride {
    original: [u32; 3],
    enlarged: [u32; 3],
}
/// The game may rewrite the phone position every frame; remember our last
/// write so it can be scaled from the original and restored when touch ends.
static PHONE_POSITION_OVERRIDE: Mutex<Option<PhonePositionOverride>> = Mutex::new(None);

/// Where the phone shows on the screen while it is up, as fractions of the
/// screen (left, top, right, bottom; its bottom may be below it), for a
/// screen this wide over high: its position seen through the phone's 70°
/// lens, across as at 16:9 whatever the screen's shape (measured on a
/// phone's 19.5:9 and an iPad's 1.43), and its size there (measured).
pub fn phone_rect(aspect: f32) -> Option<[f32; 4]> {
    let [x, y, z] = *PHONE_AT.lock().unwrap();
    if !PHONE_OUT.load(Ordering::Relaxed) || z >= -1.0 {
        return None;
    }
    let depth = -z * 35f32.to_radians().tan();
    let (u, v) = (0.5 + 0.5 * x / (depth * 16.0 / 9.0), 0.5 - 0.5 * y / depth);
    // At z -113: 0.24 of the screen's height wide, 0.48 high.
    let near = 113.0 / -z;
    let (w, h) = (0.12 * near / aspect, 0.2417 * near);
    Some([u - w, v - h, u + w, v + h])
}

/// Scale the phone toward the camera while keeping its projected center.
fn enlarge_phone_position(position: [f32; 3]) -> [f32; 3] {
    let inverse = 1.0 / TOUCH_PHONE_SCALE;
    [position[0] * inverse, position[1] * inverse, position[2] * inverse]
}

/// Read the game's phone location, optionally enlarging it for active touch
/// play. Restore the game's unmodified values when touch stops or the phone
/// is put away; rebase if the game itself changes the location meanwhile.
fn update_phone_position(mem: Mem, touch: bool, phone_out: bool) -> [f32; 3] {
    let addresses = [PHONE_POSITION, PHONE_POSITION + 4, PHONE_POSITION + 8];
    let current = addresses.map(|at| mem.u32(at));
    let mut previous = PHONE_POSITION_OVERRIDE.lock().unwrap();
    let original = match *previous {
        Some(override_) if current == override_.enlarged => override_.original,
        _ => current,
    };
    let position = original.map(f32::from_bits);
    if touch && phone_out && position[2] < -1.0 {
        let enlarged = enlarge_phone_position(position).map(f32::to_bits);
        for (at, bits) in addresses.into_iter().zip(enlarged) {
            mem.set_u32(at, bits);
        }
        *previous = Some(PhonePositionOverride { original, enlarged });
        enlarged.map(f32::from_bits)
    } else {
        if let Some(override_) = *previous
            && current == override_.enlarged
        {
            for (at, bits) in addresses.into_iter().zip(override_.original) {
                mem.set_u32(at, bits);
            }
        }
        *previous = None;
        position
    }
}

/// `CPlayerInfo`'s reasons the player's controls are off (bits: a script's
/// `SET_PLAYER_CONTROL`, an ambient script's, the pause menu's, a
/// cutscene's...): `IS_PLAYER_CONTROL_ON` is false while any is set.
const PLAYER_CONTROL_OFF: u64 = 936;
const OFF_BY_PAUSE_MENU: u32 = 0x400;
/// A script's `SET_PLAYER_CONTROL` (the mission's or an ambient one's).
const OFF_BY_SCRIPT: u32 = 0x20 | 0x800;

/// When the engine last ran each of the player's tasks.
struct Signals {
    on_foot: Option<Instant>,
    in_vehicle: Option<Instant>,
    driving: Option<(Driving, Instant)>,
    parachute: Option<Instant>,
    cutscene: Option<Instant>,
    /// The activity last seen, kept while none is (a ragdoll, a scripted
    /// scene, the moment between two tasks).
    activity: Activity,
}

/// The touch controls are in use: the camera turns with the mouse (a
/// finger's drag) while the engine keeps playing as with a pad; the menus
/// made for the mouse see keyboard and mouse, and say when they take it
/// ([`PlayerContext::cursor`]): fingers act as it there.
pub fn set_touch_play(on: bool) {
    TOUCH_PLAY.store(on, Ordering::Relaxed);
}

/// The phone, while out, is driven by taps on its own screen (the input
/// device switches to keyboard/mouse for it) rather than by the pad.
pub fn set_phone_touchscreen(on: bool) {
    PHONE_TOUCHSCREEN.store(on, Ordering::Relaxed);
}

/// The phone, while out on touchscreens, is brought nearer so it shows
/// bigger (its place in the game's memory is rewritten each frame). A
/// settings switch: off, the game's own place stands.
static PHONE_ENLARGE: AtomicBool = AtomicBool::new(true);

pub fn set_phone_enlarge(on: bool) {
    PHONE_ENLARGE.store(on, Ordering::Relaxed);
}

/// The pause menu's Targeting Mode (`PREF_TARGET_CONFIG`, in its array of
/// u32 preferences): the aim assist with a pad. Keyboard and mouse always
/// aim freely.
const PREF_TARGETING: u64 = 19566112 + 4 * 3;
/// The aim assist during touch play (a [`Targeting`]), `NO_TARGETING` until
/// the app sets it.
static TOUCH_TARGETING: AtomicU32 = AtomicU32::new(NO_TARGETING);
const NO_TARGETING: u32 = u32::MAX;
/// The menu's Targeting Mode while touch play's stands in for it.
static MENU_TARGETING: Mutex<Option<i32>> = Mutex::new(None);

/// The game's targeting modes (its menu's names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Targeting {
    /// Assisted Aim - Full: aiming locks on to the nearest target and
    /// follows it; the right stick switches between targets.
    Full = 0,
    /// Assisted Aim - Partial: aiming snaps to the target, then the right
    /// stick aims freely around it.
    Partial = 1,
    /// Free Aim - Assisted: the camera slows down over targets.
    FreeAssisted = 2,
    /// Free Aim.
    Free = 3,
}

/// The aim assist during touch play, in place of the menu's (shown and
/// kept there) out of the pause menu.
pub fn set_touch_targeting(targeting: Targeting) {
    TOUCH_TARGETING.store(targeting as u32, Ordering::Relaxed);
}

/// A player ped's `CPlayerInfo`, its targeting (`CPlayerPedTargeting`), the
/// target that is locked on to.
const PED_PLAYER_INFO: u64 = 4328;
const PLAYER_TARGETING: u64 = 1024;
const TARGETING_LOCK_ON: u64 = 48;
static AIM_LOCKED: AtomicBool = AtomicBool::new(false);

/// The player's aim is locked on to a target (the assisted targeting modes,
/// aiming): the camera follows it, the right stick switches targets.
pub fn aim_locked() -> bool {
    AIM_LOCKED.load(Ordering::Relaxed)
}

/// Whether the player's aim is locked on now.
fn update_aim_locked(env: *mut Env, mem: Mem) {
    let ped = unsafe { orig::find_player_ped(env) };
    let info = if ped != 0 { mem.u64(ped + PED_PLAYER_INFO) } else { 0 };
    let locked = info != 0 && mem.u64(info + PLAYER_TARGETING + TARGETING_LOCK_ON) != 0;
    AIM_LOCKED.store(locked, Ordering::Relaxed);
}

/// Puts touch play's targeting mode in place of the menu's while it plays,
/// the menu's back otherwise (the pause menu shows and saves it).
fn update_targeting(mem: Mem, touch_play: bool) {
    let touch = TOUCH_TARGETING.load(Ordering::Relaxed);
    let mut menu = MENU_TARGETING.lock().unwrap();
    if touch_play && touch != NO_TARGETING {
        menu.get_or_insert_with(|| mem.i32(PREF_TARGETING));
        mem.set_i32(PREF_TARGETING, touch as i32);
    } else if let Some(mode) = menu.take() {
        mem.set_i32(PREF_TARGETING, mode);
    }
}

/// What the player is doing, for the touch controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerContext {
    pub activity: Activity,
    /// On foot, next to a vehicle they can get into.
    pub vehicle_near: bool,
    /// The phone is out.
    pub phone: bool,
    /// The pause menu (the map, the store, ...) is open.
    pub menu: bool,
    /// A cutscene plays that can be skipped now.
    pub cutscene: bool,
    /// A script has the player's controls (a menu of its own, the character
    /// creator, a shop, a minigame, a scene, a mission's end).
    pub scripted: bool,
    /// Who the player is (the HUD takes their colour).
    pub character: Character,
    /// The player holds a gun: B reloads (else it strikes).
    pub gun: bool,
    /// A script's instructional buttons' bar shows (its menu or scene asks
    /// for buttons).
    pub bar: bool,
    /// A menu takes the mouse (the pause menu's, or a script's that shows
    /// its cursor).
    pub cursor: bool,
    /// The pause menu shows its map.
    pub map: bool,
    /// A screen of the game's own takes the pointer (the browser, a
    /// hacking screen): it is all the screen.
    pub pointer: bool,
    /// A warning of the engine's is up, over whatever plays (join another
    /// session? quit?): it asks for its own buttons.
    pub alert: bool,
    /// The screen is black or a loading screen shows: nothing to play.
    pub hidden: bool,
}

/// The story's characters, or anyone else (GTA Online's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Character {
    Michael,
    Franklin,
    Trevor,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    OnFoot,
    Driving(Driving),
    /// In a vehicle, not driving it.
    Passenger,
    /// Skydiving or under a parachute.
    Parachute,
}

/// The kinds of vehicles driven alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driving {
    /// Cars, motorbikes, quads, boats: steered, a throttle and a brake.
    Car,
    /// Pedalled.
    Bicycle,
    /// Helicopters, blimps, autogyros: up and down, yaw.
    Heli,
    /// A throttle, yaw, landing gear.
    Plane,
    /// Up and down, a throttle.
    Submarine,
}

pub fn player_context() -> PlayerContext {
    let mut signals = SIGNALS.lock().unwrap();
    let fresh = |t: Option<Instant>| t.is_some_and(|t| t.elapsed() < FRESH);
    // The most particular of what runs now.
    let now = match signals.driving {
        Some((driving, t)) if fresh(Some(t)) => Some(Activity::Driving(driving)),
        _ if fresh(signals.parachute) => Some(Activity::Parachute),
        _ if fresh(signals.in_vehicle) => Some(Activity::Passenger),
        _ if fresh(signals.on_foot) => Some(Activity::OnFoot),
        _ => None,
    };
    if let Some(activity) = now {
        signals.activity = activity;
    }
    PlayerContext {
        activity: signals.activity,
        vehicle_near: signals.activity == Activity::OnFoot && VEHICLE_NEAR.load(Ordering::Relaxed),
        phone: PHONE_OUT.load(Ordering::Relaxed),
        menu: PAUSE_MENU.load(Ordering::Relaxed),
        cutscene: fresh(signals.cutscene),
        scripted: SCRIPTED.load(Ordering::Relaxed),
        character: match CHARACTER.load(Ordering::Relaxed) {
            0 => Character::Michael,
            1 => Character::Franklin,
            2 => Character::Trevor,
            _ => Character::Other,
        },
        gun: GUN.load(Ordering::Relaxed),
        bar: BAR.load(Ordering::Relaxed),
        cursor: TOUCH_PLAY.load(Ordering::Relaxed)
            && (FRONTEND.load(Ordering::Relaxed)
                || (!MINIGAME.load(Ordering::Relaxed)
                    && (fresh(*CURSOR_AT.lock().unwrap()) || POINTER.load(Ordering::Relaxed)))),
        pointer: TOUCH_PLAY.load(Ordering::Relaxed) && !MINIGAME.load(Ordering::Relaxed) && POINTER.load(Ordering::Relaxed),
        map: MAP.load(Ordering::Relaxed),
        alert: ALERT.load(Ordering::Relaxed),
        hidden: HIDDEN.load(Ordering::Relaxed),
    }
}

/// When the pause menu last went a level deeper (into a column or a list).
static DEEPER_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// When the pause menu last went a level deeper: a click there that went
/// into a column picks nothing yet (the mouse's hover picks in the column
/// it is in).
pub fn menu_went_deeper() -> Option<Instant> {
    *DEEPER_AT.lock().unwrap()
}

/// Hook for `CPauseMenu::MENU_SHIFT_DEPTH`: its direction is 1 deeper, -1
/// back.
pub(crate) unsafe extern "C" fn menu_shift_depth(env: *mut Env, dir: u32, a: u32, b: u32) {
    unsafe { orig::menu_shift_depth(env, dir, a, b) };
    if dir == 1 {
        *DEEPER_AT.lock().unwrap() = Some(Instant::now());
    }
}

/// Hook for `SET_MOUSE_CURSOR_THIS_FRAME`: a script's menu takes the mouse.
pub(crate) unsafe extern "C" fn set_mouse_cursor_this_frame(env: *mut Env, info: u64) {
    unsafe { orig::set_mouse_cursor_this_frame(env, info) };
    *CURSOR_AT.lock().unwrap() = Some(Instant::now());
}

/// Hook for `CControl::Update`: during touch play out of the pause menu, the
/// engine goes on seeing the pad as the device in use, though the camera
/// reads the mouse.
pub(crate) unsafe extern "C" fn control_update(env: *mut Env, this: u64) {
    unsafe { orig::control_update(env, this) };
    // Only the player's: with the debug control on, the rest ran twice a
    // frame.
    if this != crate::prompts::CONTROL {
        return;
    }
    let mem = unsafe { Mem::of(env) };
    let ped = unsafe { orig::find_player_ped(env) };
    let info = if ped != 0 { mem.u64(ped + PED_PLAYER_INFO) } else { 0 };
    let off = if info != 0 { mem.u32(info + PLAYER_CONTROL_OFF) } else { 0 };
    // Every part of the pause menu (the store, Social Club, the video
    // editor), unless a script runs its own menu in it (GTA Online's
    // character creator) or over it: it took the player's controls, or
    // shows its own buttons.
    let frontend = unsafe { orig::pause_menu_is_active(env, 0) } != 0;
    let bar = crate::prompts::script_bar_shown(env);
    let menu = frontend && off & OFF_BY_SCRIPT == 0 && !bar;
    PAUSE_MENU.store(menu, Ordering::Relaxed);
    BAR.store(bar, Ordering::Relaxed);
    FRONTEND.store(frontend, Ordering::Relaxed);
    MAP.store(frontend && unsafe { orig::pause_menu_in_map(env) } != 0, Ordering::Relaxed);
    let scripted = off & !OFF_BY_PAUSE_MENU != 0;
    let scheme = mem.u32(PC_SCHEME);
    let minigame = scheme != 0 && scheme != DEFAULT_SCHEME;
    MINIGAME.store(minigame, Ordering::Relaxed);
    let pointer = crate::prompts::pointer_shown(env);
    POINTER.store(pointer, Ordering::Relaxed);
    let byte = |at: u64| mem.u8(at) & 1 != 0;
    let phone_created = byte(PHONE_CREATED);
    let phone_camera = byte(PHONE_CAMERA);
    let phone_going = byte(PHONE_GOING);
    let raw_phone_out = (phone_created || phone_camera) && !phone_going;
    let phone_out = PHONE_STATE.lock().unwrap().update(raw_phone_out, Instant::now());
    PHONE_OUT.store(phone_out, Ordering::Relaxed);
    let touch_play = TOUCH_PLAY.load(Ordering::Relaxed) && !menu;
    if touch_play {
        // A script's menu or scene (its buttons' bar, the player's controls
        // taken), the pause menu, a cursor or pointer shown: as with
        // keyboard and mouse, so that the screens made for the mouse show
        // theirs. The touch controls' pad works there too. Not in a
        // minigame that loaded its own keyboard and mouse scheme (its
        // controls change with it): the pad, as made for touch.
        let fresh = |t: Option<Instant>| t.is_some_and(|t| t.elapsed() < FRESH);
        // The phone's app screen now receives direct touch taps through the
        // mouse input block, so keep its mouse-driven Scaleform controls
        // active while the phone is out during touch play too — unless the
        // phone is being driven by the pad instead (a settings switch).
        let phone_screen = phone_out && PHONE_TOUCHSCREEN.load(Ordering::Relaxed);
        let mouse = !minigame && (frontend || bar || scripted || pointer || fresh(*CURSOR_AT.lock().unwrap()) || phone_screen);
        mem.set_i32(LAST_DEVICE, if mouse { KEYBOARD_MOUSE } else { 0 });
        trace_phone(phone_created, phone_camera, phone_going, raw_phone_out, phone_out, mouse, PHONE_AT.lock().unwrap()[2]);
    }
    update_targeting(mem, touch_play);
    update_aim_locked(env, mem);
    // The phone is enlarged only while touch controls are active (and the
    // enlargement is on). The same transformed position drives the touch
    // rectangle, keeping it aligned.
    let enlarge = touch_play && PHONE_ENLARGE.load(Ordering::Relaxed);
    *PHONE_AT.lock().unwrap() = update_phone_position(mem, enlarge, phone_out);
    SCRIPTED.store(scripted, Ordering::Relaxed);
    let model = if ped != 0 { mem.u64(ped + PED_ARCHETYPE) } else { 0 };
    let character = match if model != 0 { mem.u32(model + ARCHETYPE_HASH) } else { 0 } {
        MICHAEL => Character::Michael,
        FRANKLIN => Character::Franklin,
        TREVOR => Character::Trevor,
        _ => Character::Other,
    };
    CHARACTER.store(character as u32, Ordering::Relaxed);
    static NO_RELOAD_HASHES: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
    let no_reload = NO_RELOAD_HASHES.get_or_init(|| NO_RELOAD.iter().map(|w| crate::texts::label_hash(w)).collect());
    let weapons = if ped != 0 { mem.u64(ped + PED_WEAPONS) } else { 0 };
    let weapon = if weapons != 0 { mem.u32(weapons + WEAPON_SELECTED) } else { 0 };
    GUN.store(weapon != 0 && !no_reload.contains(&weapon), Ordering::Relaxed);
    let warning = unsafe { orig::warning_screen_active(env) } != 0;
    ALERT.store(warning, Ordering::Relaxed);
    let hidden = unsafe { orig::screen_faded_out(env) != 0 || orig::loading_screens_active(env) != 0 };
    HIDDEN.store(hidden, Ordering::Relaxed);
    let code_bar = warning || text_box().is_some() || frontend;
    unsafe { crate::prompts::update(env, TOUCH_PLAY.load(Ordering::Relaxed), code_bar) };
}

/// Hook for `CTaskPlayerOnFoot::ProcessPreFSM`: the player is on foot.
pub(crate) unsafe extern "C" fn player_on_foot(env: *mut Env, task: u64) -> u32 {
    let result = unsafe { orig::player_on_foot(env, task) };
    SIGNALS.lock().unwrap().on_foot = Some(Instant::now());
    if TOUCH_PLAY.load(Ordering::Relaxed) && SCAN_FRAME.fetch_add(1, Ordering::Relaxed).is_multiple_of(SCAN_EVERY) {
        let ped = unsafe { Mem::of(env) }.u64(task + TASK_PED);
        if ped != 0 {
            // As the Enter button would (CTaskPlayerOnFoot::CheckShouldEnterVehicle).
            let vehicle = unsafe { orig::scan_for_vehicle_to_enter(env, ped, ped + PED_POSITION, 0) };
            VEHICLE_NEAR.store(vehicle != 0, Ordering::Relaxed);
        }
    }
    result
}

/// Hook for `CTaskPlayerDrive::ProcessPreFSM`: the player is in a vehicle.
pub(crate) unsafe extern "C" fn player_in_vehicle(env: *mut Env, task: u64) -> u32 {
    let result = unsafe { orig::player_in_vehicle(env, task) };
    SIGNALS.lock().unwrap().in_vehicle = Some(Instant::now());
    result
}

/// Hook for `CTaskParachute::UpdateFSM`, any ped's: the player's skydives
/// or is under a parachute.
pub(crate) unsafe extern "C" fn parachute_update(env: *mut Env, task: u64, state: u32, event: u32) -> u32 {
    let result = unsafe { orig::parachute_update(env, task, state, event) };
    let ped = unsafe { Mem::of(env) }.u64(task + TASK_PED);
    if ped != 0 && ped == unsafe { orig::find_player_ped(env) } {
        SIGNALS.lock().unwrap().parachute = Some(Instant::now());
    }
    result
}

/// `CutSceneManager`'s state: whether its script lets it be skipped
/// (`SET_CUTSCENE_CAN_BE_SKIPPED`), the time it plays at and after which
/// its data forbids a skip (where it blends into the game; negative: none),
/// a skip under way, and in a network game whether the scripts allow one.
const CUT_CAN_SKIP: u64 = 5914;
const CUT_TIME: u64 = 3056;
const CUT_NO_SKIP_AFTER: u64 = 5916;
const CUT_SKIPPING: u64 = 5899;
const CUT_NETWORK_CAN_SKIP: u64 = 5920;
/// A network game is on (byte).
const NETWORK_GAME: u64 = 28501281;

/// Hook for `CutSceneManager::UpdateSkip`: a cutscene plays past its first
/// 2 s, not fading out yet. It counts if the press would skip it now, as
/// `TriggerCutsceneSkip` decides (a created phone, for one, blocks it).
pub(crate) unsafe extern "C" fn cutscene_update_skip(env: *mut Env, this: u64) {
    unsafe { orig::cutscene_update_skip(env, this) };
    let mem = unsafe { Mem::of(env) };
    let f32_at = |at: u64| f32::from_bits(mem.u32(at));
    let blocked = f32_at(this + CUT_NO_SKIP_AFTER) >= 0.0 && f32_at(this + CUT_TIME) >= f32_at(this + CUT_NO_SKIP_AFTER);
    let network = mem.u8(NETWORK_GAME) == 1 && mem.u8(this + CUT_NETWORK_CAN_SKIP) == 0;
    let skippable = mem.u8(this + CUT_CAN_SKIP) == 1
        && !blocked
        && mem.u8(PHONE_CREATED) & 1 == 0
        && mem.u8(this + CUT_SKIPPING) == 0
        && !network;
    if skippable {
        SIGNALS.lock().unwrap().cutscene = Some(Instant::now());
    }
}

/// The player drives `vehicle`, a vehicle driven as `kind`.
fn driving(env: *mut Env, vehicle: u64, kind: Driving) {
    let bicycle = vehicle != 0 && unsafe { Mem::of(env) }.i32(vehicle + VEHICLE_TYPE) == BICYCLE;
    SIGNALS.lock().unwrap().driving = Some((if bicycle { Driving::Bicycle } else { kind }, Instant::now()));
}

/// Hooks for the player's driving tasks (`ProcessDriverInputsForPlayerOnUpdate`,
/// once a frame while the player drives).
macro_rules! drive_hooks {
    ($($name:ident: $kind:ident),* $(,)?) => {$(
        pub(crate) unsafe extern "C" fn $name(env: *mut Env, task: u64, ped: u64, control: u64, vehicle: u64) {
            unsafe { orig::$name(env, task, ped, control, vehicle) };
            driving(env, vehicle, Driving::$kind);
        }
    )*};
}

drive_hooks! {
    drive_automobile: Car,
    drive_bike: Car,
    drive_boat: Car,
    drive_heli: Heli,
    drive_autogyro: Heli,
    drive_plane: Plane,
    drive_submarine: Submarine,
}

// ---- keyboard & mouse --------------------------------------------------------

// The input block.
const KEYS: u64 = 0; // u8[256] by Windows virtual-key code, 0x80 = down
const MOUSE_X: u64 = 256;
const MOUSE_Y: u64 = 260;
const MOUSE_DX: u64 = 264; // accumulated; the engine swaps them with 0
const MOUSE_DY: u64 = 268;
const MOUSE_DZ: u64 = 272; // wheel notches, + = up
const MOUSE_BUTTONS: u64 = 276;
const FOCUS: u64 = 280;
const LOCKED: u64 = 284;
const TEXT_HEAD: u64 = 288;
const TEXT_TAIL: u64 = 292;
const TEXT: u64 = 296; // u16[64] ring of UTF-16 units
const TEXT_LEN: u32 = 64;
const WIN_W: u64 = 432;
const WIN_H: u64 = 436;

/// `CMousePointer`'s "shown last frame" in this build: what its `Update`
/// decided (the pause menu or another screen with a cursor, the window in
/// front), kept from one update to the next.
const CURSOR_SHOWN: u64 = 19505961;

/// Mouse buttons in the input block.
pub mod mouse {
    pub const LEFT: u32 = 1;
    pub const RIGHT: u32 = 2;
    pub const MIDDLE: u32 = 4;
    pub const X1: u32 = 8;
    pub const X2: u32 = 16;
}

/// Writes keyboard and mouse input into the engine's input block.
#[derive(Clone, Copy)]
pub struct InputBlock {
    mem: Mem,
    at: u64,
}

impl InputBlock {
    pub(crate) fn new(mem: Mem, at: u64) -> Self {
        InputBlock { mem, at }
    }

    /// A key by Windows virtual-key code.
    pub fn key(&self, vk: u8, down: bool) {
        self.mem.set_u8(self.at + KEYS + vk as u64, if down { 0x80 } else { 0 });
    }

    /// Releases every key and button (e.g. on focus loss).
    pub fn release_all(&self) {
        self.mem.bytes_mut(self.at + KEYS, 256).fill(0);
        self.mem.atomic_u32(self.at + MOUSE_BUTTONS).store(0, Ordering::Relaxed);
    }

    /// Absolute position in back-buffer pixels.
    pub fn mouse_position(&self, x: i32, y: i32) {
        self.mem.set_i32(self.at + MOUSE_X, x);
        self.mem.set_i32(self.at + MOUSE_Y, y);
    }

    /// Relative motion (accumulates until the engine reads it).
    pub fn mouse_motion(&self, dx: i32, dy: i32) {
        self.mem.atomic_u32(self.at + MOUSE_DX).fetch_add(dx as u32, Ordering::AcqRel);
        self.mem.atomic_u32(self.at + MOUSE_DY).fetch_add(dy as u32, Ordering::AcqRel);
    }

    pub fn mouse_wheel(&self, notches: i32) {
        self.mem.atomic_u32(self.at + MOUSE_DZ).fetch_add(notches as u32, Ordering::AcqRel);
    }

    pub fn mouse_button(&self, button: u32, down: bool) {
        let word = self.mem.atomic_u32(self.at + MOUSE_BUTTONS);
        if down {
            word.fetch_or(button, Ordering::AcqRel);
        } else {
            word.fetch_and(!button, Ordering::AcqRel);
        }
    }

    pub fn focus(&self, focused: bool) {
        if !focused {
            self.release_all();
        }
        self.mem.set_u32(self.at + FOCUS, focused as u32);
    }

    pub fn pointer_locked(&self, locked: bool) {
        self.mem.set_u32(self.at + LOCKED, locked as u32);
    }

    /// Whether the game shows its own mouse cursor (the pause menu, the
    /// map, the phone's browser, ...: wherever it is clicked on). The game
    /// draws it itself; on PC it then frees the pointer.
    pub fn cursor_shown(&self) -> bool {
        self.mem.u8(CURSOR_SHOWN) & 1 != 0
    }

    /// Typed text (Enter = 13, Backspace = 8 also go through here).
    pub fn text(&self, s: &str) {
        let head = self.mem.atomic_u32(self.at + TEXT_HEAD);
        let tail = self.mem.atomic_u32(self.at + TEXT_TAIL);
        for unit in s.encode_utf16() {
            let h = head.load(Ordering::Acquire);
            if h.wrapping_sub(tail.load(Ordering::Acquire)) >= TEXT_LEN {
                break; // full
            }
            self.mem.set_u16(self.at + TEXT + (h % TEXT_LEN) as u64 * 2, unit);
            head.store(h.wrapping_add(1), Ordering::Release);
        }
    }

    /// Asks the engine to resize its back buffer.
    pub fn resize(&self, width: u32, height: u32) {
        self.mem.set_u32(self.at + WIN_W, width);
        self.mem.set_u32(self.at + WIN_H, height);
    }
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.pad_update = Some(pad_update);
    hooks.scaleform_begin_method = Some(scaleform_begin_method);
    hooks.scaleform_add_param_float = Some(scaleform_add_param_float);
    hooks.text_box_open = Some(text_box_open_hook);
    hooks.text_box_close = Some(text_box_close_hook);
    hooks.control_update = Some(control_update);
    hooks.set_mouse_cursor_this_frame = Some(set_mouse_cursor_this_frame);
    hooks.menu_shift_depth = Some(menu_shift_depth);
    hooks.player_on_foot = Some(player_on_foot);
    hooks.drive_automobile = Some(drive_automobile);
    hooks.drive_bike = Some(drive_bike);
    hooks.drive_boat = Some(drive_boat);
    hooks.drive_heli = Some(drive_heli);
    hooks.drive_autogyro = Some(drive_autogyro);
    hooks.drive_plane = Some(drive_plane);
    hooks.drive_submarine = Some(drive_submarine);
    hooks.player_in_vehicle = Some(player_in_vehicle);
    hooks.parachute_update = Some(parachute_update);
    hooks.cutscene_update_skip = Some(cutscene_update_skip);
}

#[cfg(test)]
mod phone_tests {
    use super::*;

    #[test]
    fn touch_enlarges_the_phone_without_shifting_its_projected_center() {
        let original = [93.05, 51.7, -113.0];
        let enlarged = enlarge_phone_position(original);
        let projected = |p: [f32; 3]| [p[0] / -p[2], p[1] / -p[2]];
        let before = projected(original);
        let after = projected(enlarged);
        assert!((before[0] - after[0]).abs() < 1.0e-6);
        assert!((before[1] - after[1]).abs() < 1.0e-6);
        assert!(((-original[2] / -enlarged[2]) - TOUCH_PHONE_SCALE).abs() < 1.0e-6);
    }

    #[test]
    fn phone_out_dips_while_the_phone_remakes_itself_do_not_flip_it_back() {
        let t0 = Instant::now();
        let mut phone = PhoneOut::new();
        assert!(!phone.update(false, t0));
        // Out after a short stable read, not on the first frame.
        assert!(!phone.update(true, t0));
        assert!(phone.update(true, t0 + Duration::from_millis(70)));
        // A dip shorter than the going-away wait (the phone remaking itself,
        // the device switching because it is out) changes nothing.
        assert!(phone.update(false, t0 + Duration::from_millis(100)));
        assert!(phone.update(true, t0 + Duration::from_millis(300)));
        // Held away, it is believed and stays believed.
        assert!(phone.update(false, t0 + Duration::from_millis(400)));
        assert!(!phone.update(false, t0 + Duration::from_millis(1_100)));
    }
}
