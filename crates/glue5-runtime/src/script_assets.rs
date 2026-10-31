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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

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
    if !for_missing_place(unsafe { Mem::of(env) }, name) {
        unsafe { orig::script_request(env, name) };
    }
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
    if movie == 0 && name != 0 && !movie_in_image(env, &unsafe { Mem::of(env) }.str(name)) {
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

/// The phone's movie: the one its title bar's time goes to.
static PHONE_MOVIE: AtomicU32 = AtomicU32::new(u32::MAX);

/// The phone movie's recent input and view methods, for the support
/// report: how the game's own phone script drives the movie (selection
/// steps, views shown, selections asked for), so a tap that opens the
/// wrong app can be told from what the script sent. Bounded; method names
/// and numbers only, nothing the player typed or saw.
static PHONE_MOVIE_TRACE: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

fn trace_phone_movie(line: String) {
    let mut trace = PHONE_MOVIE_TRACE.lock().unwrap();
    if trace.len() == 64 {
        trace.pop_front();
    }
    trace.push_back(format!("+{}ms {line}", crate::diagnostics_elapsed_ms()));
}

/// The phone movie's recent input and view methods, oldest first.
pub fn phone_movie_diagnostics() -> Vec<String> {
    PHONE_MOVIE_TRACE.lock().unwrap().iter().cloned().collect()
}

thread_local! {
    /// A `SET_DATA_SLOT` for the phone is being built on this thread: the
    /// numbers given it so far.
    static PHONE_SLOT: Cell<Option<u32>> = const { Cell::new(None) };
    /// A phone method being built whose numbers the trace wants: its kind
    /// (1 `SET_INPUT_EVENT`, 2 `DISPLAY_VIEW`) and how many numbers it has
    /// had, with the first kept (`DISPLAY_VIEW`'s view, for its second).
    static PHONE_METHOD: Cell<Option<(u8, u32, i32)>> = const { Cell::new(None) };
}

/// Hook for `CScaleformMgr::BeginMethodInternal(int, eSCALEFORM_BASE_CLASS,
/// char const*, int, int)`: every Scaleform method, the scripts' and the
/// code's.
pub(crate) unsafe extern "C" fn method_begin(env: *mut Env, movie: u32, base: u32, name: u64, a: u32, b: u32) -> u32 {
    let m = unsafe { Mem::of(env) };
    let is = |s: &[u8]| m.bytes(name, s.len()) == s;
    if is(b"SET_TITLEBAR_TIME\0") {
        PHONE_MOVIE.store(movie, Ordering::Relaxed);
    }
    let on_phone = movie == PHONE_MOVIE.load(Ordering::Relaxed);
    PHONE_SLOT.set((is(b"SET_DATA_SLOT\0") && on_phone).then_some(0));
    // The phone script's own driving of its movie, for the trace: the
    // selection it asks for (a tap or Select landed), and below, in
    // `param_int`, the steps it sends and the views it shows.
    if on_phone && is(b"GET_CURRENT_SELECTION\0") {
        trace_phone_movie(format!("phone movie {movie}: GET_CURRENT_SELECTION"));
        PHONE_METHOD.set(None);
    } else if on_phone && is(b"SET_INPUT_EVENT\0") {
        PHONE_METHOD.set(Some((1, 0, 0)));
    } else if on_phone && is(b"DISPLAY_VIEW\0") {
        PHONE_METHOD.set(Some((2, 0, 0)));
    } else {
        PHONE_METHOD.set(None);
    }
    unsafe { orig::method_begin(env, movie, base, name, a, b) }
}

/// Hook for `CScaleformMgr::AddParamInt(int)`: the third number of the
/// phone's `SET_DATA_SLOT` is the app's icon.
pub(crate) unsafe extern "C" fn param_int(env: *mut Env, value: u32) {
    if let Some(n) = PHONE_SLOT.get() {
        PHONE_SLOT.set(Some(n + 1));
        if n == 2 && MISSING_APPS.contains(&(value as i32)) {
            off_home(unsafe { Mem::of(env) });
        }
    }
    if let Some((kind, n, first)) = PHONE_METHOD.get() {
        PHONE_METHOD.set(Some((kind, n + 1, if n == 0 { value as i32 } else { first })));
        match (kind, n) {
            // The phone's steps and choices: 1 up, 2 right, 3 down, 4 left
            // (the movie's own numbering, as its script sends them).
            (1, 0) => trace_phone_movie(format!("phone movie: SET_INPUT_EVENT({})", value as i32)),
            (2, 1) => trace_phone_movie(format!("phone movie: DISPLAY_VIEW({first}, {})", value as i32)),
            _ => {}
        }
    }
    unsafe { orig::param_int(env, value) }
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
