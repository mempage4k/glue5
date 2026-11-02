//! What the game asks the player to press, for the touch controls: the
//! buttons of its instructional bar (the prompts at the bottom right of the
//! scripts' menus, the character creator, shops, warning screens) and the
//! inputs its help text names (`~INPUT_CONTEXT~`). The touch controls show
//! them as buttons with the game's words, and press them as a pad would, or
//! by the input itself, as the game's own mouse clicks on the bar do (some
//! inputs of these newer scripts have no pad button in this game data).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use glue5_game::{Env, exports, orig};

use crate::input::button;
use crate::memory::Mem;

/// A button the game shows to press now.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    /// Where it is on the pad, one entry per icon the game shows (a zoom
    /// may show LT and RT).
    pub pads: Vec<Pad>,
    /// The input it presses (`rage::InputType`), when the game names it.
    pub input: Option<u32>,
    /// The game's words for it, in its language (none for the help text's).
    pub label: String,
}

/// A place on the pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pad {
    /// Pad buttons ([`button`] bits).
    Buttons(u16),
    /// A stick turned, along x, y or both.
    Stick { right: bool, x: bool, y: bool },
}

/// The input that skips a cutscene (`INPUT_SKIP_CUTSCENE`).
pub const SKIP_CUTSCENE: u32 = 18;
/// Inputs are `rage::InputType`s below this.
const INPUTS: u32 = 363;

// ---- the instructional bar -----------------------------------------------------

/// `CScaleformMgr`'s movies: an array of 512-byte entries (by movie id), the
/// file name inline at +184.
const MOVIES: u64 = 19592976;
const MOVIE_COUNT: u64 = 19592984;
const MOVIE_SIZE: u64 = 512;
const MOVIE_NAME: u64 = 184;
/// The scripts' movies (`REQUEST_SCALEFORM_MOVIE`): 20 entries of 72 bytes,
/// +0 the manager's movie id, +7 drawn this frame (cleared as the scripts
/// start their frame).
const SCRIPT_MOVIES: u64 = 23852944;
const SCRIPT_MOVIE_SIZE: u64 = 72;
const SCRIPT_MOVIE_DRAWN: u64 = 7;
/// A GFxValue: 24 bytes, its type (masked) at +8, its value at +16.
const GFX_VALUE: u64 = 24;
const GFX_BOOL: u32 = 2;
const GFX_NUMBER: u32 = 3;
const GFX_STRING: u32 = 4;

/// One slot of a bar: its icons' tokens (`b_30`...), label, input.
#[derive(Clone, Debug, Default)]
struct Slot {
    icons: Vec<String>,
    label: String,
    input: Option<u32>,
}

/// An instructional movie's bar: the slots being set, the ones drawn.
#[derive(Default)]
struct Bar {
    filling: std::collections::BTreeMap<u32, Slot>,
    shown: Vec<Slot>,
    /// When it was last laid out (the code's bars: the latest is the one
    /// on screen).
    at: Option<Instant>,
}

static BARS: Mutex<Option<HashMap<u32, Bar>>> = Mutex::new(None);

/// Whether the manager's movie `id` is a bar of instructional buttons
/// (the scripts', the pause menu's, the store's: their files end so).
fn is_bar(mem: Mem, id: u32) -> bool {
    movie_file(mem, id).is_some_and(|name| name.to_ascii_lowercase().ends_with(b"instructional_buttons"))
}

/// The file of movie `id` (its slot's, which a later movie may take).
fn movie_file(mem: Mem, id: u32) -> Option<Vec<u8>> {
    let base = mem.u64(MOVIES);
    if base == 0 || id as u64 >= mem.u16(MOVIE_COUNT) as u64 {
        return None;
    }
    let name = mem.bytes(base + id as u64 * MOVIE_SIZE + MOVIE_NAME, 64);
    Some(name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())].to_vec())
}

/// Movies showing a cursor of their own (the browser's, a hacking
/// screen's: their scripts do for keyboard and mouse only), with their
/// files (a movie released with its cursor shown leaves its slot to
/// another), and when a script last moved one (those calls come each
/// frame it is used).
struct Pointer {
    shown: Vec<(u32, Option<Vec<u8>>)>,
    moved: Option<Instant>,
}

static POINTER: Mutex<Pointer> = Mutex::new(Pointer { shown: Vec::new(), moved: None });

/// A movie's method that shows, hides or moves a cursor of its own.
fn pointer_method(mem: Mem, movie: u32, method: &[u8], args: u64, count: u32) {
    let first_bool = (count > 0 && mem.u32(args + 8) & 0x8F == GFX_BOOL).then(|| mem.u8(args + 16) != 0);
    let show = match method {
        b"SHOW_CURSOR" => Some(true),
        b"HIDE_CURSOR" => Some(false),
        b"SET_CURSOR_VISIBILITY" | b"SET_IS_MOUSE_CONTROL" => first_bool,
        b"MOVE_CURSOR" | b"SET_CURSOR" | b"SET_CURSOR_POSITION" | b"SET_CURSOR_STATE" | b"SET_MOUSE_INPUT" => {
            POINTER.lock().unwrap().moved = Some(Instant::now());
            None
        }
        _ => return,
    };
    if let Some(show) = show {
        let mut pointer = POINTER.lock().unwrap();
        pointer.shown.retain(|(m, _)| *m != movie);
        if show {
            pointer.shown.push((movie, movie_file(mem, movie)));
        }
    }
}

/// A script's screen takes a pointer now: its movie, drawn this frame,
/// shows a cursor, or a script moved one lately.
pub(crate) fn pointer_shown(env: *mut Env) -> bool {
    let mem = unsafe { Mem::of(env) };
    let pointer = POINTER.lock().unwrap();
    if pointer.moved.is_some_and(|t| t.elapsed() < Duration::from_millis(250)) {
        return true;
    }
    let shows = |id: u32| pointer.shown.iter().any(|(m, file)| *m == id && movie_file(mem, id) == *file);
    !pointer.shown.is_empty()
        && (0..20)
            .map(|i| SCRIPT_MOVIES + i * SCRIPT_MOVIE_SIZE)
            .any(|e| mem.u8(e + SCRIPT_MOVIE_DRAWN) == 1 && shows(mem.i32(e) as u32))
}

/// Hook for `CScaleformMgr::CallMethodInternal`: every method call of the
/// game's movies (scripts' and code's), its arguments as GFxValues. The
/// first call of each (`instantly` false) builds the bars.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe extern "C" fn scaleform_call_method(
    env: *mut Env,
    ret: u64,
    movie: u32,
    child: u32,
    base: u32,
    name: u64,
    args: u64,
    count: u32,
    instantly: u32,
    store: u32,
    ret_id: u32,
    unique: u32,
) {
    if instantly == 0 {
        let mem = unsafe { Mem::of(env) };
        let method = mem.bytes(name, 28);
        let method = &method[..method.iter().position(|&b| b == 0).unwrap_or(method.len())];
        pointer_method(mem, movie, method, args, count);
        if matches!(method, b"CLEAR_ALL" | b"SET_DATA_SLOT" | b"SET_DATA_SLOT_EMPTY" | b"DRAW_INSTRUCTIONAL_BUTTONS")
            && is_bar(mem, movie)
        {
            bar_method(mem, movie, method, args, count);
        }
    }
    // Keep both `instantly` variants observable; the phone trace filters to
    // app dispatch calls and records the flag rather than assuming duplicates.
    crate::script_assets::trace_phone_scaleform_call(env, movie, name, args, count, instantly);
    unsafe { orig::scaleform_call_method(env, ret, movie, child, base, name, args, count, instantly, store, ret_id, unique) }
}

/// A method of the bar `movie`.
fn bar_method(mem: Mem, movie: u32, method: &[u8], args: u64, count: u32) {
    let mut bars = BARS.lock().unwrap();
    let bar = bars.get_or_insert_with(HashMap::new).entry(movie).or_default();
    match method {
        b"CLEAR_ALL" | b"SET_DATA_SLOT_EMPTY" => bar.filling.clear(),
        b"DRAW_INSTRUCTIONAL_BUTTONS" => {
            bar.shown = bar.filling.values().cloned().collect();
            bar.at = Some(Instant::now());
        }
        _ => {
            // The slot's number, its icons (several joined by %), its label,
            // then (PC) whether it is clickable and the input it presses.
            let mut index = None;
            let mut slot = Slot::default();
            let mut clickable = false;
            for i in 0..count.min(8) as u64 {
                let value = args + i * GFX_VALUE;
                match mem.u32(value + 8) & 0x8F {
                    GFX_NUMBER => {
                        let n = mem.f64(value + 16);
                        if index.is_none() {
                            index = Some(n as u32);
                        } else if clickable && (0.0..INPUTS as f64).contains(&n) {
                            slot.input = Some(n as u32);
                        }
                    }
                    GFX_BOOL => clickable = mem.u8(value + 16) != 0,
                    GFX_STRING => {
                        // A managed string (0x40) points at its text's pointer.
                        let at = mem.u64(value + 16);
                        let at = if mem.u8(value + 8) & 0x40 != 0 { mem.u64(at) } else { at };
                        let text = c_str(mem, at, 256);
                        if text.starts_with("b_") || text.starts_with("t_") {
                            slot.icons.extend(text.split('%').filter(|t| !t.is_empty()).map(str::to_owned));
                        } else if slot.label.is_empty() {
                            slot.label = text;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(index) = index {
                bar.filling.insert(index, slot);
            }
        }
    }
}

/// A NUL-terminated string of at most `max` bytes.
fn c_str(mem: Mem, at: u64, max: usize) -> String {
    if at == 0 {
        return String::new();
    }
    let bytes = mem.bytes(at, max);
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(max);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Where an icon token of the bar is on the pad (`CTextFormat`'s PAD_*
/// icons, `b_` + their number).
fn icon_pad(token: &str) -> Option<Pad> {
    use button::*;
    let n: u32 = token.strip_prefix("b_")?.parse().ok()?;
    let stick = |right, x, y| Pad::Stick { right, x, y };
    Some(match n {
        0 | 4 => Pad::Buttons(UP),
        1 | 5 => Pad::Buttons(DOWN),
        2 | 6 => Pad::Buttons(LEFT),
        3 | 7 => Pad::Buttons(RIGHT),
        8 | 9 | 47 => Pad::Buttons(UP | DOWN | LEFT | RIGHT),
        10 | 45 => Pad::Buttons(UP | DOWN),
        11 | 46 => Pad::Buttons(LEFT | RIGHT),
        12 | 13 | 18 => stick(false, false, true),
        14 | 15 | 19 => stick(false, true, false),
        17 | 20 => stick(false, true, true),
        16 => Pad::Buttons(L3),
        21 | 22 | 27 => stick(true, false, true),
        23 | 24 | 28 => stick(true, true, false),
        26 | 29 => stick(true, true, true),
        25 => Pad::Buttons(R3),
        30 => Pad::Buttons(SOUTH),
        31 => Pad::Buttons(EAST),
        32 => Pad::Buttons(WEST),
        33 => Pad::Buttons(NORTH),
        34 => Pad::Buttons(L1),
        35 => Pad::Buttons(L2),
        36 => Pad::Buttons(R1),
        37 => Pad::Buttons(R2),
        38 => Pad::Buttons(START),
        39 => Pad::Buttons(SELECT),
        _ => return None,
    })
}

// ---- the help text --------------------------------------------------------------

/// The help message (the box at the top left), slot 0 of `CHelpMessage`'s
/// texts: 600 bytes, empty while none shows. Inputs are named in it as
/// `~INPUT_CONTEXT~`.
const HELP_TEXT: u64 = 25477136;
const HELP_TEXT_LEN: usize = 600;

// ---- the inputs -------------------------------------------------------------------

/// The player's controls (`CControlMgr`'s main player control): its inputs'
/// values at +33704, 72 bytes each.
pub(crate) const CONTROL: u64 = 24488864;
const CONTROL_VALUES: u64 = 33704;
const CONTROL_VALUE_SIZE: u64 = 72;
/// `GetInputSource`'s device: the first pad (its mapping, whichever device
/// the engine saw last: direct touch has menus see keyboard and mouse).
const PAD: u32 = 0;
/// `rage::ioSource` devices: a pad's button (parameter: its bit), analog
/// button (its bit's index), axis (0x1000 + LX, LY, RX, RY; 0x1016.. the
/// left stick's up, down, left, right, 0x101A.. the right's).
const PAD_BUTTON: u32 = 9;
const PAD_ANALOG: u32 = 11;
const PAD_AXIS: u32 = 12;

/// Where `input` is on the pad with its mapping.
unsafe fn input_pad(env: *mut Env, input: u32) -> Option<Pad> {
    let mem = unsafe { Mem::of(env) };
    let (device, parameter) = unsafe {
        with_stack(env, 16, |source| {
            orig::control_input_source(env, source, CONTROL, input, PAD, 0, 0);
            (mem.u32(source), mem.u32(source + 4))
        })
    }?;
    let stick = |right, x, y| Pad::Stick { right, x, y };
    match (device, parameter) {
        (PAD_BUTTON, bits) if bits != 0 && bits <= 0xFFFF => Some(Pad::Buttons(bits as u16)),
        (PAD_ANALOG, index) if index < 16 => Some(Pad::Buttons(1 << index)),
        (PAD_AXIS, 0x1000 | 0x1018..=0x1019) => Some(stick(false, true, false)),
        (PAD_AXIS, 0x1001 | 0x1016..=0x1017) => Some(stick(false, false, true)),
        (PAD_AXIS, 0x1002 | 0x101C..=0x101D) => Some(stick(true, true, false)),
        (PAD_AXIS, 0x1003 | 0x101A..=0x101B) => Some(stick(true, false, true)),
        _ => None,
    }
}

/// `rage::InputType` by its name (`INPUT_CONTEXT`), if the code has it.
unsafe fn input_by_name(env: *mut Env, name: &str) -> Option<u32> {
    let input = unsafe {
        with_stack(env, name.len() as u64 + 1, |at| {
            Mem::of(env).write_str(at, name, name.len() + 1);
            orig::control_input_by_name(env, at)
        })
    }?;
    (input < INPUTS).then_some(input)
}

/// The inputs the touch controls' buttons stand for on foot, and the
/// stick's sprint: the game's control layout may move their pad buttons
/// (first person swaps sprint and duck; the southpaw and trigger-swap
/// schemes move others).
const MAPPED: [u32; 10] = [
    21, // SPRINT
    22, // JUMP
    23, // ENTER
    24, // ATTACK
    25, // AIM
    36, // DUCK
    37, // SELECT_WEAPON
    44, // COVER
    51, // CONTEXT
    58, // THROW_GRENADE
];

/// The pad buttons of [`MAPPED`] now.
static PAD_MAP: Mutex<Vec<(u32, u16)>> = Mutex::new(Vec::new());

/// The pad button each of the touch buttons' inputs is on now, with the
/// control layout in use (as `(input, buttons)`).
pub fn pad_map() -> Vec<(u32, u16)> {
    PAD_MAP.lock().unwrap().clone()
}

/// Runs `f` with `size` zeroed bytes of the game's stack.
unsafe fn with_stack<R>(env: *mut Env, size: u64, f: impl FnOnce(u64) -> R) -> Option<R> {
    let top = unsafe { exports::emscripten_stack_get_current(env) }.ok()?;
    let at = unsafe { exports::_emscripten_stack_alloc(env, size.next_multiple_of(16)) }.ok()?;
    unsafe { Mem::of(env) }.bytes_mut(at, size as usize).fill(0);
    let r = f(at);
    let _ = unsafe { exports::_emscripten_stack_restore(env, top) };
    Some(r)
}

/// The help text's inputs, as last read: the text they came from.
struct Help {
    text: Vec<u8>,
    prompts: Vec<Prompt>,
}

/// What the frame's prompts are, as the app reads them.
struct State {
    help: Help,
    shown: Vec<Prompt>,
    /// Inputs the touch controls hold: pressed until when at least.
    held: Vec<(u32, Instant)>,
}

static STATE: Mutex<State> = Mutex::new(State {
    help: Help { text: Vec::new(), prompts: Vec::new() },
    shown: Vec::new(),
    held: Vec::new(),
});

/// A press of an input lasts this long at least: the engine reads it once
/// a frame, and a script once per its frame.
const MIN_PRESS: Duration = Duration::from_millis(80);

/// The prompts on screen now: the instructional bars drawn last frame, then
/// the help text's inputs.
pub fn prompts() -> Vec<Prompt> {
    STATE.lock().unwrap().shown.clone()
}

/// The touch controls hold `input` down: it stays pressed a moment longer
/// (they call again while it is held). It is pressed as the game's own
/// mouse clicks on the bar press it, whatever device the game maps it to.
pub fn hold_input(input: u32) {
    if input < INPUTS {
        let mut state = STATE.lock().unwrap();
        let until = Instant::now() + MIN_PRESS;
        match state.held.iter_mut().find(|(i, _)| *i == input) {
            Some((_, u)) => *u = until,
            None => state.held.push((input, until)),
        }
    }
}

/// The scripts' bars drawn last frame (their movies' ids).
fn script_bars(mem: Mem) -> Vec<u32> {
    (0..20)
        .map(|i| SCRIPT_MOVIES + i * SCRIPT_MOVIE_SIZE)
        .filter(|&e| mem.u8(e + SCRIPT_MOVIE_DRAWN) == 1)
        .map(|e| mem.i32(e) as u32)
        .filter(|&id| is_bar(mem, id))
        .collect()
}

/// A script shows its instructional bar: it takes the player's input (its
/// menu), even where the pause menu counts as open (the GTA Online
/// character creator runs inside it).
pub(crate) fn script_bar_shown(env: *mut Env) -> bool {
    !script_bars(unsafe { Mem::of(env) }).is_empty()
}

/// Once a frame, from `CControl::Update`, while touch plays: which bars
/// show (`code_bar`: one of the code's, as a warning, the text box or the
/// pause menu have, is up), the help text's inputs, the inputs touch holds.
pub(crate) unsafe fn update(env: *mut Env, touch_play: bool, code_bar: bool) {
    let mem = unsafe { Mem::of(env) };
    let mut prompts = Vec::new();
    if touch_play {
        let map = MAPPED
            .iter()
            .filter_map(|&input| match unsafe { input_pad(env, input) } {
                Some(Pad::Buttons(bits)) => Some((input, bits)),
                _ => None,
            })
            .collect();
        *PAD_MAP.lock().unwrap() = map;
        // The scripts' bars drawn last frame; the code's: the one laid out
        // last.
        let drawn = script_bars(mem);
        let scripts: Vec<u32> =
            (0..20).map(|i| mem.i32(SCRIPT_MOVIES + i * SCRIPT_MOVIE_SIZE) as u32).collect();
        let bars = BARS.lock().unwrap();
        if let Some(bars) = bars.as_ref() {
            let code = code_bar
                .then(|| {
                    bars.iter()
                        .filter(|(id, bar)| !scripts.contains(id) && bar.at.is_some())
                        .max_by_key(|(_, bar)| bar.at)
                        .map(|(id, _)| *id)
                })
                .flatten();
            for id in drawn.iter().chain(code.as_ref()) {
                if let Some(bar) = bars.get(id) {
                    for slot in &bar.shown {
                        let pads = slot.icons.iter().filter_map(|t| icon_pad(t)).collect();
                        prompts.push(Prompt { pads, input: slot.input, label: slot.label.clone() });
                    }
                }
            }
        }
    }
    let mut state = STATE.lock().unwrap();
    // The help text, read again when it changes.
    let text = if touch_play { mem.bytes(HELP_TEXT, HELP_TEXT_LEN) } else { &[] };
    let text = &text[..text.iter().position(|&b| b == 0).unwrap_or(text.len())];
    if text != state.help.text.as_slice() {
        let mut help = Vec::new();
        for token in String::from_utf8_lossy(text).split('~').filter(|t| t.starts_with("INPUT_")) {
            let Some(input) = (unsafe { input_by_name(env, token) }) else { continue };
            if help.iter().any(|p: &Prompt| p.input == Some(input)) {
                continue;
            }
            let pads = unsafe { input_pad(env, input) }.into_iter().collect();
            help.push(Prompt { pads, input: Some(input), label: String::new() });
        }
        state.help = Help { text: text.to_vec(), prompts: help };
    }
    prompts.extend(state.help.prompts.iter().cloned());
    // A scene of a script's own, waiting for its skip.
    if touch_play && skip_asked() && !prompts.iter().any(|p| p.input == Some(SKIP_CUTSCENE)) {
        prompts.push(Prompt { pads: Vec::new(), input: Some(SKIP_CUTSCENE), label: String::new() });
    }
    state.shown = prompts;
    // The inputs touch holds, pressed for the next frame.
    let now = Instant::now();
    state.held.retain(|(_, until)| now < *until);
    let held: Vec<u32> = state.held.iter().map(|(i, _)| *i).collect();
    drop(state);
    for input in held {
        unsafe {
            with_stack(env, 16, |source| {
                // As a pad's button.
                mem.set_u32(source, PAD_BUTTON);
                orig::io_value_set_next(env, CONTROL + CONTROL_VALUES + input as u64 * CONTROL_VALUE_SIZE, 1.0, source);
            })
        };
    }
}

// ---- skipping scenes --------------------------------------------------------------

/// When a script last asked whether the cutscene skip was just pressed: a
/// scene of its own (cameras and animations, not a cutscene) can be skipped.
static SKIP_ASKED: Mutex<Option<Instant>> = Mutex::new(None);
/// A script's question counts as now for this long.
const ASKED_FRESH: Duration = Duration::from_millis(250);

/// Whether a script asks for the skip now.
pub(crate) fn skip_asked() -> bool {
    SKIP_ASKED.lock().unwrap().is_some_and(|t| t.elapsed() < ASKED_FRESH)
}

/// The input a script's `IS_[DISABLED_]CONTROL_JUST_PRESSED(control, input)`
/// asks about (`Info`: +16 its arguments, 8 bytes each).
fn note_asked(env: *mut Env, info: u64) {
    let mem = unsafe { Mem::of(env) };
    let args = mem.u64(info + 16);
    if args != 0 && mem.i32(args + 8) == SKIP_CUTSCENE as i32 {
        *SKIP_ASKED.lock().unwrap() = Some(Instant::now());
    }
}

/// Hooks for `IS_CONTROL_JUST_PRESSED` and `IS_DISABLED_CONTROL_JUST_PRESSED`.
pub(crate) unsafe extern "C" fn is_control_just_pressed(env: *mut Env, info: u64) {
    note_asked(env, info);
    unsafe { orig::is_control_just_pressed(env, info) }
}

pub(crate) unsafe extern "C" fn is_disabled_control_just_pressed(env: *mut Env, info: u64) {
    note_asked(env, info);
    unsafe { orig::is_disabled_control_just_pressed(env, info) }
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.scaleform_call_method = Some(scaleform_call_method);
    hooks.is_control_just_pressed = Some(is_control_just_pressed);
    hooks.is_disabled_control_just_pressed = Some(is_disabled_control_just_pressed);
}
