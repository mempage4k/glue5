//! The application's own settings, in `glue5.json` in the user directory.
//! Graphics quality is the game's (its menu, Settings › Graphics, saved in
//! its `settings.xml`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::{MOBILE, TV};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The game directory, once found or chosen.
    pub game_dir: Option<PathBuf>,
    /// Where the game's files come from: a folder on this device, or a
    /// server (another device sharing its folder, or glue5-serve).
    pub source: Source,
    /// The server: `host` or `host:port`.
    pub server: String,
    /// This computer shares its game folder on the network (desktop).
    pub share: bool,
    /// The whole game comes from the server to this device (in its game
    /// folder), to play without it.
    pub download_all: bool,
    /// Automatic memory detection or a profile forced by the player.
    #[serde(default)]
    pub memory_profile: MemoryProfile,
    /// Legacy persisted setting. Read once, ignored so old `false` values do
    /// not suppress the new automatic profile, and never written again.
    #[serde(skip_serializing)]
    low_memory: Option<bool>,
    /// Whether the low-memory preset is in the game's settings (written at
    /// the first start with the effective low-memory profile; the game's menu
    /// changes it after).
    pub low_preset_written: bool,
    /// The player's own debug touch areas are shown and active over the
    /// game (Settings › Diagnostics). Editing them is a separate mode.
    pub debug_touch_areas_visible: bool,
    /// Touch areas the player taught the app over the game's screens.
    pub debug_touch_areas: Vec<DebugTouchArea>,
    /// The game's resolution as a fraction of the window's, per side (the
    /// back buffer follows the window). Chosen for the screen at the first
    /// start ([`Settings::for_screen`]).
    pub render_scale: Option<f32>,
    pub vsync: bool,
    /// At most this many frames per second; 0: no limit of the engine's
    /// (the display's refresh rate with VSync).
    pub max_fps: u32,
    /// How the game's frames are scaled up to the window (what the GPU has
    /// of it; the bicubic filter otherwise).
    pub upscaler: Upscaling,
    /// How much FSR sharpens its image, 0 to 1.
    pub sharpness: f32,
    /// A frame made between each two of the game's (MetalFX Temporal, on
    /// macOS and iOS 26): twice the frames shown, half a frame more of
    /// latency.
    pub frame_generation: bool,
    /// The GPU driver the user added that the renderer opens at launch
    /// (Android, Adreno GPUs: drivers.rs); none: the phone's own.
    pub gpu_driver: Option<String>,
    /// The performance overlay over the game, and how the application's
    /// own shows.
    pub overlay: PerfOverlay,
    pub overlay_detail: OverlayDetail,
    pub overlay_corner: Corner,
    /// Stop the Home stage's parallax, transitions and procedural movement.
    pub reduce_motion: bool,
    /// The start page's music, 0 to 1.
    pub music_volume: f32,
    /// The music is dulled while a menu is open over the start page.
    pub muffle_music: bool,
    /// The menus' own sounds: the ticks and clicks of moving through them.
    pub menu_sounds: bool,
    /// How loud they are, 0 to 1.
    pub sfx_volume: f32,
    pub touch: TouchMode,
    pub touch_opacity: f32,
    pub touch_size: f32,
    /// Which set of on-screen controls to show.
    pub touch_layout: TouchLayout,
    /// How the left thumb steers a car, bike or boat by touch.
    pub touch_steer: TouchSteer,
    /// How fast a finger's drag turns the camera.
    pub touch_camera: f32,
    /// The aim assist while playing by touch (the game's Targeting Mode,
    /// in place of its menu's).
    pub touch_aim: TouchAim,
    /// How aiming and firing work by touch.
    pub touch_fire: TouchFire,
    /// Gyroscope camera input in the Five Star control profile.
    pub touch_gyro_aim: GyroAim,
    /// Gyroscope camera sensitivity.
    pub touch_gyro_sensitivity: f32,
    /// Invert the vertical gyro camera axis.
    pub touch_gyro_invert_y: bool,
    /// Accelerometer tilt steering in the Five Star control profile.
    pub touch_tilt_steer: TiltSteer,
    /// Phone angle (degrees) needed for full tilt steering.
    pub touch_tilt_full_lock: f32,
    /// Touch-control taps and the game's rumble on devices with haptic feedback.
    pub haptics: bool,
    /// The phone, while out, takes taps on its own screen (the game's input
    /// device switches to keyboard/mouse for it). Off, the pad drives the
    /// phone instead (D-pad and buttons): an experimental switch, for
    /// telling a phone that cycles open/closed apart.
    pub phone_touchscreen: bool,
    /// The phone, while out on touch controls, is brought nearer so it
    /// shows bigger. Off, it keeps the game's own size and place: an
    /// experimental switch, for telling a phone that will not stay put
    /// apart.
    pub phone_enlarge: bool,
    /// The game starts in the sandbox (the map, no story) rather than the
    /// story: the start page's last choice.
    pub sandbox: bool,
    /// GTA Online was the start page's last choice.
    pub online: bool,
    /// The player's name online (made up when GTA Online is first opened).
    pub online_name: String,
    /// Friends' notifications show (Social Club's "Hide Notifications").
    pub friend_notices: bool,
    /// The mods in the user directory's `mods` folder can run (glue5-mods).
    pub mods: bool,
    /// With mods, the game serves GLUE 5 Studio.
    pub studio: bool,
    /// Other devices on the network may open the Studio (with its code).
    pub studio_lan: bool,
    /// The Studio's pairing code: kept, so that a browser paired once stays
    /// paired when the game starts again (the Mods page makes a new one).
    pub studio_code: String,
    /// Settings saved before the render scale: the back buffer's height.
    #[serde(skip_serializing)]
    render_height: Option<u32>,
    /// Settings saved before the choice of upscalers: MetalFX's spatial one
    /// on or off.
    #[serde(skip_serializing)]
    metalfx: Option<bool>,
    /// Settings saved before the choice of overlays: Apple's HUD on or off.
    #[serde(skip_serializing)]
    metal_hud: Option<bool>,
}

/// A rectangle the player placed over a game's screen element the app
/// cannot name (the game's Scaleform screens expose no element ids to the
/// host). [`Self::rect`] is the screen's fractions (left, top, right,
/// bottom), so it follows the window's size and rotation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DebugTouchArea {
    /// The player's name for what it does ("End call", ...).
    pub name: String,
    pub rect: [f32; 4],
    /// What a tap on it does.
    pub action: DebugTouchAction,
    /// The pad button sent when `action` asks for one.
    pub button: DebugTouchButton,
    /// The game's screens it shows on.
    pub when: DebugTouchWhen,
}

impl Default for DebugTouchArea {
    fn default() -> Self {
        Self {
            name: String::new(),
            rect: [0.4, 0.4, 0.6, 0.6],
            action: DebugTouchAction::Click,
            button: DebugTouchButton::Back,
            when: DebugTouchWhen::Always,
        }
    }
}

/// What a tap on a debug touch area does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebugTouchAction {
    /// A mouse click at the area's centre: the game decides what is there.
    #[default]
    Click,
    /// One pulse of a pad button (Back ends a call on the phone's home
    /// screens, where the game's own button may be small).
    PadButton,
}

/// A pad button a debug touch area can send.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebugTouchButton {
    /// B / Circle: back, end call.
    #[default]
    Back,
    /// A / Cross: confirm, select.
    Confirm,
    Up,
    Down,
    Left,
    Right,
}

/// The game's screens a debug touch area shows on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebugTouchWhen {
    /// Wherever its debug switch shows it.
    #[default]
    Always,
    /// Ordinary play: no phone, menu, cursor screen or scene over it.
    Gameplay,
    /// While the game's phone is out.
    Phone,
    /// Screens that take a pointer: the pause menu, shops, the browser.
    Pointer,
}

/// The memory behavior used for render targets, engine pools and the game's
/// first-run graphics preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryProfile {
    /// Choose low memory from the operating system's reported total RAM.
    #[default]
    Auto,
    /// Force the smaller memory budget.
    Low,
    /// Force the normal memory budget.
    Normal,
}

/// Where the game's files come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Folder,
    Server,
}

/// The performance overlay over the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PerfOverlay {
    Off,
    /// Apple's, drawn by Metal (shown from the launch it is on at).
    MetalHud,
    /// The application's (ui/perf.rs), on every platform.
    Glue5,
}

/// How much the application's overlay shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayDetail {
    /// One line: frame rate, frame time, GPU time.
    Minimal,
    /// The frame times as a graph, and where they go.
    Standard,
    /// The busiest threads, the renderer's counts, resolution and memory too.
    Full,
}

/// A corner of the safe area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// How the game's frames are scaled up to the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Upscaling {
    Bicubic,
    /// MetalFX's spatial upscaler (Apple).
    #[serde(rename = "metalfx")]
    MetalFx,
    /// MetalFX's temporal upscaler (Apple).
    #[serde(rename = "metalfx-temporal")]
    MetalFxTemporal,
    /// AMD's FSR 2 upscaler (temporal, on GPUs with the features it needs).
    Fsr,
}

impl Upscaling {
    pub fn upscaler(self) -> glue5_gpu::Upscaler {
        match self {
            Upscaling::Bicubic => glue5_gpu::Upscaler::Bicubic,
            Upscaling::MetalFx => glue5_gpu::Upscaler::MetalFx,
            Upscaling::MetalFxTemporal => glue5_gpu::Upscaler::MetalFxTemporal,
            Upscaling::Fsr => glue5_gpu::Upscaler::Fsr,
        }
    }
}

/// When the on-screen controls show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchMode {
    /// On touch screens, while no controller (or keyboard) is in use.
    Auto,
    Always,
    Off,
}

/// Which set of on-screen controls to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchLayout {
    /// Fewer buttons: what is used all the time is on screen, the rest is
    /// on the More button's fan (the controls made for touch).
    Modern,
    /// Every action on screen, as the first touch layouts had them.
    Classic,
    /// Five Star Port-style full contextual controls (Galaxy S21 Plus only).
    FiveStar,
}

/// When gyro rotation affects camera aim in the Five Star profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GyroAim {
    Off,
    Aiming,
    Always,
}

/// How phone tilt contributes to steering in the Five Star profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TiltSteer {
    Off,
    Full,
    Assist,
}

/// How the left thumb steers vehicles by touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchSteer {
    /// A stick where the left thumb lands.
    Stick,
    /// Left and right arrows.
    Arrows,
}

/// The aim assist while playing by touch: one of the game's targeting modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchAim {
    /// Aiming locks on to the nearest target and follows it (Assisted Aim -
    /// Full).
    LockOn,
    /// Aiming snaps to the target, then the finger aims (Assisted Aim -
    /// Partial).
    Snap,
    /// The camera slows down over targets (Free Aim - Assisted).
    SlowDown,
    /// Free Aim.
    Off,
}

/// How aiming and firing work by touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TouchFire {
    /// Holding Fire with a gun aims too: one thumb aims and fires.
    Auto,
    /// Holding Aim shows a second Fire button on the left, for the left thumb.
    Manual,
}

impl TouchAim {
    pub fn targeting(self) -> glue5_runtime::input::Targeting {
        use glue5_runtime::input::Targeting;
        match self {
            TouchAim::LockOn => Targeting::Full,
            TouchAim::Snap => Targeting::Partial,
            TouchAim::SlowDown => Targeting::FreeAssisted,
            TouchAim::Off => Targeting::Free,
        }
    }
}

/// SDL reports total physical memory (MemTotal), not the device's marketing
/// RAM. Auto assigns the low-memory profile to mobile devices below 8500 MB.
const LOW_MEMORY_RAM_MB: i32 = 8500;

/// The render scale's range, in steps of 5%.
pub const RENDER_SCALES: std::ops::RangeInclusive<f32> = 0.25..=1.0;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            game_dir: None,
            source: if TV { Source::Server } else { Source::Folder },
            server: String::new(),
            share: false,
            download_all: false,
            memory_profile: MemoryProfile::Auto,
            low_memory: None,
            low_preset_written: false,
            debug_touch_areas_visible: false,
            debug_touch_areas: Vec::new(),
            render_scale: None,
            vsync: true,
            // Phones hold a steady 60 for longer than they hold 120 (heat).
            max_fps: if MOBILE { 60 } else { 0 },
            // MetalFX is only provided on Apple GPUs; elsewhere start with
            // the spatial filter that is always available.
            upscaler: if cfg!(target_vendor = "apple") {
                Upscaling::MetalFx
            } else {
                Upscaling::Bicubic
            },
            sharpness: 0.5,
            frame_generation: false,
            gpu_driver: None,
            overlay: PerfOverlay::Off,
            overlay_detail: OverlayDetail::Standard,
            overlay_corner: Corner::TopLeft,
            touch: TouchMode::Auto,
            touch_opacity: 0.6,
            touch_size: 1.0,
            touch_layout: if MOBILE { TouchLayout::Modern } else { TouchLayout::Classic },
            touch_steer: TouchSteer::Stick,
            touch_camera: 1.0,
            touch_aim: TouchAim::LockOn,
            touch_fire: TouchFire::Auto,
            touch_gyro_aim: GyroAim::Off,
            touch_gyro_sensitivity: 1.0,
            touch_gyro_invert_y: false,
            touch_tilt_steer: TiltSteer::Off,
            touch_tilt_full_lock: 22.0,
            haptics: true,
            phone_touchscreen: true,
            phone_enlarge: true,
            sandbox: false,
            online: false,
            reduce_motion: false,
            music_volume: 1.0,
            muffle_music: true,
            menu_sounds: true,
            sfx_volume: 0.8,
            online_name: String::new(),
            friend_notices: true,
            mods: false,
            studio: true,
            // A phone's own browser is no place to write mods.
            studio_lan: crate::paths::MOBILE,
            studio_code: String::new(),
            render_height: None,
            metalfx: None,
            metal_hud: None,
        }
    }
}

/// A name for a new online player: "Player" and four digits.
pub fn default_online_name() -> String {
    format!("Player{:04}", random() % 10000)
}

fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new().build_hasher().finish()
}

impl Settings {
    fn file(user_dir: &Path) -> PathBuf {
        user_dir.join("glue5.json")
    }

    pub fn load(user_dir: &Path) -> Settings {
        let file = Self::file(user_dir);
        let mut settings = match std::fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                // Kept aside (the defaults are saved over it soon), not lost.
                let aside = file.with_extension("json.bad");
                match std::fs::rename(&file, &aside) {
                    Ok(()) => log::warn!("settings unreadable ({e}): kept as {}, the defaults instead", aside.display()),
                    Err(r) => log::warn!("settings unreadable ({e}), not kept aside ({r}): the defaults instead"),
                }
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        // Old installations persisted a boolean. Consume it, but let the new
        // Auto profile choose from current hardware instead of preserving false.
        let _ = settings.low_memory.take();
        if settings.metalfx.take() == Some(false) {
            settings.upscaler = Upscaling::Bicubic;
        }
        if settings.metal_hud.take() == Some(true) {
            settings.overlay = PerfOverlay::MetalHud;
        }
        if TV {
            settings.source = Source::Server;
        }
        settings
    }

    pub fn save(&self, user_dir: &Path) {
        let json = serde_json::to_vec_pretty(self).expect("settings serialise");
        // Through a copy: a write cut short (disk full, the app killed)
        // leaves the old file, not one that loads as the defaults.
        let file = Self::file(user_dir);
        let tmp = file.with_extension("json.tmp");
        if let Err(e) = std::fs::write(&tmp, json).and_then(|()| std::fs::rename(&tmp, &file)) {
            log::warn!("cannot save settings: {e}");
        }
    }

    /// Picks the render scale, the first time, for a window `height` pixels
    /// tall: the height an older settings file had, else about 1080 lines on
    /// desktop and 720 on phones and tablets (what the game ran at before:
    /// its GPU time grows with the pixels, and phone screens have many).
    pub fn for_screen(&mut self, height: u32) {
        if self.render_scale.is_none() {
            let lines = self.render_height.take().unwrap_or(if MOBILE { 720 } else { 1080 });
            self.render_scale = Some(round_scale(lines as f32 / height.max(1) as f32));
        }
    }

    pub fn render_scale(&self) -> f32 {
        self.render_scale.unwrap_or(1.0)
    }

    /// The effective memory budget selected by the explicit profile or Auto.
    pub fn low_memory(&self) -> bool {
        match self.memory_profile {
            MemoryProfile::Auto => MOBILE && sdl3::cpuinfo::system_ram() < LOW_MEMORY_RAM_MB,
            MemoryProfile::Low => true,
            MemoryProfile::Normal => false,
        }
    }
}

/// A render scale in the range, to its 5% step.
pub fn round_scale(scale: f32) -> f32 {
    ((scale * 20.0).round() / 20.0).clamp(*RENDER_SCALES.start(), *RENDER_SCALES.end())
}

/// The game's back buffer for a window of `pixels` at `scale`, as the
/// browser build sizes it: even sides (the engine halves them for its
/// post-processing), at least the 320x240 the engine takes, at most 4K
/// (1440p with low memory: render targets the size of the screen are most
/// of the GPU's memory).
pub fn render_size((w, h): (u32, u32), scale: f32, low_memory: bool) -> (u32, u32) {
    let cap = if low_memory { (2560.0, 1440.0) } else { (3840.0, 2160.0) };
    let (w, h) = (w as f32 * scale, h as f32 * scale);
    let k = (cap.0 / w).min(cap.1 / h).min(1.0);
    let side = |v: f32, min: u32| ((v * k / 2.0).round() as u32 * 2).max(min);
    (side(w, 320), side(h, 240))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_sizes() {
        assert_eq!(render_size((2560, 1440), 0.75, false), (1920, 1080));
        assert_eq!(render_size((2532, 1170), 0.6, false), (1520, 702));
        assert_eq!(render_size((300, 100), 0.5, false), (320, 240));
        assert_eq!(render_size((6016, 3384), 1.0, false), (3840, 2160));
        assert_eq!(render_size((6016, 3384), 1.0, true), (2560, 1440));
    }

    #[test]
    fn upscaler_and_overlay_from_older_settings() {
        let dir = std::env::temp_dir().join(format!("glue5-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("glue5.json"), r#"{"metalfx": false}"#).unwrap();
        assert_eq!(Settings::load(&dir).upscaler, Upscaling::Bicubic);
        std::fs::write(dir.join("glue5.json"), r#"{"metalfx": true}"#).unwrap();
        assert_eq!(Settings::load(&dir).upscaler, Upscaling::MetalFx);
        std::fs::write(dir.join("glue5.json"), r#"{"upscaler": "metalfx-temporal"}"#).unwrap();
        assert_eq!(Settings::load(&dir).upscaler, Upscaling::MetalFxTemporal);
        std::fs::write(dir.join("glue5.json"), r#"{"metal_hud": true}"#).unwrap();
        assert_eq!(Settings::load(&dir).overlay, PerfOverlay::MetalHud);
        std::fs::write(dir.join("glue5.json"), r#"{"metal_hud": false}"#).unwrap();
        assert_eq!(Settings::load(&dir).overlay, PerfOverlay::Off);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Settings that cannot be read are kept aside, not saved over.
    #[test]
    fn unreadable_settings_kept_aside() {
        let dir = std::env::temp_dir().join(format!("glue5-settings-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("glue5.json"), b"{\"online\": tru").unwrap();
        let s = Settings::load(&dir);
        assert_eq!(s.online, Settings::default().online);
        s.save(&dir);
        assert_eq!(std::fs::read(dir.join("glue5.json.bad")).unwrap(), b"{\"online\": tru");
        assert!(serde_json::from_slice::<Settings>(&std::fs::read(dir.join("glue5.json")).unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn old_low_memory_boolean_migrates_to_auto_without_clearing_game_preset_state() {
        let dir = std::env::temp_dir().join(format!("glue5-settings-memory-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("glue5.json"), r#"{"low_memory": false, "low_preset_written": true}"#).unwrap();
        let settings = Settings::load(&dir);
        assert_eq!(settings.memory_profile, MemoryProfile::Auto);
        assert!(settings.low_preset_written);
        settings.save(&dir);
        let saved = std::fs::read_to_string(dir.join("glue5.json")).unwrap();
        assert!(saved.contains("\"memory_profile\": \"auto\""));
        assert!(!saved.contains("\"low_memory\""));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn memory_profiles_override_auto_and_round_trip() {
        let mut settings = Settings::default();
        settings.memory_profile = MemoryProfile::Low;
        assert!(settings.low_memory());
        settings.memory_profile = MemoryProfile::Normal;
        assert!(!settings.low_memory());
        settings.memory_profile = MemoryProfile::Auto;
        assert_eq!(
            settings.low_memory(),
            MOBILE && sdl3::cpuinfo::system_ram() < LOW_MEMORY_RAM_MB
        );
        assert_eq!(serde_json::to_string(&MemoryProfile::Auto).unwrap(), "\"auto\"");
        assert_eq!(serde_json::to_string(&MemoryProfile::Low).unwrap(), "\"low\"");
        assert_eq!(serde_json::to_string(&MemoryProfile::Normal).unwrap(), "\"normal\"");
    }

    #[test]
    fn five_star_layout_is_saved_without_changing_the_old_layout_values() {
        assert_eq!(serde_json::to_string(&TouchLayout::Modern).unwrap(), "\"modern\"");
        assert_eq!(serde_json::to_string(&TouchLayout::Classic).unwrap(), "\"classic\"");
        assert_eq!(serde_json::to_string(&TouchLayout::FiveStar).unwrap(), "\"fivestar\"");
        assert_eq!(serde_json::from_str::<TouchLayout>("\"fivestar\"").unwrap(), TouchLayout::FiveStar);
    }

    #[test]
    fn older_settings_default_to_stick_steering() {
        let settings: Settings = serde_json::from_str(r#"{"touch_layout":"classic"}"#).unwrap();
        assert_eq!(settings.touch_steer, TouchSteer::Stick);
        assert_eq!(settings.touch_fire, TouchFire::Auto);
        assert_eq!(settings.touch_layout, TouchLayout::Classic);
        assert_eq!(settings.touch_gyro_aim, GyroAim::Off);
        assert_eq!(settings.touch_tilt_steer, TiltSteer::Off);
        assert!(!settings.reduce_motion, "older settings default to animated backgrounds");
    }

    #[test]
    fn first_scale() {
        let mut s = Settings { render_height: Some(720), ..Settings::default() };
        s.for_screen(1440);
        assert_eq!(s.render_scale, Some(0.5));
        s.for_screen(720);
        assert_eq!(s.render_scale, Some(0.5), "chosen once");
        let mut s = Settings::default();
        s.for_screen(1170);
        assert_eq!(s.render_scale, Some(if MOBILE { 0.6 } else { 0.9 }));
    }
}
