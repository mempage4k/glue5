//! The application's own UI, drawn with egui over the game (before the game
//! runs, alone): the start page, the settings, the credits, and on touch
//! screens the settings button and the touch controls. The game's menus
//! stay the game's.
//!
//! One responsive layout, fitted to the safe area. The same widgets
//! everywhere, all reachable from a controller or a keyboard: the D-pad,
//! left stick or arrows move the focus, A / Space / Enter activates, B /
//! Esc goes back.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{
    Align2, Color32, FontId, Id, LayerId, Order, Pos2, Rect, SafeAreaInsets, Stroke, Vec2, ViewportId, pos2, vec2,
};
use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use sdl3::mouse::MouseButton;
use sdl3::video::Window;

use crate::settings::{PerfOverlay, Settings};
use crate::sfx::{self, Sound};

mod chip;
mod credits;
mod friends;
mod icons;
mod menus;
mod mods;
mod menu;
mod pages;
mod perf;
mod popup;
mod scenes;
mod servers;
mod shell;
mod spinner;
mod stage;
mod theme;
mod widgets;
use credits::credits_page;
use friends::{FriendsPage, friends_page};
use mods::{ModsPage, mods_page};
pub use mods::StudioInfo;
use pages::settings_page;
use servers::{ServersPage, servers_page};

use crate::touch::Touch;

/// A page of the application's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// Before the game: its files, the ways into it, Settings, Credits.
    Start,
    Settings,
    /// Who made what the application is built on, and their licenses.
    Credits,
    /// The player's friend code, requests, friends.
    Friends,
    /// Where the game's files stream from: the server in use, the ones on
    /// the network, one by its address, this device's copy.
    Servers,
    /// The mods, GLUE 5 Studio, the mods' log.
    Mods,
}

/// How the game starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Story,
    /// The whole map, free roam, no missions.
    Sandbox,
    /// GTA Online's freemode, with the lobby's other players.
    Online,
    /// The game's fly-through of the map, its results on the start page.
    Benchmark,
}

/// A timestamped observation the player can add to the GLUE 5 process log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugMarker {
    ScreenFrozen,
    AudioContinues,
    ControlsRespond,
    ControlsUnresponsive,
}

impl DebugMarker {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::ScreenFrozen => "SCREEN_FROZEN",
            Self::AudioContinues => "AUDIO_CONTINUES",
            Self::ControlsRespond => "CONTROLS_RESPOND",
            Self::ControlsUnresponsive => "CONTROLS_UNRESPONSIVE",
        }
    }
}

/// What the pages ask the application to do.
pub enum Action {
    Play(Mode),
    Quit,
    /// The system's folder picker, for the game (desktop).
    ChooseGame,
    /// The game's files, on a phone: Android's folder picker for a folder of
    /// the player's own, read where it is (nothing is copied); elsewhere, and
    /// if the picker will not open, look again at the known places (the files
    /// may have been copied in meanwhile).
    FindGame,
    /// Android's document picker: a GTA V profile folder.
    ImportSave,
    /// Android's document picker: a folder whose contents mirror the game
    /// folder (for example an omitted audio pack).
    ImportResources,
    /// Copy a report of Android import paths, outcomes, and save-file names.
    CopyImportDiagnostics,
    /// Capture the previous session's GLUE 5 process log through Shizuku.
    CaptureSessionLogcat,
    /// Start a user-requested GLUE 5 process log capture via Shizuku.
    StartLogcatCapture,
    /// Stop the live GLUE 5 process log capture and save/share its file.
    StopLogcatCapture,
    /// Add a timestamped observation to the GLUE 5 process log.
    MarkDebugEvent(DebugMarker),
    /// Close the pages and edit the debug touch areas over the game.
    EditDebugTouchAreas,
    /// The game's graphics settings back to the engine's defaults.
    ResetGraphics,
    /// Join a friend (by its key) playing GTA Online in `lobby`, from the
    /// start page.
    JoinFriend { key: String, lobby: String },
    /// Back to the game from the friends (a join asked for).
    CloseFriends,
    /// Delete the stream's copy of the game.
    ClearCopy,
    /// Download the optional package with this id (and what it needs).
    DownloadPackage(String),
    /// Pause the package transfer running.
    PausePackageDownload,
    /// Resume the paused package transfer.
    ResumePackageDownload,
    /// Cancel the package transfer running (what arrived stays for later).
    CancelPackageDownload,
    /// Look for servers on the network.
    FindServers,
    /// Stream from this server (an address found or typed).
    UseServer(String),
    /// The system's file picker, for a GPU driver package (Android).
    AddDriver,
    /// Delete the GPU driver package with this id.
    RemoveDriver(String),
}

/// What a text field of the app's takes: the system keyboard follows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextKind {
    #[default]
    Text,
    /// A friend code: letters and digits, in capitals.
    Code,
    /// A server's address or name.
    Address,
    /// The player's name.
    Name,
    /// A number (a mod's menu's typed entry).
    Number,
}

/// Where the focused text field keeps its [`TextKind`] (egui's memory).
const TEXT_KIND: &str = "text kind";

/// The system clipboard's text (a paste asked for by a tap).
fn clipboard_text() -> Option<String> {
    unsafe {
        let text = sdl3::sys::clipboard::SDL_GetClipboardText();
        if text.is_null() {
            return None;
        }
        let owned = std::ffi::CStr::from_ptr(text).to_string_lossy().into_owned();
        sdl3::sys::stdinc::SDL_free(text.cast());
        Some(owned).filter(|t| !t.is_empty())
    }
}

/// What the stream says over the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamAlert {
    /// The engine's reads have waited for the server for a moment.
    Waiting,
    /// The server does not answer (the game plays on from the copy).
    Offline,
}

/// A server found on the network, and what it answered when asked whether
/// it serves this game (nothing yet while it is asked).
pub type FoundServer = (glue5_stream::discovery::Found, Option<glue5_stream::Check>);

/// Navigation from a controller.
#[derive(Clone, Copy, Debug)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Activate,
    Back,
}

/// What the running game started with: settings that differ apply when it
/// starts again.
pub struct Started {
    pub game_dir: PathBuf,
    pub source: crate::settings::Source,
    pub max_fps: u32,
    pub low_memory: bool,
    pub mods: bool,
}

/// The phone's heat, and the back-off the game is held to because of it.
#[derive(Clone, Copy, Debug)]
pub struct Thermal {
    /// The back-off level: 0 (none) to 3.
    pub level: u8,
    /// The frames per second the game is held to (0: not held back).
    pub fps: u32,
    /// The system's own state: 0 none, 1 light, 2 moderate, 3 severe, 4
    /// critical, 5 emergency, 6 shutdown; -1: not read.
    pub status: i32,
}

impl Thermal {
    /// The system's state, in a word.
    pub fn state(&self) -> &'static str {
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

/// Optional content the server in use offers: a snapshot of its validated
/// manifest, what of it is installed, and the one transfer running. Taken
/// by the app when the stream is ready; the pages only read it.
pub struct Packages {
    /// The server's manifest; none: it offers no optional content (an older
    /// server, or none published — ordinary, not an error).
    pub manifest: Option<glue5_stream::PackageManifest>,
    /// The ids installed when last checked (a receipt and complete files).
    pub installed: Vec<String>,
    /// The transfer running, or the one that just failed.
    pub transfer: Option<PackageTransfer>,
}

/// One optional package on its way (or stopped on the way).
pub struct PackageTransfer {
    /// The package the user asked for (what it needs comes with it).
    pub package_id: String,
    /// The latest word from the worker: its phase and aggregate bytes. Its
    /// own package id may name a dependency being installed first.
    pub progress: glue5_stream::TransferProgress,
    /// It failed: why. A cancellation the user asked for is not kept.
    pub error: Option<String>,
    /// The worker still runs.
    pub active: bool,
}

/// What the pages show about the application.
pub struct Status<'a> {
    /// The game's folder, if found.
    pub game_dir: Option<&'a Path>,
    /// Why it was not (or why it would not start).
    pub problem: Option<&'a str>,
    /// The game runs (and started with these).
    pub started: Option<&'a Started>,
    /// Why the last game stopped, if it did.
    pub last_failure: Option<&'a str>,
    /// The game's graphics settings went back to their defaults.
    pub graphics_reset: bool,
    /// The window, in pixels.
    pub window: (u32, u32),
    /// The game's back buffer, while it runs.
    pub back_buffer: Option<(u32, u32)>,
    /// The upscalers this GPU has.
    pub upscalers: &'a [crate::settings::Upscaling],
    /// This GPU can generate frames (with MetalFX Temporal).
    pub frame_generation: bool,
    /// Apple's Metal HUD can show (it was on at launch).
    #[cfg_attr(not(target_vendor = "apple"), allow(dead_code))]
    pub metal_hud: bool,
    /// The display's refresh rate at most, in Hz.
    pub refresh_rate: f32,
    /// The frames per second the game is held to (0: the display's rate):
    /// the setting's, or the thermal back-off's if that is slower.
    pub frame_cap: u32,
    /// The phone's heat, and what the game is held to because of it (Android;
    /// none elsewhere).
    pub thermal: Option<Thermal>,
    /// Play waits for the game's pipelines: how far they are (0 to 1).
    pub preparing: Option<f32>,
    /// The mode Play starts (or the last one played).
    pub mode: Mode,
    /// The last benchmark's results.
    pub benchmark: Option<&'a crate::benchmark::Results>,
    /// The game running reaches GTA Online (GTA Online, the story, the
    /// sandbox).
    pub online: bool,
    /// Friend requests and game invites waiting for an answer.
    pub friend_requests: usize,
    /// Where the game's files come from (the setting).
    pub source: crate::settings::Source,
    /// The stream from a server, if one is set: its address and state.
    pub stream: Option<(&'a str, &'a glue5_stream::Status)>,
    /// Optional content the server in use offers; none until its client is
    /// ready and the snapshot was taken.
    pub packages: Option<&'a Packages>,
    /// This computer sharing its game folder (desktop).
    pub sharing: Option<crate::sharing::State>,
    /// The game runs with mods.
    pub mods: bool,
    /// A mod has a menu (a button for them beside the settings button).
    pub mod_menus: bool,
    /// GLUE 5 Studio, serving.
    pub studio: Option<&'a StudioInfo>,
    /// Servers found on the network, and whether a search runs.
    pub servers: &'a [FoundServer],
    pub searching: bool,
    /// A sign over the game: it waits for the server, or the server is away.
    pub stream_alert: Option<StreamAlert>,
    /// The GPU drivers the user can add, where this GPU takes them (Android,
    /// Adreno) or some were added.
    pub drivers: Option<&'a crate::drivers::Drivers>,
}

/// The settings button over the game, sized like the touch controls: at the
/// top's middle, beside their camera view and pause buttons.
pub const GEAR_RADIUS: f32 = 20.0;

/// Where the settings button is, in the safe area `safe`, at the touch
/// controls' scale.
pub fn gear_center(safe: Rect, scale: f32) -> Pos2 {
    pos2(safe.center().x + 54.0 * scale, safe.min.y + 30.0 * scale)
}

const WARN: Color32 = Color32::from_rgb(240, 180, 100);

/// The settings' categories, in the pause menu's rhythm: what the screen does,
/// what it sounds like, how it is played, with what, and where its files come
/// from — nine short lists instead of four conglomerated ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsTab {
    #[default]
    Graphics,
    Display,
    Audio,
    Controls,
    Gameplay,
    Files,
    Network,
    Advanced,
    Diagnostics,
}

pub struct Ui {
    ctx: egui::Context,
    stage: stage::Stage,
    tab: SettingsTab,
    selected_mode: Mode,
    gamepad_prompts: bool,
    gamepad_play: bool,
    overlay: glue5_gpu::Overlay,
    events: Vec<egui::Event>,
    pointer: Pos2,
    /// Only one finger drives egui's pointer; all fingers still send Touch.
    primary_touch: Option<(u64, u64)>,
    start: Instant,
    page: Option<Page>,
    /// Where the settings go back to: the start page before the game.
    home: Option<Page>,
    /// Where the credits, the friends and the servers go back to: the page
    /// they were opened from.
    behind: Option<Page>,
    friends: FriendsPage,
    servers: ServersPage,
    mods: ModsPage,
    /// The last submission drew something (an empty one clears it).
    drawn: bool,
    note: Option<String>,
    /// The render scale while its slider is dragged (it applies when let go).
    scale_edit: Option<f32>,
    /// A mode was chosen: this frame shows that it starts, then it does.
    starting: Option<Mode>,
    /// A warning over the page, waiting for its answer.
    popup: Option<popup::Open>,
    /// GTA Online's card on the start page is open (its lobby and name).
    online_open: bool,
    /// The settings button as last drawn (centre, radius), in points.
    gear: Option<(Pos2, f32)>,
    /// The mods' menus button, likewise.
    menus_button: Option<(Pos2, f32)>,
    /// When egui wants to draw again (before the game, frames are drawn
    /// only for input or for this).
    repaint_at: Option<Instant>,
    perf: perf::PerfHud,
    /// Notifications over everything (Social Club's overlay showed them),
    /// the oldest first, and where they were last drawn (to tap them).
    toasts: std::collections::VecDeque<Toast>,
    /// A help line being shown (see [`Hint`]).
    hint: Option<Hint>,
    toast_area: Option<Rect>,
    /// The touch controls as last drawn: their taps are the game's, even
    /// under a notification.
    controls: Vec<Rect>,
    /// The stream's sign over the game.
    chip: chip::Chip,
    /// Where the focused text field was last drawn (the system keyboard
    /// keeps it in view).
    text_rect: Option<Rect>,
    /// The on-screen keyboard's height over the window's bottom, in points:
    /// the pages end above it.
    keyboard: f32,
    /// The settings button's icon.
    gear_icon: Option<egui::TextureHandle>,
    /// GTA V's loading mark, used while Play prepares its pipelines.
    spinner: egui::TextureHandle,
    /// Which control had the focus at the last frame: when it changes, the
    /// selection moved, and the menu ticks (sfx).
    last_focus: Option<Id>,
}

/// A notification: as Social Club's, a title, a name and a line; the
/// friends' tab it opens; until when it shows.
struct Toast {
    toast: friends::Toast,
    /// A mod's: tapped, it opens the mods' menus (else the friends).
    from_mod: bool,
    since: Instant,
    until: Instant,
}

/// How long a notification shows (each one more after the ones before),
/// how many at most, how wide and high, how long it fades.
const TOAST_FOR: Duration = Duration::from_millis(3500);
const TOASTS: usize = 3;
const TOAST_WIDTH: f32 = 300.0;
/// A notification's height at most (title, name, line; avatar beside).
const TOAST_HEIGHT: f32 = 72.0;
const TOAST_FADE: f32 = 0.35;

/// A help line, the game's own kind of hint (its help text): one dark
/// strap of words that stays as long as it takes to read, then goes. Ours
/// says the things no page was asked about — that a copy landed on the
/// clipboard, that a wait is over.
struct Hint {
    text: String,
    since: Instant,
    until: Instant,
}

/// The help line's fade (the game's HUD components fade in 100 ms).
const HINT_FADE: f32 = 0.1;

/// How long a help line stays, in the game's help-text shape: a base plus
/// 333 ms a word, the subtotal raised by a third — never a blink, never a
/// paragraph's worth of waiting.
fn hint_for(text: &str) -> Duration {
    let words = text.split_whitespace().count() as u64;
    Duration::from_millis(((2000 + words * 333) * 13 / 10).clamp(3000, 12000))
}

impl Ui {
    pub fn new(overlay: glue5_gpu::Overlay, user_dir: &Path, selected_mode: Mode) -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let gear_icon = crate::touch::load_icon(&ctx, "settings");
        let spinner = spinner::load(&ctx);
        Ui {
            ctx,
            stage: stage::Stage::new(Some(user_dir.to_path_buf())),
            tab: SettingsTab::default(),
            selected_mode,
            gamepad_prompts: false,
            gamepad_play: false,
            overlay,
            events: Vec::new(),
            pointer: Pos2::ZERO,
            primary_touch: None,
            start: Instant::now(),
            page: Some(Page::Start),
            home: Some(Page::Start),
            behind: Some(Page::Start),
            friends: FriendsPage::default(),
            servers: ServersPage::default(),
            mods: ModsPage::default(),
            drawn: false,
            note: None,
            scale_edit: None,
            starting: None,
            popup: None,
            online_open: false,
            gear: None,
            repaint_at: Some(Instant::now()),
            perf: Default::default(),
            toasts: Default::default(),
            hint: None,
            toast_area: None,
            controls: Vec::new(),
            chip: Default::default(),
            menus_button: None,
            text_rect: None,
            keyboard: 0.0,
            gear_icon,
            spinner,
            last_focus: None,
        }
    }

    pub fn page(&self) -> Option<Page> {
        self.page
    }

    pub fn is_open(&self) -> bool {
        self.page.is_some()
    }

    /// The game runs: no start page any more.
    pub fn game_started(&mut self) {
        self.home = None;
        self.starting = None;
        self.stage.release();
        self.show(None);
    }

    /// A friends' notification, over the game or the pages.
    pub fn notify(&mut self, notice: &glue5_runtime::friends::Notice) {
        let now = Instant::now();
        while self.toasts.len() >= TOASTS {
            self.toasts.pop_front();
        }
        // Each leaves its time after the one before it: they go one at a time.
        let after = self.toasts.back().map_or(now, |t| t.until.max(now));
        let toast = friends::toast(notice);
        self.toasts.push_back(Toast { toast, from_mod: false, since: now, until: after + TOAST_FOR });
        sfx::want(Sound::Notify);
        self.repaint();
    }

    /// A mod's notification (`ui.notify`).
    pub fn notify_mod(&mut self, module: &str, text: &str) {
        let now = Instant::now();
        while self.toasts.len() >= TOASTS {
            self.toasts.pop_front();
        }
        let after = self.toasts.back().map_or(now, |t| t.until.max(now));
        let toast = friends::Toast { title: "Mod", name: module.to_owned(), text: text.to_owned(), tab: Default::default() };
        self.toasts.push_back(Toast { toast, from_mod: true, since: now, until: after + TOAST_FOR });
        sfx::want(Sound::Notify);
        self.repaint();
    }

    /// A help line over the page or the game (see [`Hint`]); a new one
    /// replaces the one showing, as the game's hints do.
    pub fn alert(&mut self, text: impl Into<String>) {
        let text = text.into();
        let now = Instant::now();
        // The same line again (a repeated press, a state flapping): let it
        // stay its full time from now, without blinking or ticking twice.
        if let Some(hint) = &mut self.hint {
            if hint.text == text {
                hint.until = now + hint_for(&text);
                self.repaint();
                return;
            }
        }
        self.hint = Some(Hint { until: now + hint_for(&text), since: now, text });
        sfx::want(Sound::Notify);
        self.repaint();
    }

    /// The mods' menus (F7); again, closed (where the player was in them
    /// stays for the next time).
    pub fn toggle_mod_menus(&mut self) {
        if self.page == Some(Page::Mods) {
            self.show(self.behind);
        } else {
            self.mods.show_menus();
            self.show(Some(Page::Mods));
        }
    }

    /// The friends (the Home key, Back + RB); again, back.
    pub fn toggle_friends(&mut self) {
        if self.page == Some(Page::Friends) {
            self.back();
        } else {
            self.show(Some(Page::Friends));
        }
    }

    /// The friends at the tab of the newest notification (one was tapped).
    pub fn open_toast(&mut self) {
        let from_mod = self.toasts.back().is_some_and(|t| t.from_mod);
        if let Some(t) = self.toasts.back() {
            self.friends.tab = t.toast.tab;
        }
        self.toasts.clear();
        if from_mod {
            self.mods.show_menus();
            self.show(Some(Page::Mods));
        } else {
            self.show(Some(Page::Friends));
        }
    }

    /// Whether a finger at `x`, `y` (0..1, as SDL gives them) is on a
    /// notification.
    pub fn toast_hit(&self, x: f32, y: f32, window: &Window) -> bool {
        let size = screen_size(window);
        let p = pos2(x * size.x, y * size.y);
        self.toast_area.is_some_and(|r| r.contains(p)) && !self.controls.iter().any(|r| r.contains(p))
    }

    /// The settings (from the game or the start page); again, back.
    pub fn toggle_settings(&mut self) {
        if self.page == Some(Page::Settings) {
            self.back();
        } else {
            self.show(Some(Page::Settings));
        }
    }

    /// From the settings back to the start page, or the game; from the
    /// credits back to where they were opened; on the start page, GTA
    /// Online's card closes.
    pub fn back(&mut self) {
        // A warning on show takes the way back first: dropping it is its
        // cancel, and the page under it stays where it was.
        if self.popup.take().is_some() {
            sfx::want(Sound::Back);
            self.repaint();
            return;
        }
        let was = self.page;
        match self.page {
            Some(Page::Start) if self.online_open => {
                self.online_open = false;
                self.repaint();
            }
            Some(Page::Settings) => self.show(self.home),
            // Out of a field, or up a menu, first.
            Some(Page::Mods) if self.mods.back(&self.ctx) => self.repaint(),
            Some(Page::Credits | Page::Friends | Page::Servers | Page::Mods) => self.show(self.behind),
            _ => {}
        }
        if self.page != was {
            sfx::want(Sound::Back);
        }
    }

    /// A mod asked for its menu (`menu:show()`): the Mods page opens at it,
    /// unless a page is open.
    pub fn show_mod_menu(&mut self, module: &str, menu: u32) {
        if self.page.is_none() && self.mods.show_menu(module, menu) {
            self.show(Some(Page::Mods));
        }
    }

    fn show(&mut self, page: Option<Page>) {
        if self.page == Some(Page::Mods) && page != Some(Page::Mods) {
            self.mods.closed();
        }
        let secondary = |p: Option<Page>| matches!(p, Some(Page::Credits | Page::Friends | Page::Servers | Page::Mods));
        if secondary(page) && self.page != page {
            // One opened from another goes back where the first came from
            // (else back would go between the two for ever).
            if !secondary(self.page) {
                self.behind = self.page;
            }
            if page == Some(Page::Servers) {
                self.servers.opened();
            }
        }
        if page != self.page {
            if self.primary_touch.take().is_some() {
                self.events.push(egui::Event::PointerGone);
            }
            // Over the game: the menu opens and closes. Inside it: another page.
            match (self.page, page) {
                (None, Some(_)) => sfx::want(Sound::Open),
                (Some(_), None) => sfx::want(Sound::Close),
                (Some(_), Some(_)) => sfx::want(Sound::Tab),
                (None, None) => {}
            }
        }
        self.page = page;
        self.note = None;
        self.scale_edit = None;
        self.ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        self.repaint();
    }

    /// A message shown in the settings (e.g. "applies at the next start").
    pub fn note(&mut self, text: impl Into<String>) {
        self.note = Some(text.into());
        self.repaint();
    }

    /// Draw again at the next frame (the window changed, ...).
    pub fn repaint(&mut self) {
        self.repaint_in(Duration::ZERO);
    }

    pub fn repaint_in(&mut self, delay: Duration) {
        let at = Instant::now() + delay;
        self.repaint_at = Some(self.repaint_at.map_or(at, |t| t.min(at)));
    }

    /// Before the game: whether a frame should be drawn now, and how long
    /// the application can wait for input otherwise.
    pub fn wants_frame(&self) -> bool {
        !self.events.is_empty() || self.repaint_at.is_some_and(|t| Instant::now() >= t)
    }

    /// A text field of the app's has the focus: text input on (the system
    /// keyboard on touch screens).
    pub fn wants_text(&self) -> bool {
        self.page.is_some() && self.ctx.egui_wants_keyboard_input()
    }

    /// The on-screen keyboard's height over the window's bottom, in points
    /// ([`crate::keyboard::overlap`]): an open page is drawn again above it
    /// as it moves.
    pub fn keyboard(&mut self, overlap: f32) {
        if overlap != self.keyboard {
            self.keyboard = overlap;
            if self.page.is_some() {
                self.repaint();
            }
        }
    }

    /// What the focused text field takes, and where it is (in points).
    pub fn text_field(&self) -> Option<(TextKind, Rect)> {
        let kind = self.ctx.data(|d| d.get_temp::<TextKind>(Id::new(TEXT_KIND))).unwrap_or_default();
        self.text_rect.filter(|_| self.wants_text()).map(|rect| (kind, rect))
    }

    pub fn idle_for(&self) -> Duration {
        let most = Duration::from_millis(250);
        self.repaint_at.map_or(most, |t| t.saturating_duration_since(Instant::now()).min(most))
    }

    /// Whether a finger at `x`, `y` (0..1, as SDL gives them) is on the
    /// settings button.
    pub fn gear_hit(&self, x: f32, y: f32, window: &Window) -> bool {
        let size = screen_size(window);
        self.gear.is_some_and(|(c, r)| c.distance(pos2(x * size.x, y * size.y)) <= r * 1.4)
    }

    /// Whether a finger at `x`, `y` (0..1) is on the mods' menus button.
    pub fn menus_hit(&self, x: f32, y: f32, window: &Window) -> bool {
        let size = screen_size(window);
        self.menus_button.is_some_and(|(c, r)| c.distance(pos2(x * size.x, y * size.y)) <= r * 1.4)
    }

    /// Whether a finger at `x`, `y` (0..1) is on the stream's sign.
    pub fn chip_hit(&self, x: f32, y: f32, window: &Window) -> bool {
        let size = screen_size(window);
        self.chip.hit(pos2(x * size.x, y * size.y))
    }

    /// The Servers page (from the stream's sign over the game).
    pub fn show_servers(&mut self) {
        self.show(Some(Page::Servers));
    }

    /// Input for a page while one is open.
    pub fn event(&mut self, event: &Event, window: &Window) {
        if matches!(event, Event::KeyDown { .. } | Event::MouseButtonDown { .. } | Event::FingerDown { .. }) {
            self.gamepad_prompts = false;
        }
        if self.page == Some(Page::Start)
            && !self.ctx.egui_wants_keyboard_input()
            && let Event::KeyDown { scancode: Some(sc), repeat: false, .. } = event
            && matches!(sc, Scancode::Left | Scancode::Right)
        {
            self.cycle_mode(if *sc == Scancode::Right { 1 } else { -1 });
            return;
        }
        let size = screen_size(window);
        let scale = points_per_window_unit(window);
        let button = |b: &MouseButton| match b {
            MouseButton::Right => Some(egui::PointerButton::Secondary),
            MouseButton::Middle => Some(egui::PointerButton::Middle),
            MouseButton::Left => Some(egui::PointerButton::Primary),
            _ => None,
        };
        let modifiers = egui::Modifiers::default();
        match event {
            Event::MouseMotion { x, y, .. } => self.pointer_to(pos2(*x, *y) * scale),
            Event::MouseButtonDown { mouse_btn, x, y, .. } | Event::MouseButtonUp { mouse_btn, x, y, .. } => {
                let pressed = matches!(event, Event::MouseButtonDown { .. });
                if let Some(button) = button(mouse_btn) {
                    self.pointer_to(pos2(*x, *y) * scale);
                    self.events.push(egui::Event::PointerButton { pos: self.pointer, button, pressed, modifiers });
                }
            }
            Event::MouseWheel { x, y, .. } => self.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: vec2(*x, *y),
                modifiers,
                phase: egui::TouchPhase::Move,
            }),
            // A finger is a touch (egui scrolls by dragging only on a touch
            // screen) and the pointer.
            // A finger the system took (an edge gesture, a call) lets go too.
            Event::FingerDown { touch_id, finger_id, x, y, pressure, .. }
            | Event::FingerUp { touch_id, finger_id, x, y, pressure, .. }
            | Event::FingerCanceled { touch_id, finger_id, x, y, pressure, .. } => {
                let pressed = matches!(event, Event::FingerDown { .. });
                let phase = match event {
                    Event::FingerDown { .. } => egui::TouchPhase::Start,
                    Event::FingerUp { .. } => egui::TouchPhase::End,
                    _ => egui::TouchPhase::Cancel,
                };
                let at = pos2(x * size.x, y * size.y);
                self.touch(*touch_id, *finger_id, phase, at, *pressure);
                let finger = (*touch_id, *finger_id);
                if pressed && self.primary_touch.is_none() {
                    self.primary_touch = Some(finger);
                }
                if self.primary_touch == Some(finger) {
                    // A cancel releases away from the press, never as a click.
                    self.pointer_to(if phase == egui::TouchPhase::Cancel { pos2(-1.0e4, -1.0e4) } else { at });
                    self.events.push(egui::Event::PointerButton {
                        pos: self.pointer,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers,
                    });
                    if !pressed {
                        self.primary_touch = None;
                        self.events.push(egui::Event::PointerGone);
                    }
                }
            }
            Event::FingerMotion { touch_id, finger_id, x, y, pressure, .. } => {
                let p = pos2(x * size.x, y * size.y);
                self.touch(*touch_id, *finger_id, egui::TouchPhase::Move, p, *pressure);
                if self.primary_touch == Some((*touch_id, *finger_id)) {
                    self.pointer_to(p);
                }
            }
            Event::TextInput { text, .. } => self.events.push(egui::Event::Text(text.clone())),
            // Pasting and copying into the app's text fields.
            Event::KeyDown { scancode: Some(sc @ (Scancode::V | Scancode::C | Scancode::X)), keymod, .. }
                if keymod.intersects(sdl3::keyboard::Mod::LCTRLMOD | sdl3::keyboard::Mod::RCTRLMOD)
                    || keymod.intersects(sdl3::keyboard::Mod::LGUIMOD | sdl3::keyboard::Mod::RGUIMOD) =>
            {
                self.events.push(match sc {
                    Scancode::V => match window.subsystem().clipboard().clipboard_text() {
                        Ok(text) => egui::Event::Paste(text),
                        Err(_) => return,
                    },
                    Scancode::C => egui::Event::Copy,
                    _ => egui::Event::Cut,
                });
            }
            Event::KeyDown { scancode: Some(sc), keymod, .. } | Event::KeyUp { scancode: Some(sc), keymod, .. } => {
                let pressed = matches!(event, Event::KeyDown { .. });
                if let Some(key) = key(*sc) {
                    let shift = keymod.intersects(sdl3::keyboard::Mod::LSHIFTMOD | sdl3::keyboard::Mod::RSHIFTMOD);
                    self.key(key, pressed, egui::Modifiers { shift, ..Default::default() });
                }
            }
            _ => {}
        }
    }

    fn touch(&mut self, device: u64, finger: u64, phase: egui::TouchPhase, pos: Pos2, force: f32) {
        self.events.push(egui::Event::Touch {
            device_id: egui::TouchDeviceId(device),
            id: egui::TouchId(finger),
            phase,
            pos,
            force: Some(force),
        });
    }

    fn pointer_to(&mut self, p: Pos2) {
        self.pointer = p;
        self.events.push(egui::Event::PointerMoved(p));
    }

    fn key(&mut self, key: egui::Key, pressed: bool, modifiers: egui::Modifiers) {
        self.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
    }

    fn cycle_mode(&mut self, step: isize) {
        let modes = [Mode::Story, Mode::Online, Mode::Sandbox, Mode::Benchmark];
        let at = modes.iter().position(|mode| *mode == self.selected_mode).unwrap_or(0) as isize;
        self.selected_mode = modes[(at + step).rem_euclid(modes.len() as isize) as usize];
        self.repaint();
    }

    pub fn nav(&mut self, nav: Nav) {
        self.gamepad_prompts = true;
        // A warning on show takes the pad for itself: its keys are taken
        // out of the frame's input before the page under it can see them,
        // so they must arrive as themselves, focus or no focus.
        if self.popup.is_some() {
            let key = match nav {
                Nav::Up => egui::Key::ArrowUp,
                Nav::Down => egui::Key::ArrowDown,
                Nav::Left => egui::Key::ArrowLeft,
                Nav::Right => egui::Key::ArrowRight,
                Nav::Activate => egui::Key::Enter,
                Nav::Back => return self.back(),
            };
            self.key(key, true, Default::default());
            self.key(key, false, Default::default());
            return;
        }
        if self.page == Some(Page::Start) && !self.ctx.egui_wants_keyboard_input() {
            match nav {
                Nav::Left | Nav::Right => {
                    self.cycle_mode(if matches!(nav, Nav::Right) { 1 } else { -1 });
                    return;
                }
                Nav::Activate => {
                    if self.ctx.memory(|memory| memory.focused()).is_none() {
                        self.gamepad_play = true;
                        self.repaint();
                    } else {
                        self.key(egui::Key::Enter, true, Default::default());
                        self.key(egui::Key::Enter, false, Default::default());
                    }
                    return;
                }
                _ => {}
            }
        }
        let key = match nav {
            Nav::Up => egui::Key::ArrowUp,
            Nav::Down => egui::Key::ArrowDown,
            Nav::Left => egui::Key::ArrowLeft,
            Nav::Right => egui::Key::ArrowRight,
            Nav::Activate => egui::Key::Enter,
            Nav::Back => return self.back(),
        };
        // Nothing has the focus yet: the first move gives it to the page's
        // first control (egui moves it from a focused widget only).
        let key = if self.ctx.memory(|m| m.focused()).is_none() { egui::Key::Tab } else { key };
        self.key(key, true, Default::default());
        self.key(key, false, Default::default());
    }

    /// Draws the open page, or over the game the touch controls (if given),
    /// the settings button (if given, at this opacity) and the performance
    /// overlay (if on).
    pub fn frame(
        &mut self,
        window: &Window,
        settings: &mut Settings,
        status: &Status,
        touch: Option<&Touch>,
        gear: Option<f32>,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        // The GPU blur uses the shell's full-output panel bounds, clamped at
        // the safe area's right edge. It is updated from exact geometry below.
        let screen = Rect::from_min_size(Pos2::ZERO, screen_size(window));
        let safe = safe_rect(window);
        let blur_active = self.page.is_some()
            && status.started.is_some()
            && screen.width() > 0.0
            && screen.height() > 0.0;
        // A closed panel must stop blurring immediately. While an earlier
        // overlay is in flight, keep the last exact egui rectangle rather
        // than replacing it with this window-level approximation.
        if !blur_active {
            self.overlay.set_background_blur(None);
        }
        let gear = gear.filter(|_| self.page.is_none());
        let hud = status.started.is_some() && self.page.is_none() && settings.overlay == PerfOverlay::Glue5;
        if !hud {
            self.perf.hide();
        }
        let alert = status.stream_alert.filter(|_| self.page.is_none());
        let now = Instant::now();
        self.toasts.retain(|t| now < t.until);
        if self.hint.as_ref().is_some_and(|h| now >= h.until) {
            self.hint = None;
        }
        if self.toasts.is_empty() {
            self.toast_area = None;
        }
        if self.page.is_none()
            && touch.is_none()
            && gear.is_none()
            && alert.is_none()
            && self.toasts.is_empty()
            && self.hint.is_none()
            && (!hud || !self.perf.due(settings.overlay_detail))
            && status.started.is_none()
        {
            if self.drawn && !hud {
                self.overlay.submit(Vec::new(), Default::default(), 1.0);
                self.drawn = false;
            }
            self.events.clear();
            self.gear = None;
            self.menus_button = None;
            self.chip = Default::default();
            return actions;
        }
        // Over the game, one pass per frame shown: the renderer draws only
        // the newest submission at each present, the loop turns faster (its
        // input stays queued for the next pass).
        if status.started.is_some() && self.overlay.waiting() {
            return actions;
        }
        // Play was chosen at the last frame: this one shows that the game
        // starts, then it does.
        let starting = std::mem::take(&mut self.starting);
        // A warning on show keeps the keys it answers to itself.
        let popup_was_open = self.popup.is_some();
        let popup_keys = if self.popup.is_some() {
            popup::take_keys(&mut self.events)
        } else {
            popup::Keys::default()
        };
        let ppp = pixels_per_point(window);
        let screen = Rect::from_min_size(Pos2::ZERO, screen_size(window));
        let safe = safe_rect(window);
        // A page ends above the on-screen keyboard (egui's content rect, where
        // the sheets go); what is drawn over the game keeps to the safe area.
        let keyboard = if self.page.is_some() { self.keyboard } else { 0.0 };
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            safe_area_insets: Some(SafeAreaInsets(egui::epaint::MarginF32 {
                left: safe.min.x - screen.min.x,
                right: screen.max.x - safe.max.x,
                top: safe.min.y - screen.min.y,
                bottom: (screen.max.y - safe.max.y).max(keyboard),
            })),
            time: Some(self.start.elapsed().as_secs_f64()),
            events: std::mem::take(&mut self.events),
            focused: true,
            ..Default::default()
        };
        raw.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);

        let ctx = self.ctx.clone();
        // The page to show next, or back to the previous one.
        let mut go = None;
        let mut back = false;
        ctx.begin_pass(raw);
        shell::set_output_rect(&ctx, screen);
        // Use the exact same full-output/content geometry as the shell.
        // If this pass is skipped while another overlay is in flight, the
        // last exact rectangle remains active instead of widening the blur.
        let viewport = shell::output_rect(&ctx);
        let content = ctx.content_rect();
        let panel = shell::panel_rect(viewport, content);
        let blur = blur_active.then(|| panel_fractions(panel, screen));
        self.overlay.set_background_blur(blur);
        shell::begin(self.page);
        if status.started.is_some() {
            self.stage.discard_pending();
        }
        if self.page.is_some() && status.started.is_none() {
            self.stage.draw(&ctx, status.game_dir, self.page == Some(Page::Start), !settings.reduce_motion);
        }
        if let Some(touch) = touch {
            touch.draw(&ctx.layer_painter(LayerId::background()), settings.touch_opacity);
        }
        let scale = crate::touch::control_scale(screen.size(), settings.touch_size);
        self.gear = gear.map(|alpha| draw_gear(&ctx, safe, scale, alpha, self.gear_icon.as_ref()));
        self.menus_button = gear.filter(|_| status.mod_menus).map(|alpha| draw_menus_button(&ctx, safe, scale, alpha));
        let overlay = hud.then(|| self.perf.draw(&ctx, safe, settings, status));
        // The notifications and the stream's sign keep off everything else
        // drawn over the game.
        self.controls = touch.map_or_else(Vec::new, Touch::occupied);
        let mut taken = self.controls.clone();
        taken.extend(self.gear.into_iter().chain(self.menus_button).map(|(c, r)| Rect::from_center_size(c, Vec2::splat(2.0 * r))));
        taken.extend(overlay);
        // Under the settings button when it shows.
        // Under the top's buttons (the touch controls' camera view and
        // pause, the settings), where they show.
        let below = if self.gear.is_some() { (30.0 + GEAR_RADIUS) * scale + 8.0 } else { 12.0 };
        self.toast_area = draw_toasts(&ctx, safe, below, &taken, &self.toasts, now);
        if !self.toasts.is_empty() || self.hint.is_some() {
            // Through egui: its delay decides the next frame after this one.
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        // The sign, legible whatever the touch controls' opacity.
        taken.extend(self.toast_area);
        if let Some(hint) = &self.hint {
            taken.extend(draw_hint(&ctx, safe, below, &taken, hint, now));
        }
        let alpha = gear.unwrap_or(1.0).max(0.75);
        self.chip.draw(&ctx, safe, alert, self.gear, &taken, scale, alpha);
        match self.page {
            Some(Page::Start) => {
                menu::show_with_spinner(
                    &ctx,
                    settings,
                    status,
                    &mut self.selected_mode,
                    Some(&self.spinner),
                    self.gamepad_prompts,
                    starting,
                    &mut actions,
                    &mut go,
                );
                if std::mem::take(&mut self.gamepad_play) {
                    if status.game_dir.is_some() && status.preparing.is_none() && starting.is_none() {
                        sfx::want(Sound::Launch);
                        actions.push(Action::Play(self.selected_mode));
                    } else {
                        // Play, with nothing to play.
                        sfx::want(Sound::Error);
                    }
                }
            }
            Some(Page::Settings) => {
                let note = self.note.as_deref();
                let (tab, scale_edit) = (&mut self.tab, &mut self.scale_edit);
                back = settings_page(&ctx, settings, status, note, tab, scale_edit, &mut actions, &mut go);
            }
            Some(Page::Credits) => back = credits_page(&ctx),
            Some(Page::Friends) => {
                let game = friends::Context { started: status.started.is_some(), online: status.online };
                back = friends_page(&ctx, &mut self.friends, game, &mut actions);
            }
            Some(Page::Servers) => back = servers_page(&ctx, &mut self.servers, settings, status, &mut actions),
            // Its X closes it, from inside a menu too (back goes up a menu).
            Some(Page::Mods) => go = mods_page(&ctx, &mut self.mods, settings, status).then_some(self.behind),
            None => {}
        }
        // The shell already displays the build label in its header.
        if self.page.is_none() {
            draw_build_identity(&ctx, safe);
        }
        // A page that had to ask first: its warning opens over what it
        // drew, and its answer is the action the page stood for.
        if let Some(request) = popup::take() {
            self.popup = Some(popup::Open::new(request));
            sfx::want(Sound::Open);
        }
        let answered = self.popup.as_mut().map(|open| popup::show(&ctx, open, screen, content, popup_keys));
        let popup_shown = popup_was_open || answered.is_some();
        match answered {
            None | Some(popup::Outcome::Open) => {}
            Some(popup::Outcome::Confirmed(confirm)) => {
                self.popup = None;
                sfx::want(Sound::Confirm);
                actions.push(match confirm {
                    popup::Confirm::ClearCopy => Action::ClearCopy,
                    popup::Confirm::ResetGraphics => Action::ResetGraphics,
                });
            }
            Some(popup::Outcome::Cancelled) => {
                self.popup = None;
                sfx::want(Sound::Back);
            }
        }
        // The selection moved to another control (a d-pad, an arrow, a stick,
        // or the pointer over another line): the menu ticks, as a console's
        // does. Typing in a field moves the focus too; that is not a tick.
        let focus = ctx.memory(|m| m.focused());
        if focus != self.last_focus && focus.is_some() && !ctx.egui_wants_keyboard_input() {
            sfx::want(Sound::Focus);
        }
        self.last_focus = focus;
        let mut output = ctx.end_pass();
        self.text_rect = output.platform_output.ime.as_ref().map(|ime| ime.rect);
        for command in std::mem::take(&mut output.platform_output.commands) {
            match command {
                egui::OutputCommand::OpenUrl(open) => {
                    if let Err(e) = sdl3::url::open_url(&open.url) {
                        log::warn!("cannot open {}: {e}", open.url);
                    }
                }
                egui::OutputCommand::CopyText(text) => {
                    let _ = window.subsystem().clipboard().set_clipboard_text(&text);
                }
                _ => {}
            }
        }
        let delay = output.viewport_output.get(&ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay);
        self.repaint_at = (delay < Duration::from_secs(10)).then(|| Instant::now() + delay);
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        self.overlay.submit(primitives, output.textures_delta, output.pixels_per_point);
        self.drawn = true;
        // A tab of the shell's strip asks for a page the same way a page's own
        // link does.
        let tab = shell::take_nav();
        // While a warning owned the frame, the page under it does not leave
        // or follow a tab from a press meant for the warning.
        if !popup_shown {
            if back {
                self.back();
            } else if let Some(page) = go.or(tab) {
                self.show(page);
            }
        }
        if self.starting.is_some() {
            self.repaint();
        } else if status.preparing.is_some() {
            self.repaint_in(Duration::from_millis(200));
        }
        if let Some(mode) = starting {
            actions.push(Action::Play(mode));
        }
        actions
    }
}

/// A small, always-on build label over both the start page and the running game.
fn draw_build_identity(ctx: &egui::Context, safe: Rect) {
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("alpha-build-identity")));
    let text_color = Color32::from_white_alpha(225);
    let padding = vec2(5.0, 3.0);
    let galley = painter.layout_no_wrap(crate::version::LABEL.to_owned(), FontId::proportional(9.0), text_color);
    let size = galley.size() + vec2(10.0, 6.0);
    let rect = Rect::from_min_size(
        pos2(
            (safe.right() - size.x - 8.0).max(safe.left() + 4.0),
            (safe.top() + 28.0).min(safe.bottom() - size.y - 4.0),
        ),
        size,
    );
    painter.rect_filled(rect, 0, Color32::from_black_alpha(170));
    painter.rect_stroke(rect, 0, Stroke::new(0.5, Color32::from_white_alpha(45)), egui::StrokeKind::Inside);
    painter.galley(rect.min + padding, galley, text_color);
}

/// The notifications, at the safe area's top right (`below` its top), the
/// newest at the bottom, fading in and out: where they are, if any.
fn draw_toasts(
    ctx: &egui::Context,
    safe: Rect,
    below: f32,
    taken: &[Rect],
    toasts: &std::collections::VecDeque<Toast>,
    now: Instant,
) -> Option<Rect> {
    if toasts.is_empty() {
        return None;
    }
    let scale = theme::layout_scale(safe);
    let width = (TOAST_WIDTH * scale).min(safe.width() - 32.0);
    // All of them where they fit, else the newest alone; nowhere free (the
    // touch controls fill the top): none over the game, the page has them.
    let (at, shown) = [toasts.len(), 1].into_iter().find_map(|n| {
        let height = n as f32 * TOAST_HEIGHT * scale + (n - 1) as f32 * 8.0 * scale;
        toast_place(safe, below, vec2(width, height), taken).map(|at| (at, n))
    })?;
    let response = egui::Area::new(Id::new("toasts"))
        .order(Order::Tooltip)
        .fixed_pos(at)
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 8.0 * scale;
            for t in toasts.iter().skip(toasts.len() - shown) {
                let shown = now.duration_since(t.since).as_secs_f32();
                let left = t.until.saturating_duration_since(now).as_secs_f32();
                ui.scope(|ui| {
                    ui.multiply_opacity((shown.min(left) / TOAST_FADE).clamp(0.0, 1.0));
                    friends::draw_toast(ui, &t.toast, width);
                });
            }
        });
    Some(response.response.rect)
}

/// The help line (see [`Hint`]): the game's own shape — one black strap as
/// high as its words (margins 7/5/4/12), hugging the top right corner
/// (the left is the shell's panel) where it covers nothing else, fading
/// in and out. Where it is, if it could be placed.
fn draw_hint(ctx: &egui::Context, safe: Rect, below: f32, taken: &[Rect], hint: &Hint, now: Instant) -> Option<Rect> {
    let scale = theme::layout_scale(safe);
    let margins = vec2((7.0 + 5.0) * scale, (4.0 + 12.0) * scale);
    let wrap = ((360.0 * scale).min((safe.width() - 32.0).max(1.0)) - margins.x).max(1.0);
    let measure = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("hint-measure")));
    let galley = measure.layout(
        hint.text.clone(),
        FontId::new(14.5 * scale, theme::fonts::regular()),
        theme::palette::TEXT_HI,
        wrap,
    );
    let size = galley.size() + margins;
    // The game's help line sits in a top corner, not floating mid-top: try
    // the top right first (over the scene, clear of the panel), and fall
    // back to the notifications' placement when the corner is busy.
    let corner = pos2(safe.right() - size.x - 16.0, safe.top() + 12.0);
    let corner_rect = Rect::from_min_size(corner, size);
    let corner_free = safe.contains_rect(corner_rect) && !taken.iter().any(|t| t.intersects(corner_rect.expand(4.0)));
    let at = if corner_free { corner } else { toast_place(safe, below, size, taken)? };
    let shown = now.duration_since(hint.since).as_secs_f32();
    let left = hint.until.saturating_duration_since(now).as_secs_f32();
    let response = egui::Area::new(Id::new("hint"))
        .order(Order::Tooltip)
        .fixed_pos(at)
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_width(size.x);
            ui.set_height(size.y);
            ui.multiply_opacity((shown.min(left) / HINT_FADE).clamp(0.0, 1.0));
            let painter = ui.painter();
            painter.rect_filled(Rect::from_min_size(at, size), 0, Color32::from_rgba_unmultiplied(0, 0, 0, 186));
            painter.galley(at + vec2(7.0 * scale, 4.0 * scale), galley, theme::palette::TEXT_HI);
        });
    Some(response.response.rect)
}

/// Where the notifications go (their top left), the first place where they
/// cover nothing else on screen (`taken`): in the top's middle under its
/// buttons (the touch controls' camera view and pause, the settings; the
/// game shows nothing there), in the top right or left corner, else lower
/// in those three places, down to the middle of the screen at most.
fn toast_place(safe: Rect, below: f32, size: Vec2, taken: &[Rect]) -> Option<Pos2> {
    // The three above, then the columns between the middle and either side:
    // the touch controls leave one of those free more often than they do one
    // of the three, on a phone with the controls at their largest.
    let inset = (safe.width() - size.x) / 4.0;
    let starts = [
        pos2(safe.center().x - size.x / 2.0, safe.top() + below),
        pos2(safe.right() - size.x - 16.0, safe.top() + 12.0),
        pos2(safe.left() + 16.0, safe.top() + 12.0),
        pos2(safe.left() + inset, safe.top() + below),
        pos2(safe.right() - size.x - inset, safe.top() + below),
        pos2(safe.left() + inset, safe.top() + 12.0),
        pos2(safe.right() - size.x - inset, safe.top() + 12.0),
    ];
    let lowest = safe.center().y - size.y / 2.0;
    let free = |at: Pos2| {
        let rect = Rect::from_min_size(at, size);
        safe.contains_rect(rect) && !taken.iter().any(|t| t.intersects(rect.expand(4.0)))
    };
    (0..)
        .map(|step| step as f32 * 8.0)
        .take_while(|down| starts.iter().any(|s| s.y + down <= lowest))
        .flat_map(|down| starts.map(|s| s + vec2(0.0, down)))
        .find(|&at| at.y <= lowest && free(at))
}

// Points: the UI's unit, the same physical size everywhere. As SDL's
// README-highdpi describes, a point is the window's display scale in pixels;
// SDL's window coordinates are points on Apple platforms but pixels on
// Android and Windows (where the display's content scale enlarges the UI).

/// Pixels per point.
fn pixels_per_point(window: &Window) -> f32 {
    window.display_scale().max(0.5)
}

/// Points per unit of SDL's window coordinates (event and safe area
/// positions).
pub(crate) fn points_per_window_unit(window: &Window) -> f32 {
    window.pixel_density() / pixels_per_point(window)
}

/// The window's size in points.
pub fn screen_size(window: &Window) -> Vec2 {
    let (w, h) = window.size_in_pixels();
    vec2(w as f32, h as f32) / pixels_per_point(window)
}

/// A rectangle in points, in SDL's window coordinates.
pub fn window_rect(window: &Window, r: Rect) -> sdl3::rect::Rect {
    let r = Rect::from_min_max(r.min / points_per_window_unit(window), r.max / points_per_window_unit(window));
    sdl3::rect::Rect::new(r.min.x as i32, r.min.y as i32, r.width().max(1.0) as u32, r.height().max(1.0) as u32)
}

/// The part of the window clear of notches and rounded corners, in points.
pub fn safe_rect(window: &Window) -> Rect {
    match window.safe_area() {
        Ok(r) if r.width() > 0 && r.height() > 0 => {
            let scale = points_per_window_unit(window);
            Rect::from_min_size(
                pos2(r.x() as f32, r.y() as f32) * scale,
                vec2(r.width() as f32, r.height() as f32) * scale,
            )
        }
        _ => Rect::from_min_size(Pos2::ZERO, screen_size(window)),
    }
}

/// The settings button: the touch controls' look, at their size.
fn draw_gear(ctx: &egui::Context, safe: Rect, scale: f32, alpha: f32, icon: Option<&egui::TextureHandle>) -> (Pos2, f32) {
    let c = gear_center(safe, scale);
    let r = GEAR_RADIUS * scale;
    let look = crate::touch::Look::new(alpha, scale, glue5_runtime::input::Character::Other);
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("gear")));
    painter.circle(c, r, look.fill(false), look.stroke(false));
    match icon {
        Some(icon) => {
            let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
            painter.image(icon.id(), Rect::from_center_size(c, Vec2::splat(r * 1.15)), uv, look.icon);
        }
        None => {
            painter.text(c, Align2::CENTER_CENTER, "⚙", FontId::proportional(r * 0.95), look.icon);
        }
    }
    (c, r)
}

/// The mods' menus button, the settings button's right neighbour: sliders.
fn draw_menus_button(ctx: &egui::Context, safe: Rect, scale: f32, alpha: f32) -> (Pos2, f32) {
    let c = gear_center(safe, scale) + vec2(54.0 * scale, 0.0);
    let r = GEAR_RADIUS * scale;
    let look = crate::touch::Look::new(alpha, scale, glue5_runtime::input::Character::Other);
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("gear")));
    painter.circle(c, r, look.fill(false), look.stroke(false));
    let stroke = egui::Stroke::new(r * 0.1, look.icon);
    for (dy, knob) in [(-0.24, 0.18), (0.24, -0.2)] {
        let y = c.y + dy * r;
        painter.line_segment([pos2(c.x - 0.48 * r, y), pos2(c.x + 0.48 * r, y)], stroke);
        painter.circle_filled(pos2(c.x + knob * r, y), r * 0.15, look.icon);
    }
    (c, r)
}

fn key(sc: Scancode) -> Option<egui::Key> {
    Some(match sc {
        Scancode::Tab => egui::Key::Tab,
        Scancode::Return | Scancode::KpEnter => egui::Key::Enter,
        Scancode::Space => egui::Key::Space,
        Scancode::Backspace => egui::Key::Backspace,
        Scancode::Delete => egui::Key::Delete,
        Scancode::Home => egui::Key::Home,
        Scancode::End => egui::Key::End,
        Scancode::Up => egui::Key::ArrowUp,
        Scancode::Down => egui::Key::ArrowDown,
        Scancode::Left => egui::Key::ArrowLeft,
        Scancode::Right => egui::Key::ArrowRight,
        _ => return None,
    })
}

/// Express a shell panel in full-output coordinates for the GPU scissor.
fn panel_fractions(panel: Rect, screen: Rect) -> [f32; 4] {
    let width = screen.width().max(f32::EPSILON);
    let height = screen.height().max(f32::EPSILON);
    [
        (panel.left() - screen.left()) / width,
        (panel.top() - screen.top()) / height,
        (panel.right() - screen.left()) / width,
        (panel.bottom() - screen.top()) / height,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::touch::Touch;
    use glue5_runtime::input::{Activity, Character, Driving, PlayerContext};

    #[test]
    fn a_hint_stays_longer_the_more_there_is_to_read() {
        let short = hint_for("Copied.");
        let long = hint_for("The whole report is on the clipboard, ready to paste anywhere.");
        assert!(long > short, "{short:?} !< {long:?}");
        assert!(short >= Duration::from_millis(3000));
        assert!(hint_for(&"word ".repeat(80)) <= Duration::from_millis(12000));
    }

    #[test]
    fn blur_matches_the_full_bleed_panel_in_the_output() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1568.0, 704.0));
        let safe = Rect::from_min_max(pos2(100.0, 52.0), pos2(1468.0, 650.0));
        let panel = shell::panel_rect(screen, safe);
        let fractions = panel_fractions(panel, screen);
        assert!(fractions[0].abs() < f32::EPSILON);
        assert!(fractions[1].abs() < f32::EPSILON);
        assert!((fractions[2] - 0.62).abs() < f32::EPSILON);
        assert!((fractions[3] - 1.0).abs() < f32::EPSILON);
    }

    /// Every touch layout, at every controls' size, on phones and a tablet:
    /// the notifications, where they show, cover no control and are inside
    /// the safe area.
    #[test]
    fn notifications_cover_no_control() {
        let activities = [
            Activity::OnFoot,
            Activity::Driving(Driving::Car),
            Activity::Driving(Driving::Heli),
            Activity::Driving(Driving::Plane),
            Activity::Passenger,
            Activity::Parachute,
        ];
        let screens = [
            (vec2(874.0, 402.0), [62.0, 0.0, 62.0, 21.0]),
            (vec2(844.0, 390.0), [47.0, 0.0, 47.0, 21.0]),
            (vec2(1194.0, 834.0), [0.0, 24.0, 0.0, 20.0]),
        ];
        let (mut placed, mut tried) = (0, 0);
        for (screen, [l, t, r, b]) in screens {
            let safe = Rect::from_min_max(pos2(l, t), pos2(screen.x - r, screen.y - b));
            for scale in [0.7, 1.0, 1.5] {
                let controls = crate::touch::control_scale(screen, scale);
                let gear = Rect::from_center_size(gear_center(safe, controls), Vec2::splat(2.0 * GEAR_RADIUS * controls));
                let below = (30.0 + GEAR_RADIUS) * controls + 8.0;
                for activity in activities {
                    for (vehicle_near, phone) in [(false, false), (true, false), (false, true)] {
                        let context = PlayerContext {
                            activity,
                            vehicle_near,
                            phone,
                            menu: false,
                            cutscene: false,
                            bar: false,
                            scripted: false,
                            character: Character::Other,
                            gun: false,
                            cursor: false,
                            map: false,
                            pointer: false,
                            alert: false,
                            hidden: false,
                        };
                        let mut touch = Touch::new();
                        touch.layout(screen, safe, scale);
                        touch.set_context(context);
                        let mut taken = touch.occupied();
                        taken.push(gear);
                        let width = (TOAST_WIDTH * theme::layout_scale(safe)).min(safe.width() - 32.0);
                        let size = vec2(width, TOAST_HEIGHT * theme::layout_scale(safe));
                        tried += 1;
                        let Some(at) = toast_place(safe, below, size, &taken) else { continue };
                        placed += 1;
                        let rect = Rect::from_min_size(at, size);
                        assert!(safe.contains_rect(rect), "{screen:?} {scale} {context:?}: {rect:?}");
                        for t in &taken {
                            assert!(!t.intersects(rect), "{screen:?} {scale} {context:?}: {rect:?} on {t:?}");
                        }
                    }
                }
            }
        }
        // They show almost always.
        assert!(placed * 10 >= tried * 9, "shown {placed} of {tried} times");
    }
}

