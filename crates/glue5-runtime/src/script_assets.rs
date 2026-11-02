//! Assets the scripts wait for that this game data lacks.
//!
//! The scripts are a later version than the game data, and some of what
//! they load is not in it. The engine never finishes loading it, and a
//! script that waits for it waits forever:
//!
//! - Sound banks (GTA Online's `DLC_GTAO\SNACKS`, which `MP_WASTED` and
//!   `HUD_321_GO` are mapped to as well): the bank's file is not found and
//!   its wave slot stays failed, so a script's request
//!   (`audScriptAudioEntity::RequestScriptBank`) answers "not yet". Freemode's
//!   interaction animations (the quick action, pointing...) wait for the
//!   snacks bank with sprint, attacks, the weapon wheel and the phone
//!   disabled until the player dies.
//!   A bank named by a later update (an apartment shower's
//!   `dlc_EXEC1/MP_APARTMENT_SHOWER_01`) is not even in the bank list: the
//!   request by name answers "not yet" for good, and a player who spawns in
//!   the shower waits in the sky.
//! - Texture dictionaries (GTA Online's `MPShopSale`): one the streaming
//!   store has no slot for is never loaded, so the interaction menu, which
//!   waits for it with its other assets, opens without ever being drawn.
//!
//! - Models and text blocks of later updates (golf waits for two models,
//!   the awards' texts `MPAWD` and the club emblems' textures).
//! - Animation dictionaries, clip sets, particle assets and Scaleform movies
//!   of later updates: the apartment's door (`anim@apt_trans@hinge_l`, the
//!   heists'), the heist board's movies, freemode's pointing
//!   (`anim@mp_point`). Every script's requests, scanned, name hundreds
//!   this data lacks.
//! - Cutscenes of later updates: Lester's introduction to the heists
//!   (`HEIST_INT`), which `am_Heist_Int` plays for a high-end apartment's
//!   owner, waiting for it to load. Started, a cutscene the data lacks ends
//!   at once, and the scripts go on as after it.
//!
//! Each counts as loaded for the scripts here: they go on without those
//! sounds, textures, models or texts.
//!
//! Objects of later updates are created by scripts that wait for them to
//! exist: an apartment's front door (`hei_heist_apart2_door`, the heists'),
//! which the apartment's script creates again every frame, its other
//! players waiting for it. A model the data lacks makes a stand-in here,
//! hidden and without collision: the script has its object, the player
//! sees the place as this data built it.
//!
//! Interiors that later updates rebuilt: GTA Online's high-end apartments
//! became `hei_dlc_apart_high_new*` with the heists, and the scripts look
//! for the apartment by that type. This data has the earlier apartment at
//! the same place, under another name, so the lookup finds nothing and a
//! player who owns one, spawning there, waits in the sky forever ("Joining
//! GTA Online"). A type the data lacks finds the interior at those
//! coordinates here, whatever its type.
//!
//! Whole places are missing too: GTA Online's "simple interiors" (the
//! casino, bunkers, hangars, nightclubs, the arcade...), DLC from 2016 on.
//! Freemode starts `AM_MP_SMPL_INTERIOR_EXT` near each one's entrance, and
//! walking in takes the player's control for the entry scene, which never
//! ends: the casino's waits for its door (a model this data lacks) to be
//! created, and the interior behind it does not exist either. That script
//! never loads here, so the entrances do nothing.
//!
//! And screens: the phone's movie (`cellphone_ifruit`) has no app for GTA
//! Online's boss agency (`CELL_BOSSAGE`), which the scripts put on the
//! phone. Opening it, the movie's `LOAD_APP` and `CELLPHONE_APP` call each
//! other without end; each ActionScript call pushes a diagnostic context,
//! and at the 33rd the engine stops ("MessageCommon - too many contexts
//! pushed"). ActionScript calls nest at most `AS_DEPTH` deep here: deeper
//! ones return at once. And the app is not on the phone's home screen here
//! (see `PHONE_HOME`), so it is not opened at all.

use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};

use glue5_game::{Env, exports, orig};

use crate::memory::Mem;

/// Scripts for places this game data lacks (see above).
const MISSING_PLACES: &[&str] = &["AM_MP_SMPL_INTERIOR_EXT"];

fn for_missing_place(m: Mem, name: u64) -> bool {
    let name = m.str(name);
    MISSING_PLACES.iter().any(|s| s.eq_ignore_ascii_case(&name))
}

/// Hook for `script_commands::RequestScript(char const*)`: REQUEST_SCRIPT.
pub(crate) unsafe extern "C" fn script_request(env: *mut Env, name: u64) {
    let mem = unsafe { Mem::of(env) };
    if let Some(tap) = recent_phone_tap_context()
        && let Some(script) = phone_app_script_name(mem, name)
    {
        trace_phone_tap(Some(tap), format!("phone script REQUEST_SCRIPT name={script}"));
    }
    if !for_missing_place(mem, name) {
        unsafe { orig::script_request(env, name) };
    }
}

/// A bounded, static phone/app script identifier useful for correlating the
/// existing selection path with its request. Never trace arbitrary script text.
fn phone_app_script_name(m: Mem, name: u64) -> Option<String> {
    if name == 0 {
        return None;
    }
    let bytes = m.bytes(name, 64);
    let end = bytes.iter().position(|&byte| byte == 0)?;
    if end == 0 || end > 63 {
        return None;
    }
    let script = std::str::from_utf8(&bytes[..end]).ok()?;
    if !script.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')) {
        return None;
    }
    let lower = script.to_ascii_lowercase();
    (lower.starts_with("app") || lower.starts_with("cellphone_") || lower.starts_with("cell_"))
        .then(|| script.to_owned())
}

/// Hook for `script_commands::HasScriptLoaded(char const*)`:
/// HAS_SCRIPT_LOADED, never for those scripts (freemode asks again while
/// the player is near an entrance).
pub(crate) unsafe extern "C" fn script_loaded(env: *mut Env, name: u64) -> u32 {
    if for_missing_place(unsafe { Mem::of(env) }, name) { 0 } else { unsafe { orig::script_loaded(env, name) } }
}

/// `audWaveSlot`'s loading status: loaded, loading, failed.
const LOADED: u32 = 0;
const FAILED: u32 = 2;

thread_local! {
    /// Inside a script's bank request.
    static REQUEST: Cell<bool> = const { Cell::new(false) };
}

/// Hook for `audScriptAudioEntity::RequestScriptBank(unsigned int, bool,
/// unsigned int)`.
pub(crate) unsafe extern "C" fn bank_request(env: *mut Env, bank: u32, over_network: u32, players: u32) -> u32 {
    REQUEST.with(|r| r.set(true));
    let loaded = unsafe { orig::script_bank_request(env, bank, over_network, players) };
    REQUEST.with(|r| r.set(false));
    loaded
}

/// The sound factory (`audSoundFactory`), whose banks requests name.
const SOUND_FACTORY: u64 = 12033216;
/// Bank indices above this are none (not found).
const LAST_BANK: u32 = 65534;

/// Hook for `audScriptAudioEntity::RequestScriptBank(char const*, bool,
/// unsigned int, bool)`: yes for a name no bank has.
pub(crate) unsafe extern "C" fn named_bank_request(env: *mut Env, name: u64, over_network: u32, players: u32, hint: u32) -> u32 {
    let loaded = unsafe { orig::script_named_bank_request(env, name, over_network, players, hint) };
    if loaded != 0 || name == 0 { loaded } else { !bank_known(env, &unsafe { Mem::of(env) }.str(name)) as u32 }
}

/// Whether the sound factory has a bank of this name, looked up as the
/// request does: with backslashes, then under `SCRIPT\`.
fn bank_known(env: *mut Env, name: &str) -> bool {
    let name = name.replace('/', "\\");
    [name.clone(), format!("SCRIPT\\{name}")].iter().any(|n| {
        let Ok(top) = (unsafe { exports::emscripten_stack_get_current(env) }) else { return true };
        let Ok(at) = (unsafe { exports::_emscripten_stack_alloc(env, n.len() as u64 + 1) }) else { return true };
        unsafe { Mem::of(env) }.write_str(at, n, n.len() + 1);
        let index = unsafe { orig::bank_index_from_name(env, SOUND_FACTORY, at) };
        let _ = unsafe { exports::_emscripten_stack_restore(env, top) };
        index <= LAST_BANK
    })
}

/// Hook for `rage::audWaveSlot::GetBankLoadingStatus(unsigned int) const`.
pub(crate) unsafe extern "C" fn bank_loading_status(env: *mut Env, slot: u64, bank: u32) -> u32 {
    let status = unsafe { orig::bank_loading_status(env, slot, bank) };
    if status == FAILED && REQUEST.with(Cell::get) { LOADED } else { status }
}

/// A streaming store's map from name hashes to slots, in this build: at
/// `map` the entries ({ u32 hash, u32 slot, u32 next }), at +8 the buckets
/// (the first entry's index each), at +16 the bucket count.
#[derive(Clone, Copy)]
struct SlotMap(u64);

/// The texture dictionary store's.
const TXDS: SlotMap = SlotMap(11097504);
/// The particle asset store's.
const PTFX_ASSETS: SlotMap = SlotMap(12283968);
/// The cutscene store's (its names hashed in the files' namespace, 1).
const CUTSCENES: SlotMap = SlotMap(19337352);
const NONE: u32 = u32::MAX;

impl SlotMap {
    /// Whether the store has a slot for this name hash.
    fn has(self, m: Mem, hash: u32) -> bool {
        let buckets = m.u32(self.0 + 16);
        if buckets == 0 {
            return false;
        }
        let entries = m.u64(self.0);
        let mut i = m.u32(m.u64(self.0 + 8) + u64::from(hash % buckets) * 4);
        while i != NONE {
            let entry = entries + u64::from(i) * 12;
            if m.u32(entry) == hash {
                return m.u32(entry + 4) != NONE;
            }
            i = m.u32(entry + 8);
        }
        false
    }
}

/// Hook for `graphics_commands::HasStreamedTxdLoaded(char const*)`.
pub(crate) unsafe extern "C" fn txd_loaded(env: *mut Env, name: u64) -> u32 {
    if unsafe { orig::script_txd_loaded(env, name) } != 0 {
        return 1;
    }
    let hash = unsafe { orig::hash_string(env, 0, name) };
    !TXDS.has(unsafe { Mem::of(env) }, hash) as u32
}

/// Hook for `streaming_commands::HasAnimDictLoaded(char const*)`: yes too for
/// a dictionary the anim store has no slot for.
pub(crate) unsafe extern "C" fn anim_dict_loaded(env: *mut Env, name: u64) -> u32 {
    if unsafe { orig::anim_dict_loaded(env, name) } != 0 {
        return 1;
    }
    (unsafe { orig::anim_dict_exists(env, name) } == 0) as u32
}

/// Hook for `streaming_commands::HasClipSetLoaded(char const*)` (clip sets and
/// anim sets): yes too for a name no clip set has.
pub(crate) unsafe extern "C" fn clip_set_loaded(env: *mut Env, name: u64) -> u32 {
    if unsafe { orig::clip_set_loaded(env, name) } != 0 {
        return 1;
    }
    let hash = unsafe { orig::hash_string(env, 0, name) };
    let Ok(top) = (unsafe { exports::emscripten_stack_get_current(env) }) else { return 0 };
    let Ok(id) = (unsafe { exports::_emscripten_stack_alloc(env, 16) }) else { return 0 };
    unsafe { Mem::of(env) }.set_u32(id, hash);
    let clip_set = unsafe { orig::clip_set(env, id) };
    let _ = unsafe { exports::_emscripten_stack_restore(env, top) };
    (clip_set == 0) as u32
}

/// Hook for `streaming_commands::HasNamedPtFxAssetLoaded(char const*)`: yes
/// too for an asset the particle asset store has no slot for.
pub(crate) unsafe extern "C" fn named_ptfx_loaded(env: *mut Env, name: u64) -> u32 {
    if unsafe { orig::named_ptfx_loaded(env, name) } != 0 {
        return 1;
    }
    let hash = unsafe { orig::hash_string(env, 0, name) };
    !PTFX_ASSETS.has(unsafe { Mem::of(env) }, hash) as u32
}

/// Script threads that asked for a Scaleform movie this data lacks (which
/// the request answers with 0).
static LACKING_MOVIE: Mutex<Vec<u64>> = Mutex::new(Vec::new());

/// Hook for `graphics_commands::CommandRequestScaleformMovieCommon(char
/// const*, bool, bool)`: REQUEST_SCALEFORM_MOVIE and its variants.
pub(crate) unsafe extern "C" fn scaleform_request(env: *mut Env, name: u64, a: u32, b: u32) -> u32 {
    let movie = unsafe { orig::scaleform_request(env, name, a, b) };
    let mem = unsafe { Mem::of(env) };
    if let Some(file) = phone_movie_file_name(mem, name) {
        if movie != 0 {
            identify_phone_movie(movie, file);
        } else {
            trace_phone_movie(format!("phone movie request failed file={file} id=0"));
        }
    } else if movie != 0 && name != 0 {
        clear_reused_phone_movie(movie);
    }
    if movie == 0 && name != 0 && !movie_in_image(env, &mem.str(name)) {
        let thread = unsafe { orig::script_active_thread(env) };
        let mut lacking = LACKING_MOVIE.lock().unwrap();
        if thread != 0 && !lacking.contains(&thread) {
            lacking.push(thread);
        }
    }
    movie
}

/// Whether the game data has this Scaleform movie (its file).
fn movie_in_image(env: *mut Env, name: &str) -> bool {
    let Ok(top) = (unsafe { exports::emscripten_stack_get_current(env) }) else { return true };
    // Room for what the engine adds to the name.
    let cap = name.len() + 64;
    let Ok(at) = (unsafe { exports::_emscripten_stack_alloc(env, cap as u64) }) else { return true };
    unsafe { Mem::of(env) }.write_str(at, name, cap);
    let exists = unsafe { orig::scaleform_in_image(env, at) } != 0;
    let _ = unsafe { exports::_emscripten_stack_restore(env, top) };
    exists
}

/// The cutscene last asked for lacks in this data.
static LACKING_CUTSCENE: AtomicBool = AtomicBool::new(false);

/// Whether the cutscene store has this cutscene.
fn cutscene_exists(env: *mut Env, name: u64) -> bool {
    let hash = unsafe { orig::hash_string(env, 1, name) };
    CUTSCENES.has(unsafe { Mem::of(env) }, hash)
}

/// Hook for `cutscene_commands::CommandRequestCutScene(char const*, int)`.
pub(crate) unsafe extern "C" fn cutscene_request(env: *mut Env, name: u64, flags: u32) {
    LACKING_CUTSCENE.store(name != 0 && !cutscene_exists(env, name), Ordering::Relaxed);
    unsafe { orig::cutscene_request(env, name, flags) }
}

/// Hook for `cutscene_commands::CommandRequestCutSceneWithPlayBackList(char
/// const*, int, int)`.
pub(crate) unsafe extern "C" fn cutscene_request_list(env: *mut Env, name: u64, sections: u32, flags: u32) {
    LACKING_CUTSCENE.store(name != 0 && !cutscene_exists(env, name), Ordering::Relaxed);
    unsafe { orig::cutscene_request_list(env, name, sections, flags) }
}

/// Hook for `scrWrapped_HAS_CUTSCENE_LOADED::Call(rage::scrThread::Info&)`:
/// yes too when the cutscene asked for lacks.
pub(crate) unsafe extern "C" fn cutscene_loaded(env: *mut Env, info: u64) {
    unsafe { orig::cutscene_loaded(env, info) };
    let m = unsafe { Mem::of(env) };
    let result = m.u64(info);
    if m.u32(result) == 0 && LACKING_CUTSCENE.load(Ordering::Relaxed) {
        m.set_u32(result, 1);
    }
}

/// Hook for `scrWrapped_HAS_THIS_CUTSCENE_LOADED::Call(rage::scrThread::
/// Info&)` (the cutscene's name the first parameter): yes too for a
/// cutscene this data lacks.
pub(crate) unsafe extern "C" fn this_cutscene_loaded(env: *mut Env, info: u64) {
    unsafe { orig::this_cutscene_loaded(env, info) };
    let m = unsafe { Mem::of(env) };
    let result = m.u64(info);
    let name = m.u64(m.u64(info + 16));
    if m.u32(result) == 0 && name != 0 && !cutscene_exists(env, name) {
        m.set_u32(result, 1);
    }
}

/// Hook for `scrWrapped_HAS_SCALEFORM_MOVIE_FILENAME_LOADED::Call(rage::
/// scrThread::Info&)` (the movie's name the first parameter): yes too for a
/// movie this data lacks.
pub(crate) unsafe extern "C" fn scaleform_file_loaded(env: *mut Env, info: u64) {
    unsafe { orig::scaleform_file_loaded(env, info) };
    let m = unsafe { Mem::of(env) };
    let result = m.u64(info);
    let name = m.u64(m.u64(info + 16));
    if m.u32(result) == 0 && name != 0 && !movie_in_image(env, &m.str(name)) {
        m.set_u32(result, 1);
    }
}

/// Hook for `scrWrapped_HAS_SCALEFORM_MOVIE_LOADED::Call(rage::scrThread::
/// Info&)` (the movie the first parameter, the answer the result): the movie
/// 0 counts as loaded for a script that asked for one this data lacks.
pub(crate) unsafe extern "C" fn scaleform_loaded(env: *mut Env, info: u64) {
    unsafe { orig::scaleform_loaded(env, info) };
    let m = unsafe { Mem::of(env) };
    let result = m.u64(info);
    if m.u32(result) == 0
        && m.u32(m.u64(info + 16)) == 0
        && LACKING_MOVIE.lock().unwrap().contains(&unsafe { orig::script_active_thread(env) })
    {
        m.set_u32(result, 1);
    }
}

/// Hook for `streaming_commands::HasModelLoaded(int)`: HAS_MODEL_LOADED,
/// yes for a model the data lacks (no archetype has its hash).
pub(crate) unsafe extern "C" fn model_loaded(env: *mut Env, hash: u32) -> u32 {
    if unsafe { orig::script_model_loaded(env, hash) } != 0 {
        return 1;
    }
    !has_archetype(env, hash) as u32
}

/// What stands in for a model the data lacks: a small one of the base game.
const STAND_IN: &str = "prop_golf_ball";

/// The engine's name hash (`rage::atStringHash`, Jenkins' one-at-a-time
/// over the lowercase name).
const fn name_hash(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let mut h: u32 = 0;
    let mut i = 0;
    while i < bytes.len() {
        h = h.wrapping_add(bytes[i].to_ascii_lowercase() as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
        i += 1;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

/// A script's object creation (its model the first parameter, the object
/// the result): a model no archetype has becomes `STAND_IN`, hidden.
fn create_object(env: *mut Env, info: u64, create: impl FnOnce()) {
    let m = unsafe { Mem::of(env) };
    let model = m.u64(info + 16);
    let lacking = !has_archetype(env, m.u32(model));
    if lacking {
        if !has_archetype(env, name_hash(STAND_IN)) {
            return create();
        }
        m.set_u32(model, name_hash(STAND_IN));
    }
    create();
    let object = m.u32(m.u64(info));
    if lacking && object != 0 {
        unsafe {
            orig::set_entity_visible(env, object, 0, 0);
            orig::set_entity_collision(env, object, 0, 0);
        }
    }
}

/// Hook for `scrWrapped_CREATE_OBJECT::Call(rage::scrThread::Info&)`.
pub(crate) unsafe extern "C" fn create_object_offset(env: *mut Env, info: u64) {
    create_object(env, info, || unsafe { orig::create_object(env, info) });
}

/// Hook for `scrWrapped_CREATE_OBJECT_NO_OFFSET::Call(rage::scrThread::Info&)`.
pub(crate) unsafe extern "C" fn create_object_no_offset(env: *mut Env, info: u64) {
    create_object(env, info, || unsafe { orig::create_object_no_offset(env, info) });
}

/// Hook for `interior_commands::CommandGetInteriorAtCoordsOfType(rage::
/// scrVector const&, char const*)`: GET_INTERIOR_AT_COORDS_WITH_TYPE, the
/// interior at those coordinates for a type no archetype has (see above).
pub(crate) unsafe extern "C" fn interior_of_type(env: *mut Env, coords: u64, kind: u64) -> u32 {
    let interior = unsafe { orig::interior_of_type(env, coords, kind) };
    if interior != 0 || kind == 0 {
        return interior;
    }
    let hash = unsafe { orig::hash_string(env, 0, kind) };
    if has_archetype(env, hash) { interior } else { unsafe { orig::interior_at(env, coords) } }
}

/// Whether an archetype (a model, an interior) has this name hash.
fn has_archetype(env: *mut Env, hash: u32) -> bool {
    let Ok(top) = (unsafe { exports::emscripten_stack_get_current(env) }) else { return true };
    let Ok(id) = (unsafe { exports::_emscripten_stack_alloc(env, 16) }) else { return true };
    let archetype = unsafe { orig::archetype_from_hash(env, hash, id) };
    let _ = unsafe { exports::_emscripten_stack_restore(env, top) };
    archetype != 0
}

/// The additional text slots (CTextFile's 23) and the block each was last
/// asked to load, until the slot is freed.
const TEXT_SLOTS: usize = 23;
static ASKED: Mutex<[Option<String>; TEXT_SLOTS]> = Mutex::new([const { None }; TEXT_SLOTS]);

/// Whether a block that is not loaded counts as loaded: only one asked for
/// in that slot whose load is over (the data lacks it). One never asked is
/// not: the engine asks whether a block is loaded before it asks for it
/// (a cutscene's subtitles, a movie's), and so do scripts.
fn failed(requesting: bool, asked: Option<&str>, name: &str) -> bool {
    !requesting && asked.is_some_and(|a| a.eq_ignore_ascii_case(name))
}

/// Hook for `CTextFile::RequestAdditionalText(char const*, int,
/// CTextFile::eLocationOfTextBlock)`: the block asked for, kept for its slot.
pub(crate) unsafe extern "C" fn text_request(env: *mut Env, name: u64, slot: u32, location: u32) {
    // First the request, which frees the slot of what it held.
    unsafe { orig::text_request(env, name, slot, location) };
    if let Some(asked) = ASKED.lock().unwrap().get_mut(slot as usize) {
        *asked = Some(unsafe { Mem::of(env) }.str(name));
    }
}

/// Hook for `CTextFile::FreeTextSlot(int, bool)`: the slot asks for nothing.
pub(crate) unsafe extern "C" fn text_free(env: *mut Env, slot: u32) {
    if let Some(asked) = ASKED.lock().unwrap().get_mut(slot as usize) {
        *asked = None;
    }
    unsafe { orig::text_free(env, slot) };
}

/// Hook for `CTextFile::HasThisAdditionalTextLoaded(char const*, int)`:
/// yes too for a block asked for whose load ended without it (see `failed`).
pub(crate) unsafe extern "C" fn additional_text_loaded(env: *mut Env, name: u64, slot: u32) -> u32 {
    if unsafe { orig::additional_text_loaded(env, name, slot) } != 0 {
        return 1;
    }
    let requesting = unsafe { orig::additional_text_requesting(env, name, slot) } != 0;
    let asked = ASKED.lock().unwrap().get(slot as usize).cloned().flatten();
    failed(requesting, asked.as_deref(), &unsafe { Mem::of(env) }.str(name)) as u32
}

/// How deep ActionScript calls nest on a thread: the engine keeps 32
/// diagnostic contexts per thread, one per call and a few of its own; no
/// movie of the game's goes near.
const AS_DEPTH: u32 = 24;

thread_local! {
    /// ActionScript calls under way on this thread.
    static AS_NESTED: Cell<u32> = const { Cell::new(0) };
}

/// An ActionScript call, unless `AS_DEPTH` are already under way (see
/// above): then it returns at once, its result undefined. True if it ran.
fn nested(call: impl FnOnce()) -> bool {
    let depth = AS_NESTED.get();
    if depth >= AS_DEPTH {
        return false;
    }
    AS_NESTED.set(depth + 1);
    call();
    AS_NESTED.set(depth);
    true
}

static AS_STOPPED: AtomicBool = AtomicBool::new(false);

fn stopped(env: *mut Env, name: u64) {
    if !AS_STOPPED.swap(true, Ordering::Relaxed) {
        let name = if name == 0 { String::new() } else { unsafe { Mem::of(env) }.str(name) };
        log::warn!("scaleform: ActionScript calls nested {AS_DEPTH} deep, stopped at {name} (a movie older than the scripts)");
    }
}

/// Hook for `GASAsFunctionObject::Invoke(GASFnCall const&, GASLocalFrame*,
/// char const*)`: a function of a movie's ActionScript.
pub(crate) unsafe extern "C" fn as_function_invoke(env: *mut Env, this: u64, call: u64, frame: u64, name: u64) {
    if !nested(|| unsafe { orig::as_function_invoke(env, this, call, frame, name) }) {
        stopped(env, name);
    }
}

/// Hook for `GASUserDefinedFunctionObject::Invoke(...)`: likewise.
pub(crate) unsafe extern "C" fn user_function_invoke(env: *mut Env, this: u64, call: u64, frame: u64, name: u64) {
    if !nested(|| unsafe { orig::user_function_invoke(env, this, call, frame, name) }) {
        stopped(env, name);
    }
}

/// The phone's home screen in its movie: view 1, its apps given by the
/// scripts as `SET_DATA_SLOT(view, slot, icon, notifications, name)`. Apps
/// the movie has no screen for, by icon: the boss agency (57). Their slot
/// goes to view 25 instead (the boss agency's own, which the movie keeps
/// without showing): the home screen has an app less, and as the movie moves
/// the selection over the apps it has, it cannot be chosen.
const PHONE_HOME: f64 = 1.0;
const MISSING_APPS: &[i32] = &[57];
const OFF_HOME: f64 = 25.0;

/// `CScaleformMgr`'s methods being built, one per kind of thread (4), 424
/// bytes each: the movie at +0 (255: none), the parameters at +56 (16 bytes
/// each: a double, its type at +8, 1 for a number), how many at +408.
const METHODS: u64 = 19591280;
const METHOD_SIZE: u64 = 424;
const PARAMS: u64 = 56;
const PARAM_COUNT: u64 = 408;
const NUMBER: u32 = 1;
const GFX_VALUE: u64 = 24;
const GFX_BOOL: u32 = 2;
const GFX_NUMBER: u32 = 3;
const GFX_STRING: u32 = 4;

/// The phone's movie, identified from its request name (with a title-bar fallback).
static PHONE_MOVIE: AtomicU32 = AtomicU32::new(u32::MAX);

/// The phone movie's recent input, selection and app-dispatch events, for the
/// support report: how the game's phone script responds to a direct tap.
/// Bounded; static method/script identifiers and semantic numbers only, with
/// no phone labels, user-entered text or pointer coordinates.
static PHONE_MOVIE_TRACE: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

const PHONE_MOVIE_TRACE_MAX: usize = 192;
const PHONE_SELECTION_RESULTS_MAX: usize = 16;
const PHONE_SLOT_MAP_MAX: usize = 64;
const PHONE_TAP_WINDOW_MS: u128 = 2_500;

#[derive(Clone, Copy)]
struct PhoneTapContext {
    sequence: u64,
    started_at_ms: u128,
    last_activity_ms: u128,
}

static PHONE_TAP_CONTEXT: Mutex<Option<PhoneTapContext>> = Mutex::new(None);
static PHONE_SELECTION_BEGIN_TRACED: Mutex<VecDeque<(u32, u64, u128, i32)>> = Mutex::new(VecDeque::new());
static PHONE_MOVIE_TRACE_DROPPED: AtomicU64 = AtomicU64::new(0);
static PHONE_SELECTION_RESULTS_DROPPED: AtomicU64 = AtomicU64::new(0);
static PHONE_SLOT_MAP_DROPPED: AtomicU64 = AtomicU64::new(0);
static PHONE_MOVIE_FILE: Mutex<Option<(u32, &'static str)>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct PhoneSelectionResult {
    handle: i32,
    movie: u32,
    tap: Option<PhoneTapContext>,
    view: i32,
    ready: Option<bool>,
}

#[derive(Clone, Copy)]
struct PhoneSlot {
    view: i32,
    slot: i32,
    arg3: i32,
}

/// Bounded handles and slot metadata for the phone's asynchronous selection.
static PHONE_SELECTION_RESULTS: Mutex<VecDeque<PhoneSelectionResult>> = Mutex::new(VecDeque::new());
static PHONE_SLOT_MAP: Mutex<VecDeque<PhoneSlot>> = Mutex::new(VecDeque::new());
static PHONE_ACTIVE_VIEW: AtomicI32 = AtomicI32::new(-1);

fn phone_movie_file_name(m: Mem, name: u64) -> Option<&'static str> {
    if name == 0 {
        return None;
    }
    let name = m.str(name).to_ascii_lowercase();
    let file = name.rsplit(|ch| ch == '/' || ch == '\\').next()?;
    let file = file.strip_suffix(".gfx").unwrap_or(file);
    match file {
        "cellphone_ifruit" => Some("cellphone_ifruit"),
        "cellphone_badger" => Some("cellphone_badger"),
        "cellphone_facade" => Some("cellphone_facade"),
        _ => None,
    }
}

fn clear_phone_movie_state() {
    PHONE_SLOT_MAP.lock().unwrap().clear();
    PHONE_SELECTION_RESULTS.lock().unwrap().clear();
    PHONE_ACTIVE_VIEW.store(-1, Ordering::Relaxed);
    PHONE_METHOD.set(None);
    PHONE_SELECTION_PENDING.set(None);
}

fn identify_phone_movie(movie: u32, file: &'static str) {
    let previous_movie = PHONE_MOVIE.swap(movie, Ordering::Relaxed);
    let previous_file = *PHONE_MOVIE_FILE.lock().unwrap();
    let identity = Some((movie, file));
    let changed = previous_movie != movie || previous_file != identity;
    *PHONE_MOVIE_FILE.lock().unwrap() = identity;
    if changed {
        clear_phone_movie_state();
        trace_phone_movie(format!("phone movie identified id={movie} file={file} source=REQUEST_SCALEFORM_MOVIE"));
    }
}

fn phone_movie_file(movie: u32) -> &'static str {
    (*PHONE_MOVIE_FILE.lock().unwrap())
        .filter(|(id, _)| *id == movie)
        .map_or("unknown", |(_, file)| file)
}

fn clear_reused_phone_movie(movie: u32) {
    if PHONE_MOVIE.compare_exchange(movie, u32::MAX, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
        *PHONE_MOVIE_FILE.lock().unwrap() = None;
        clear_phone_movie_state();
        trace_phone_movie(format!("phone movie identity cleared id={movie} reason=movie_id_reused"));
    }
}

fn trace_phone_movie(line: String) {
    let mut trace = PHONE_MOVIE_TRACE.lock().unwrap();
    if trace.len() == PHONE_MOVIE_TRACE_MAX {
        trace.pop_front();
        PHONE_MOVIE_TRACE_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
    trace.push_back(format!("+{}ms {line}", crate::diagnostics_elapsed_ms()));
}

fn trace_phone_tap(tap: Option<PhoneTapContext>, line: String) {
    let line = match tap {
        Some(tap) => {
            let age_ms = crate::diagnostics_elapsed_ms().saturating_sub(tap.started_at_ms);
            format!("tap_seq_id={} tap_age_ms={age_ms} {line}", tap.sequence)
        }
        None => line,
    };
    trace_phone_movie(line);
}

pub(crate) fn note_phone_touch_tap(sequence: u64, forwarded: bool) {
    let now = crate::diagnostics_elapsed_ms();
    if forwarded {
        *PHONE_TAP_CONTEXT.lock().unwrap() = Some(PhoneTapContext {
            sequence,
            started_at_ms: now,
            last_activity_ms: now,
        });
        let tap = PhoneTapContext { sequence, started_at_ms: now, last_activity_ms: now };
        let movie = PHONE_MOVIE.load(Ordering::Relaxed);
        let movie_label = if movie == u32::MAX { "unknown".to_owned() } else { movie.to_string() };
        let file = phone_movie_file(movie);
        let view = PHONE_ACTIVE_VIEW.load(Ordering::Relaxed);
        let view_label = if view >= 0 { view.to_string() } else { "unknown".to_owned() };
        let slots = if view >= 0 { phone_view_slots(view) } else { "view_slots=unknown".to_owned() };
        trace_phone_tap(
            Some(tap),
            format!(
                "input=mouse_left_down forwarded=true phone_movie_id={movie_label} file={file} active_view={view_label} {slots}"
            ),
        );
    } else {
        clear_phone_touch_context("mouse_down_not_forwarded");
        trace_phone_movie(format!("tap_seq_id={sequence} input=mouse_left_down forwarded=false"));
    }
}

pub(crate) fn clear_phone_touch_context(reason: &'static str) {
    let previous = PHONE_TAP_CONTEXT.lock().unwrap().take();
    if let Some(tap) = previous {
        trace_phone_tap(Some(tap), format!("tap_context=cleared reason={reason}"));
    }
}

fn phone_tap_context_is_recent(tap: PhoneTapContext, now_ms: u128) -> bool {
    now_ms.saturating_sub(tap.last_activity_ms) <= PHONE_TAP_WINDOW_MS
}

fn recent_phone_tap_context() -> Option<PhoneTapContext> {
    let now = crate::diagnostics_elapsed_ms();
    (*PHONE_TAP_CONTEXT.lock().unwrap()).filter(|&tap| phone_tap_context_is_recent(tap, now))
}

fn trace_phone_selection_begin(tap: PhoneTapContext, movie: u32, view: i32, file: &str) {
    const LIMIT: usize = 32;
    let key = (movie, tap.sequence, tap.started_at_ms, view);
    let mut seen = PHONE_SELECTION_BEGIN_TRACED.lock().unwrap();
    if seen.contains(&key) {
        return;
    }
    if seen.len() == LIMIT {
        seen.pop_front();
    }
    seen.push_back(key);
    drop(seen);
    let view_label = if view >= 0 { view.to_string() } else { "unknown".to_owned() };
    let slots = if view >= 0 { phone_view_slots(view) } else { "view_slots=unknown".to_owned() };
    trace_phone_tap(
        Some(tap),
        format!("phone movie id={movie} file={file}: GET_CURRENT_SELECTION begin active_view={view_label} {slots}"),
    );
}

fn refresh_phone_tap_context(tap: PhoneTapContext) -> bool {
    let now = crate::diagnostics_elapsed_ms();
    let mut current = PHONE_TAP_CONTEXT.lock().unwrap();
    let Some(active) = current.as_mut() else { return false };
    if active.sequence != tap.sequence || active.started_at_ms != tap.started_at_ms {
        return false;
    }
    active.last_activity_ms = now;
    true
}

fn remember_phone_slot(view: i32, slot: i32, arg3: i32) {
    let mut slots = PHONE_SLOT_MAP.lock().unwrap();
    if let Some(index) = slots.iter().position(|entry| entry.view == view && entry.slot == slot) {
        slots.remove(index);
    }
    if slots.len() == PHONE_SLOT_MAP_MAX {
        slots.pop_front();
        PHONE_SLOT_MAP_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
    slots.push_back(PhoneSlot { view, slot, arg3 });
}

fn forget_phone_slot(view: i32, slot: i32) {
    let mut slots = PHONE_SLOT_MAP.lock().unwrap();
    if let Some(index) = slots.iter().position(|entry| entry.view == view && entry.slot == slot) {
        slots.remove(index);
    }
}

fn phone_slot_arg3(view: i32, slot: i32) -> Option<i32> {
    PHONE_SLOT_MAP
        .lock()
        .unwrap()
        .iter()
        .find(|entry| entry.view == view && entry.slot == slot)
        .map(|entry| entry.arg3)
}

fn format_phone_view_slots(mut slots: Vec<(i32, i32)>, dropped: u64) -> String {
    slots.sort_unstable_by_key(|&(slot, _)| slot);
    if slots.is_empty() {
        "view_slots=unknown".to_owned()
    } else {
        let slots = slots.iter().map(|(slot, arg3)| format!("{slot}:{arg3}")).collect::<Vec<_>>().join(",");
        if dropped == 0 {
            format!("view_slots=[{slots}]")
        } else {
            format!("view_slots=[{slots}] slot_map_dropped={dropped}")
        }
    }
}

fn phone_view_slots(view: i32) -> String {
    let slots = PHONE_SLOT_MAP
        .lock()
        .unwrap()
        .iter()
        .filter(|entry| entry.view == view)
        .map(|entry| (entry.slot, entry.arg3))
        .collect::<Vec<_>>();
    format_phone_view_slots(slots, PHONE_SLOT_MAP_DROPPED.load(Ordering::Relaxed))
}

fn remember_phone_selection_result(handle: i32, movie: u32, tap: Option<PhoneTapContext>, view: i32) {
    let mut results = PHONE_SELECTION_RESULTS.lock().unwrap();
    if let Some(index) = results.iter().position(|result| result.handle == handle && result.movie == movie) {
        results.remove(index);
    }
    if results.len() == PHONE_SELECTION_RESULTS_MAX {
        results.pop_front();
        PHONE_SELECTION_RESULTS_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
    results.push_back(PhoneSelectionResult { handle, movie, tap, view, ready: None });
}

fn note_phone_selection_ready(handle: i32, ready: bool) -> Option<(u32, Option<PhoneTapContext>, bool)> {
    let current_movie = PHONE_MOVIE.load(Ordering::Relaxed);
    let mut results = PHONE_SELECTION_RESULTS.lock().unwrap();
    let result = results.iter_mut().find(|result| result.handle == handle && result.movie == current_movie)?;
    let changed = result.ready != Some(ready);
    result.ready = Some(ready);
    Some((result.movie, result.tap, changed))
}

fn phone_selection_result(handle: i32) -> Option<(u32, Option<PhoneTapContext>, i32, Option<bool>)> {
    let current_movie = PHONE_MOVIE.load(Ordering::Relaxed);
    PHONE_SELECTION_RESULTS
        .lock()
        .unwrap()
        .iter()
        .find(|result| result.handle == handle && result.movie == current_movie)
        .map(|result| (result.movie, result.tap, result.view, result.ready))
}

fn forget_phone_selection_result(handle: i32) {
    let mut results = PHONE_SELECTION_RESULTS.lock().unwrap();
    if let Some(index) = results.iter().position(|result| result.handle == handle) {
        results.remove(index);
    }
}

/// Recent phone-tap, movie-selection and app-script events, oldest first.
pub fn phone_movie_diagnostics() -> Vec<String> {
    let mut events = PHONE_MOVIE_TRACE.lock().unwrap().iter().cloned().collect::<Vec<_>>();
    let dropped = PHONE_MOVIE_TRACE_DROPPED.load(Ordering::Relaxed);
    let now = crate::diagnostics_elapsed_ms();
    if dropped != 0 {
        events.push(format!("+{now}ms phone_movie_trace dropped={dropped}"));
    }
    let results_dropped = PHONE_SELECTION_RESULTS_DROPPED.load(Ordering::Relaxed);
    if results_dropped != 0 {
        events.push(format!("+{now}ms phone_selection_handles dropped={results_dropped}"));
    }
    let slots_dropped = PHONE_SLOT_MAP_DROPPED.load(Ordering::Relaxed);
    if slots_dropped != 0 {
        events.push(format!("+{now}ms phone_view_slot_map dropped={slots_dropped}"));
    }
    events
}

#[derive(Clone, Copy)]
enum PhoneMethodKind {
    Input,
    View,
    DataSlot,
    EmptySlot,
}

#[derive(Clone, Copy)]
struct PhoneMethodTrace {
    kind: PhoneMethodKind,
    int_count: u32,
    first: i32,
    second: i32,
    tap: Option<PhoneTapContext>,
}

impl PhoneMethodTrace {
    const fn new(kind: PhoneMethodKind, tap: Option<PhoneTapContext>) -> Self {
        Self { kind, int_count: 0, first: 0, second: 0, tap }
    }
}

thread_local! {
    /// A phone Scaleform method whose integer parameters are being traced.
    static PHONE_METHOD: Cell<Option<PhoneMethodTrace>> = const { Cell::new(None) };
    /// Phone movie, tap context and active view for the pending selection handle.
    static PHONE_SELECTION_PENDING: Cell<Option<(u32, Option<PhoneTapContext>, i32)>> = const { Cell::new(None) };
}

/// Hook for `CScaleformMgr::BeginMethodInternal(int, eSCALEFORM_BASE_CLASS,
/// char const*, int, int)`: every Scaleform method, the scripts' and the
/// code's.
pub(crate) unsafe extern "C" fn method_begin(env: *mut Env, movie: u32, base: u32, name: u64, a: u32, b: u32) -> u32 {
    let m = unsafe { Mem::of(env) };
    let is = |s: &[u8]| m.bytes(name, s.len()) == s;
    if is(b"SET_TITLEBAR_TIME\0") && PHONE_MOVIE.load(Ordering::Relaxed) == u32::MAX {
        // Fallback only: request-time filename recognition below is the
        // stronger identity signal. Keep the source explicit in the trace.
        let previous = PHONE_MOVIE.swap(movie, Ordering::Relaxed);
        *PHONE_MOVIE_FILE.lock().unwrap() = None;
        if previous != movie {
            clear_phone_movie_state();
        }
        trace_phone_movie(format!("phone movie identified id={movie} file=unknown source=SET_TITLEBAR_TIME_fallback"));
    }
    let on_phone = movie == PHONE_MOVIE.load(Ordering::Relaxed);
    PHONE_SELECTION_PENDING.set(None);
    let tap = recent_phone_tap_context();
    // The phone script's own selection query and its surrounding methods,
    // correlated to the most recent delivered phone tap when one is close.
    if on_phone && is(b"GET_CURRENT_SELECTION\0") {
        if let Some(tap) = tap {
            let view = PHONE_ACTIVE_VIEW.load(Ordering::Relaxed);
            let file = phone_movie_file(movie);
            trace_phone_selection_begin(tap, movie, view, file);
            PHONE_SELECTION_PENDING.set(Some((movie, Some(tap), view)));
        }
        PHONE_METHOD.set(None);
    } else if on_phone && is(b"SET_INPUT_EVENT\0") && tap.is_some() {
        PHONE_METHOD.set(Some(PhoneMethodTrace::new(PhoneMethodKind::Input, tap)));
    } else if on_phone && is(b"DISPLAY_VIEW\0") {
        PHONE_METHOD.set(Some(PhoneMethodTrace::new(PhoneMethodKind::View, tap)));
    } else if on_phone && is(b"SET_DATA_SLOT\0") {
        PHONE_METHOD.set(Some(PhoneMethodTrace::new(PhoneMethodKind::DataSlot, tap)));
    } else if on_phone && is(b"SET_DATA_SLOT_EMPTY\0") {
        PHONE_METHOD.set(Some(PhoneMethodTrace::new(PhoneMethodKind::EmptySlot, tap)));
    } else {
        PHONE_METHOD.set(None);
    }
    unsafe { orig::method_begin(env, movie, base, name, a, b) }
}

/// Hook for `scrWrapped_END_SCALEFORM_MOVIE_METHOD_RETURN_VALUE::Call`:
/// the phone's asynchronous selection method's return handle.
pub(crate) unsafe extern "C" fn scaleform_method_return_end(env: *mut Env, info: u64) {
    let pending = PHONE_SELECTION_PENDING.with(|pending| pending.take());
    unsafe { orig::scaleform_method_return_end(env, info) };
    let Some((movie, tap, view)) = pending else { return };
    let m = unsafe { Mem::of(env) };
    let result = m.u64(info);
    if result == 0 {
        trace_phone_tap(tap, format!("phone movie id={movie}: GET_CURRENT_SELECTION handle unavailable"));
        return;
    }
    let handle = m.i32(result);
    remember_phone_selection_result(handle, movie, tap, view);
}

/// Hook for `scrWrapped_IS_SCALEFORM_MOVIE_METHOD_RETURN_VALUE_READY::Call`:
/// observe readiness for a tracked phone selection result.
pub(crate) unsafe extern "C" fn scaleform_method_return_ready(env: *mut Env, info: u64) {
    unsafe { orig::scaleform_method_return_ready(env, info) };
    let m = unsafe { Mem::of(env) };
    let result = m.u64(info);
    let argument = m.u64(info + 16);
    if result == 0 || argument == 0 {
        return;
    }
    let handle = m.i32(argument);
    let ready = m.u32(result) != 0;
    if let Some((movie, tap, changed)) = note_phone_selection_ready(handle, ready)
        && changed
        && ready
    {
        trace_phone_tap(
            tap,
            format!("phone movie id={movie}: GET_CURRENT_SELECTION handle={handle} ready=1"),
        );
    }
}

/// Hook for `scrWrapped_GET_SCALEFORM_MOVIE_METHOD_RETURN_VALUE_INT::Call`:
/// read the phone's selection result after the original native, without
/// changing the value returned to the game script.
pub(crate) unsafe extern "C" fn scaleform_method_return_int(env: *mut Env, info: u64) {
    unsafe { orig::scaleform_method_return_int(env, info) };
    let m = unsafe { Mem::of(env) };
    let result = m.u64(info);
    let argument = m.u64(info + 16);
    if result == 0 || argument == 0 {
        return;
    }
    let handle = m.i32(argument);
    let Some((movie, tap, view, ready_state)) = phone_selection_result(handle) else { return };
    if ready_state == Some(false) {
        return;
    }
    let value = m.i32(result);
    let ready = if ready_state == Some(true) { "1" } else { "unknown" };
    let tap_active = ready_state == Some(true) && tap.is_some_and(refresh_phone_tap_context);
    let view_label = if view >= 0 { view.to_string() } else { "unknown".to_owned() };
    let selection_arg3 = if view >= 0 {
        phone_slot_arg3(view, value).map_or_else(|| "selection_arg3=unknown".to_owned(), |arg3| format!("selection_arg3={arg3}"))
    } else {
        "selection_arg3=unknown".to_owned()
    };
    let slots = if view >= 0 { phone_view_slots(view) } else { "view_slots=unknown".to_owned() };
    let file = phone_movie_file(movie);
    trace_phone_tap(
        tap,
        format!(
            "phone movie id={movie} file={file}: GET_CURRENT_SELECTION handle={handle} ready={ready} selection={value} active_view={view_label} {selection_arg3} {slots} tap_context_active={}",
            tap_active as u8
        ),
    );
    forget_phone_selection_result(handle);
}

/// Hook for `CScaleformMgr::AddParamInt(int)`: phone selection methods and
/// the view/slot/icon numbers needed to interpret the asynchronous result.
pub(crate) unsafe extern "C" fn param_int(env: *mut Env, value: u32) {
    if let Some(mut method) = PHONE_METHOD.get() {
        let value = value as i32;
        match method.kind {
            PhoneMethodKind::Input if method.int_count == 0 => {
                if let Some(tap) = method.tap {
                    trace_phone_tap(Some(tap), format!("phone movie: SET_INPUT_EVENT({value})"));
                }
            }
            PhoneMethodKind::View => match method.int_count {
                0 => {
                    method.first = value;
                    PHONE_ACTIVE_VIEW.store(value, Ordering::Relaxed);
                    if let Some(tap) = method.tap {
                        trace_phone_tap(Some(tap), format!("phone movie: DISPLAY_VIEW view={value}"));
                    }
                }
                1 => {
                    method.second = value;
                    if let Some(tap) = method.tap {
                        trace_phone_tap(
                            Some(tap),
                            format!("phone movie: DISPLAY_VIEW view={} argument2={}", method.first, method.second),
                        );
                    }
                }
                _ => {}
            },
            PhoneMethodKind::DataSlot => match method.int_count {
                0 => method.first = value,
                1 => method.second = value,
                2 => {
                    let view = if MISSING_APPS.contains(&value) && off_home(unsafe { Mem::of(env) }) {
                        OFF_HOME as i32
                    } else {
                        method.first
                    };
                    remember_phone_slot(view, method.second, value);
                }
                _ => {}
            },
            PhoneMethodKind::EmptySlot => match method.int_count {
                0 => method.first = value,
                1 => {
                    method.second = value;
                    forget_phone_slot(method.first, method.second);
                }
                _ => {}
            },
            _ => {}
        }
        method.int_count += 1;
        PHONE_METHOD.set(Some(method));
    }
    unsafe { orig::param_int(env, value as u32) }
}

fn phone_action_method(method: &str) -> bool {
    // Keep this hook to DISPLAY_VIEW arguments and app-dispatch calls rather
    // than every SET_DATA_SLOT or similarly named Scaleform method.
    matches!(method, "DISPLAY_VIEW" | "LOAD_APP" | "CELLPHONE_APP" | "OPEN_APP" | "SET_APP" | "SET_CURRENT_APP")
}

fn safe_phone_token(m: Mem, value: u64) -> Option<String> {
    if m.u32(value + 8) & 0x8F != GFX_STRING {
        return None;
    }
    let flags = m.u8(value + 8);
    let mut at = m.u64(value + 16);
    if at == 0 {
        return None;
    }
    if flags & 0x40 != 0 {
        at = m.u64(at);
    }
    if at == 0 {
        return None;
    }
    let bytes = m.bytes(at, 49);
    let end = bytes.iter().position(|&byte| byte == 0)?;
    if end == 0 || end > 48 {
        return None;
    }
    let token = std::str::from_utf8(&bytes[..end]).ok()?;
    let upper = token.to_ascii_uppercase();
    let safe = token.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        && ["CELL_", "APP_", "ICON_", "CELLPHONE_"]
            .iter()
            .any(|prefix| upper.starts_with(prefix));
    safe.then(|| token.to_owned())
}

fn phone_call_arguments(m: Mem, method: &str, args: u64, count: u32) -> String {
    let numeric_method = matches!(
        method,
        "DISPLAY_VIEW" | "SET_DATA_SLOT" | "SET_DATA_SLOT_EMPTY" | "SET_INPUT_EVENT"
    );
    if !numeric_method {
        let identifiers = (0..count.min(6) as u64)
            .filter_map(|i| safe_phone_token(m, args + i * GFX_VALUE))
            .collect::<Vec<_>>();
        return if identifiers.is_empty() {
            format!("argc={count}")
        } else {
            format!("argc={count} ids=[{}]", identifiers.join(","))
        };
    }
    let argument_limit = match method {
        "DISPLAY_VIEW" => 2,
        "SET_DATA_SLOT" => 3,
        "SET_DATA_SLOT_EMPTY" => 2,
        "SET_INPUT_EVENT" => 1,
        _ => 0,
    };
    let values = (0..count.min(argument_limit) as u64)
        .map(|i| {
            let value = args + i * GFX_VALUE;
            let kind = m.u32(value + 8) & 0x8F;
            match kind {
                GFX_NUMBER => {
                    let number = m.f64(value + 16);
                    if number.is_finite()
                        && number.fract() == 0.0
                        && (i32::MIN as f64..=i32::MAX as f64).contains(&number)
                    {
                        (number as i32).to_string()
                    } else {
                        "number".to_owned()
                    }
                }
                GFX_BOOL => format!("bool:{}", m.u8(value + 16) != 0),
                GFX_STRING => safe_phone_token(m, value).unwrap_or_else(|| "text".to_owned()),
                _ => "other".to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("args=[{values}]")
}

/// Record only semantic phone-movie calls near a delivered direct tap.
/// String contents are omitted except for static app/icon identifiers; no
/// pointer coordinates or user-entered text are included.
pub(crate) fn trace_phone_scaleform_call(env: *mut Env, movie: u32, name: u64, args: u64, count: u32, instantly: u32) {
    if movie != PHONE_MOVIE.load(Ordering::Relaxed) || name == 0 || (count != 0 && args == 0) {
        return;
    }
    let Some(tap) = recent_phone_tap_context() else { return };
    let m = unsafe { Mem::of(env) };
    let method_bytes = m.bytes(name, 28);
    let method_end = method_bytes.iter().position(|&byte| byte == 0).unwrap_or(method_bytes.len());
    let Ok(method) = std::str::from_utf8(&method_bytes[..method_end]) else { return };
    if !phone_action_method(method) {
        return;
    }
    let arguments = phone_call_arguments(m, method, args, count);
    trace_phone_tap(
        Some(tap),
        format!(
            "phone movie id={movie} file={}: dispatch={method} instantly={instantly} {arguments}",
            phone_movie_file(movie)
        ),
    );
}

/// The `SET_DATA_SLOT` being built for the home screen (its view and slot
/// given), moved to `OFF_HOME`. False if no one method is that.
fn off_home(m: Mem) -> bool {
    let number = |s: u64, i: u64| (m.u32(s + PARAMS + i * 16 + 8) == NUMBER).then(|| m.f64(s + PARAMS + i * 16));
    let mut building = (0..4).map(|i| METHODS + i * METHOD_SIZE).filter(|&s| {
        m.u8(s) != 255 && m.u8(s + PARAM_COUNT) == 2 && number(s, 0) == Some(PHONE_HOME) && number(s, 1).is_some()
    });
    match (building.next(), building.next()) {
        (Some(s), None) => {
            m.set_u64(s + PARAMS, OFF_HOME.to_bits());
            true
        }
        _ => false,
    }
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.method_begin = Some(method_begin);
    hooks.param_int = Some(param_int);
    hooks.scaleform_method_return_end = Some(scaleform_method_return_end);
    hooks.scaleform_method_return_ready = Some(scaleform_method_return_ready);
    hooks.scaleform_method_return_int = Some(scaleform_method_return_int);
    hooks.as_function_invoke = Some(as_function_invoke);
    hooks.user_function_invoke = Some(user_function_invoke);
    hooks.script_model_loaded = Some(model_loaded);
    hooks.interior_of_type = Some(interior_of_type);
    hooks.create_object = Some(create_object_offset);
    hooks.create_object_no_offset = Some(create_object_no_offset);
    hooks.additional_text_loaded = Some(additional_text_loaded);
    hooks.text_request = Some(text_request);
    hooks.text_free = Some(text_free);
    hooks.script_bank_request = Some(bank_request);
    hooks.bank_loading_status = Some(bank_loading_status);
    hooks.script_named_bank_request = Some(named_bank_request);
    hooks.script_txd_loaded = Some(txd_loaded);
    hooks.anim_dict_loaded = Some(anim_dict_loaded);
    hooks.clip_set_loaded = Some(clip_set_loaded);
    hooks.named_ptfx_loaded = Some(named_ptfx_loaded);
    hooks.scaleform_request = Some(scaleform_request);
    hooks.scaleform_loaded = Some(scaleform_loaded);
    hooks.scaleform_file_loaded = Some(scaleform_file_loaded);
    hooks.cutscene_request = Some(cutscene_request);
    hooks.cutscene_request_list = Some(cutscene_request_list);
    hooks.cutscene_loaded = Some(cutscene_loaded);
    hooks.this_cutscene_loaded = Some(this_cutscene_loaded);
    hooks.script_request = Some(script_request);
    hooks.script_loaded = Some(script_loaded);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The phone's slot for an app its movie lacks leaves the home screen:
    /// the one method being built with the home view and a slot gets the
    /// other view; any other method stays as it is.
    #[test]
    fn an_app_the_phone_lacks_leaves_its_home_screen() {
        let mut buf = vec![0u8; (METHODS + 4 * METHOD_SIZE) as usize];
        let m = Mem::over(&mut buf);
        let state = |i: u64| METHODS + i * METHOD_SIZE;
        let number = |s: u64, i: u64, v: f64| {
            m.set_u64(s + PARAMS + i * 16, v.to_bits());
            m.set_u32(s + PARAMS + i * 16 + 8, NUMBER);
        };
        for i in 0..4 {
            m.set_u8(state(i), 255);
        }
        // Another thread builds a method of another movie's.
        m.set_u8(state(1), 3);
        m.set_u8(state(1) + PARAM_COUNT, 2);
        number(state(1), 0, 7.0);
        number(state(1), 1, 8.0);
        assert!(!off_home(m), "nothing for the home screen");
        // The phone's home screen, slot 8.
        m.set_u8(state(2), 6);
        m.set_u8(state(2) + PARAM_COUNT, 2);
        number(state(2), 0, PHONE_HOME);
        number(state(2), 1, 8.0);
        assert!(off_home(m));
        assert_eq!(m.f64(state(2) + PARAMS), OFF_HOME);
        assert_eq!(m.f64(state(1) + PARAMS), 7.0, "the other one untouched");
    }

    #[test]
    fn phone_script_trace_keeps_only_static_phone_app_identifiers() {
        let mut buf = vec![0u8; 512];
        let m = Mem::over(&mut buf);
        m.write_str(8, "appContacts", 64);
        m.write_str(80, "cellphone_ifruit", 64);
        m.write_str(160, "am_mission", 64);
        m.write_str(240, "app private text", 64);

        assert_eq!(phone_app_script_name(m, 8).as_deref(), Some("appContacts"));
        assert_eq!(phone_app_script_name(m, 80).as_deref(), Some("cellphone_ifruit"));
        assert_eq!(phone_app_script_name(m, 160), None);
        assert_eq!(phone_app_script_name(m, 240), None, "free text is never retained");
    }

    #[test]
    fn phone_movie_identity_is_recognized_at_request_time_from_its_file_name() {
        let mut buf = vec![0u8; 128];
        let m = Mem::over(&mut buf);
        m.write_str(8, "movies/CELLphone_IFruit.gfx", 64);
        assert_eq!(phone_movie_file_name(m, 8), Some("cellphone_ifruit"));
        m.write_str(8, "scaleforms\\cellphone_badger.gfx", 64);
        assert_eq!(phone_movie_file_name(m, 8), Some("cellphone_badger"));
        m.write_str(8, "pause_menu.gfx", 64);
        assert_eq!(phone_movie_file_name(m, 8), None);
    }

    #[test]
    fn phone_tap_correlation_has_a_bounded_activity_window() {
        let tap = PhoneTapContext { sequence: 7, started_at_ms: 100, last_activity_ms: 200 };
        assert!(phone_tap_context_is_recent(tap, 2_700), "the boundary is inclusive");
        assert!(!phone_tap_context_is_recent(tap, 2_701), "expired contexts are not reused");
        assert_eq!(tap.started_at_ms, 100, "refreshing activity does not rewrite the original tap age");
    }

    #[test]
    fn phone_view_slot_summary_is_sorted_and_marks_truncation() {
        assert_eq!(format_phone_view_slots(vec![(3, 57), (1, 12)], 0), "view_slots=[1:12,3:57]");
        assert_eq!(format_phone_view_slots(vec![(1, 12)], 4), "view_slots=[1:12] slot_map_dropped=4");
        assert_eq!(format_phone_view_slots(Vec::new(), 0), "view_slots=unknown");
    }

    #[test]
    fn generic_scaleform_trace_keeps_view_and_app_dispatch_not_slot_churn() {
        assert!(phone_action_method("LOAD_APP"));
        assert!(phone_action_method("CELLPHONE_APP"));
        assert!(!phone_action_method("SET_DATA_SLOT"));
        assert!(phone_action_method("DISPLAY_VIEW"));
        assert!(!phone_action_method("GET_CURRENT_SELECTION"));
    }

    #[test]
    fn phone_movie_trace_limits_strings_and_numeric_fields() {
        let mut buf = vec![0u8; 512];
        let m = Mem::over(&mut buf);
        m.set_u32(8, GFX_STRING | 0x40);
        m.set_u64(24, 64);
        m.set_u64(64, 128);
        m.write_str(128, "CELL_CONTACTS", 49);
        assert_eq!(safe_phone_token(m, 8).as_deref(), Some("CELL_CONTACTS"));
        m.write_str(128, "Private contact", 49);
        assert_eq!(safe_phone_token(m, 8), None);

        let args = 256;
        for index in 0..4 {
            let value = args + index * GFX_VALUE;
            m.set_u32(value + 8, GFX_NUMBER);
            m.set_u64(value + 16, ((index + 1) as f64).to_bits());
        }
        assert_eq!(phone_call_arguments(m, "SET_DATA_SLOT", args, 4), "args=[1,2,3]");
        assert_eq!(phone_call_arguments(m, "UNKNOWN_METHOD", args, 4), "argc=4");
    }

    /// ActionScript that calls itself without end (the phone's LOAD_APP and
    /// CELLPHONE_APP for an app its movie lacks) stops at AS_DEPTH instead of
    /// running the engine out of diagnostic contexts.
    #[test]
    fn actionscript_calling_itself_stops() {
        fn call(calls: &mut u32) {
            *calls += 1;
            nested(|| call(calls));
        }
        let mut calls = 0;
        assert!(nested(|| call(&mut calls)));
        assert_eq!(calls, AS_DEPTH, "each nested call ran once, up to the limit");
        assert_eq!(AS_NESTED.get(), 0, "unwound");
        // Calls in turn (not nested) are not limited.
        for _ in 0..100 {
            assert!(nested(|| ()));
        }
    }

    #[test]
    fn names_hash_as_the_engine_does() {
        // The heists' apartment door, as the apartment's script asks for it.
        assert_eq!(name_hash("hei_heist_apart2_door"), 34120519);
        assert_eq!(name_hash("HEI_HEIST_APART2_DOOR"), 34120519);
    }

    #[test]
    fn only_a_block_asked_for_counts_as_loaded_when_its_load_failed() {
        // Never asked: the engine and scripts ask before they request it.
        assert!(!failed(false, None, "MPAWD"));
        // Another block asked in that slot.
        assert!(!failed(false, Some("FMMC"), "MPAWD"));
        // Asked, still loading.
        assert!(!failed(true, Some("MPAWD"), "MPAWD"));
        // Asked, and its load ended without it.
        assert!(failed(false, Some("mpawd"), "MPAWD"));
    }
}
