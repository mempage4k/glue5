//! The pages' lists — settings, friends, servers, mods, credits — all built
//! from the same rows and painted inside the shell (`ui::shell`). The PLAY
//! tab's own page is `ui::menu`.
use super::widgets::{Control, action_button, action_buttons, choice, focus, setting, slider, text_field, toggle};
use super::{Action, Mode, Page, SettingsTab, Status, TextKind, WARN, theme};
use crate::drivers::Drivers;
use crate::paths::{MOBILE, TV};
use crate::settings::{
    self, Corner, DebugTouchAction, DebugTouchButton, DebugTouchWhen, GyroAim, MemoryProfile, OverlayDetail,
    PerfOverlay, RENDER_SCALES, Settings, Source, TiltSteer, TouchAim, TouchFire, TouchLayout, TouchMode, TouchSteer,
    Upscaling,
};
use crate::sharing::State as Sharing;
use glue5_stream::Phase;
use egui::{Color32, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};

/// A status dot: the game's files are there.
pub(super) const READY: Color32 = theme::palette::ACCENT;

/// A card's title, and on its right a status: a dot and a word.
pub(super) fn card_title(ui: &mut egui::Ui, title: &str, state: Option<(&str, Color32)>) {
    let scale = theme::density(ui);
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0 * scale), Sense::hover());
    let painter = ui.painter();
    let mut title_width = row.width();
    if let Some((word, color)) = state {
        let text = painter.layout_no_wrap(word.into(), egui::FontId::proportional(11.0 * scale), Color32::WHITE);
        let pill = Rect::from_min_max(
            pos2(row.right() - text.size().x - 28.0 * scale, row.center().y - 11.0 * scale),
            pos2(row.right(), row.center().y + 11.0 * scale),
        );
        title_width = pill.left() - row.left() - 8.0 * scale;
        painter.rect_filled(pill, 10, theme::expressive::SURFACE_HIGH);
        painter.circle_filled(pos2(pill.left() + 11.0 * scale, pill.center().y), 3.5 * scale, color);
        painter.galley(pos2(pill.left() + 19.0 * scale, pill.center().y - text.size().y * 0.5), text, Color32::WHITE);
    }
    // A long title (a computer's name) ends with … before the pill.
    let font = egui::FontId::new(17.0 * scale, egui::FontFamily::Name("condensed".into()));
    let mut job = egui::text::LayoutJob::simple_singleline(title.into(), font, Color32::WHITE);
    job.wrap = egui::text::TextWrapping::truncate_at_width(title_width.max(1.0));
    let galley = painter.layout_job(job);
    painter.galley(pos2(row.left(), row.center().y - galley.size().y * 0.5), galley, Color32::WHITE);
}

pub(super) fn small(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).small().color(color));
}

/// A thin bar: how far something is (0 to 1).
pub(super) fn rail(ui: &mut egui::Ui, done: f32) {
    let scale = theme::density(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0 * scale), Sense::hover());
    ui.painter().rect_filled(rect, 0, theme::expressive::OUTLINE);
    let filled = Rect::from_min_size(rect.min, vec2(rect.width() * done.clamp(0.0, 1.0), rect.height()));
    ui.painter().rect_filled(filled, 0, theme::expressive::PRIMARY);
}

pub(super) fn bytes(n: u64) -> String {
    if n >= 1 << 30 { format!("{:.1} GB", n as f64 / 1e9) } else { format!("{:.0} MB", n as f64 / 1e6) }
}

/// The stream's state in a word, for a card's pill.
pub(super) fn stream_state(s: &glue5_stream::Status) -> (&'static str, Color32) {
    match &s.phase {
        Phase::Failed(_) => ("Stopped", WARN),
        Phase::Ready if !s.connected => ("Offline", WARN),
        Phase::Ready => ("Ready", READY),
        Phase::Syncing { .. } => ("Taking files", theme::MUTED),
        Phase::Connecting => ("Connecting", WARN),
    }
}

/// What the stream is doing: a line, and a bar while it takes something.
pub(super) fn stream_progress(ui: &mut egui::Ui, s: &glue5_stream::Status) {
    match &s.phase {
        Phase::Failed(why) => small(ui, why, WARN),
        Phase::Connecting => match &s.error {
            Some(e) => small(ui, e, WARN),
            None => small(ui, "Looking for the server…", theme::MUTED),
        },
        Phase::Syncing { what, done, total } => {
            let what = if *what == "text" { "small files" } else { "shaders" };
            let done = *done as f32 / (*total).max(1) as f32;
            small(ui, &format!("Taking the {what}, once: {:.0}%", done * 100.0), theme::MUTED);
            rail(ui, done);
        }
        Phase::Ready if s.complete => small(ui, "The whole game is on this device.", theme::MUTED),
        Phase::Ready if !s.connected => small(ui, "Playing from this device's copy until the server answers.", theme::MUTED),
        Phase::Ready if s.filling => {
            let done = s.cache_bytes as f32 / s.total_bytes.max(1) as f32;
            let line = format!("Downloading the whole game: {} of {}", bytes(s.cache_bytes), bytes(s.total_bytes));
            small(ui, &line, theme::MUTED);
            if s.no_room {
                small(ui, "Not enough space on this device: the download waits for some.", WARN);
            }
            rail(ui, done);
        }
        Phase::Ready if s.boot.0 < s.boot.1 => {
            let done = s.boot.0 as f32 / s.boot.1 as f32;
            small(ui, &format!("Taking what every start reads: {:.0}%", done * 100.0), theme::MUTED);
            rail(ui, done);
        }
        Phase::Ready => {}
    }
}

/// A server's name, if the network search found it, else its address.
pub(super) fn server_name(status: &Status, address: &str) -> String {
    status.servers.iter().find(|s| is_server(s, address)).map_or_else(|| address.to_owned(), |(f, _)| f.name.clone())
}

/// Whether a server found is the one at `address`: by its name, or by an
/// address its name stands for (a Mac found as `name.local`, typed as its IP).
pub(super) fn is_server((found, check): &super::FoundServer, address: &str) -> bool {
    let ip = address.parse::<std::net::SocketAddr>().ok().or_else(|| {
        let ip = address.parse::<std::net::IpAddr>().ok()?;
        Some(std::net::SocketAddr::new(ip, glue5_stream::PORT))
    });
    found.address == address
        || ip.is_some_and(|ip| matches!(check, Some(glue5_stream::Check::Ready { at, .. }) if at.contains(&ip)))
}

/// The server in use in a few words: its name, and how it is doing.
fn server_summary(settings: &Settings, status: &Status) -> String {
    let server = settings.server.trim();
    if server.is_empty() {
        return "None chosen yet".to_owned();
    }
    let name = server_name(status, server);
    match status.stream.filter(|(s, _)| *s == server) {
        Some((_, s)) => format!("{name} · {}", stream_state(s).0),
        // The game runs from another one; this one is for the next start.
        None if status.started.is_some() => format!("{name} · next start"),
        None => name,
    }
}

/// The settings, over the start page or the game. True when closed.
#[allow(clippy::too_many_arguments)]
/// The settings' nine categories, in the pause menu's rhythm.
const CATEGORIES: [(SettingsTab, &str); 9] = [
    (SettingsTab::Graphics, "GRAPHICS"),
    (SettingsTab::Display, "DISPLAY"),
    (SettingsTab::Audio, "AUDIO"),
    (SettingsTab::Controls, "CONTROLS"),
    (SettingsTab::Gameplay, "GAMEPLAY"),
    (SettingsTab::Files, "FILES"),
    (SettingsTab::Network, "NETWORK"),
    (SettingsTab::Advanced, "ADVANCED"),
    (SettingsTab::Diagnostics, "DIAGNOSTICS"),
];

pub(super) fn settings_page(
    ctx: &egui::Context,
    settings: &mut Settings,
    status: &Status,
    note: Option<&str>,
    tab: &mut SettingsTab,
    scale_edit: &mut Option<f32>,
    actions: &mut Vec<Action>,
    go: &mut Option<Option<Page>>,
) -> bool {
    sheet(ctx, "settings-window", "SETTINGS", |ui| {
        let scale = theme::density(ui);
        categories(ui, tab, scale);
        ui.add_space(6.0 * scale);
        if let Some(note) = note {
            ui.label(RichText::new(note).size(11.5 * scale).color(WARN));
        }
        settings_items(ui, settings, status, *tab, scale_edit, actions, go);
    })
}

/// The settings sub-navigation: a compact grid, so every category is visible
/// and tappable without a hidden horizontal scroll strip.
fn categories(ui: &mut egui::Ui, tab: &mut SettingsTab, scale: f32) {
    let gap = 3.0 * scale;
    let available = ui.available_width();
    // In Android landscape the five short labels fit in two rows, leaving
    // more of the viewport for actual settings. Narrow layouts fall back to
    // three or two columns rather than clipping category names.
    let columns = if available >= 340.0 * scale { 5 } else if available >= 220.0 * scale { 3 } else { 2 };
    let button_w = ((available - gap * (columns - 1) as f32) / columns as f32).max(1.0);
    let button_h = 27.0 * scale;
    for row in CATEGORIES.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for &(value, name) in row {
                let here = *tab == value;
                let (rect, response) = ui.allocate_exact_size(vec2(button_w, button_h), Sense::click());
                response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, here, name));
                let painter = ui.painter();
                painter.rect_filled(rect, 0, if here { theme::palette::TEXT_HI } else { theme::palette::INK_800 });
                painter.rect_stroke(
                    rect,
                    0,
                    Stroke::new(1.0, if here { theme::palette::TEXT_HI } else { theme::palette::LINE }),
                    StrokeKind::Inside,
                );
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    name,
                    egui::FontId::new(9.5 * scale, theme::fonts::strong()),
                    if here { theme::palette::INK_950 } else { theme::palette::TEXT_MID },
                );
                if response.has_focus() && !here {
                    painter.rect_stroke(rect, 0, Stroke::new(1.0, theme::palette::LIME), StrokeKind::Inside);
                }
                if focus(response).clicked() && !here {
                    *tab = value;
                }
            }
        });
    }
    ui.add_space(3.0 * scale);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, theme::palette::SEPARATOR);
}

/// A line with a switch: whether it changed.
fn switch_row(ui: &mut egui::Ui, label: &str, help: &str, on: &mut bool) -> bool {
    let before = *on;
    super::widgets::row(ui, label, help, |ui| toggle(ui, &mut *on));
    *on != before
}

/// A line with a value picked from a few: whether it changed.
fn pick_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let before = *value;
    super::widgets::row(ui, label, help, |ui| choice(ui, &mut *value, options));
    *value != before
}

/// A line with a slider: whether it changed.
fn slide_row(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
    suffix: &str,
    decimals: usize,
) -> bool {
    let before = *value;
    super::widgets::row(ui, label, help, |ui| slider(ui, &mut *value, range, step, suffix, decimals));
    (*value - before).abs() > f32::EPSILON
}

/// A line that does something when it is taken: whether it was.
fn action_row(ui: &mut egui::Ui, label: &str, help: &str, text: &str) -> bool {
    let response = super::widgets::row(ui, label, help, |ui| super::widgets::action_value(ui, text));
    response.clicked()
}

/// A line that only says something (a path, a resolution, a state).
fn info_row(ui: &mut egui::Ui, label: &str, help: &str, text: &str) {
    super::widgets::row(ui, label, help, |ui| super::widgets::value_text(ui, text));
}

/// A menu page inside the app's one shell (`ui::shell`): the header, the tab
/// strip, the rows `add` draws, the description of the line the pointer is on,
/// and the footer — full screen, with the start page's scene behind it. True
/// when the player asked to go back.
pub(super) fn sheet(ctx: &egui::Context, _id: &str, title: &str, add: impl FnOnce(&mut egui::Ui)) -> bool {
    super::shell::frame(ctx, title, add)
}

fn settings_items(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    status: &Status,
    tab: SettingsTab,
    scale_edit: &mut Option<f32>,
    actions: &mut Vec<Action>,
    go: &mut Option<Option<Page>>,
) {
    let scale = theme::density(ui);
    let _ = scale; // Android's diagnostics only
    let started = status.started;
    match tab {
        SettingsTab::Graphics => {
            let mut value = scale_edit.unwrap_or(settings.render_scale());
            let (rw, rh) = settings::render_size(status.window, value, settings.low_memory());
            let help = format!("{rw} × {rh} · the game is drawn at this scale and stretched to the screen");
            let r = super::widgets::row(ui, "Resolution scale", &help, |ui| {
                slider(ui, &mut value, RENDER_SCALES, 0.05, "%", 0)
            });
            if r.dragged() {
                *scale_edit = Some(value);
            } else if r.changed() || r.drag_stopped() {
                settings.render_scale = Some(settings::round_scale(value));
                *scale_edit = None;
            }
            if status.upscalers.len() > 1 {
                let options: Vec<_> = status
                    .upscalers
                    .iter()
                    .map(|&u| {
                        let name = match u {
                            Upscaling::Bicubic => "Bicubic",
                            Upscaling::MetalFx => "MetalFX",
                            Upscaling::MetalFxTemporal => "MetalFX Temporal",
                            Upscaling::Fsr => "FSR 2",
                        };
                        (u, name)
                    })
                    .collect();
                pick_row(ui, "Upscaling", "How the frame is stretched to the screen", &mut settings.upscaler, &options);
            }
            if settings.upscaler == Upscaling::Fsr {
                slide_row(ui, "Sharpening", "FSR 2's sharpening", &mut settings.sharpness, 0.0..=1.0, 0.05, "%", 0);
            }
            if status.frame_generation && settings.upscaler == Upscaling::MetalFxTemporal {
                switch_row(
                    ui,
                    "Frame generation",
                    "A frame between each two of the game's: twice the frames, half a frame of latency",
                    &mut settings.frame_generation,
                );
            }
            switch_row(ui, "VSync", "Wait for the display between frames", &mut settings.vsync);
            let unlimited = if settings.vsync { "Display" } else { "Unlimited" };
            let mut rates = vec![(0, unlimited), (60, "60")];
            if status.refresh_rate >= 119.0 {
                rates.push((40, "40"));
            }
            rates.push((30, "30"));
            let differs = started.is_some_and(|s| s.max_fps != settings.max_fps);
            // A frame rate changed here applies at the next start: the row's
            // help says so while it differs from the running game's.
            let _ = pick_row(
                ui,
                "Frame rate",
                next_start(differs).unwrap_or("Frames per second at most"),
                &mut settings.max_fps,
                &rates,
            );
            // Android only: the phone's heat holds the game back.
            #[cfg(target_os = "android")]
            {
                let help = match (settings.thermal_adapt, status.thermal) {
                    (settings::ThermalAdapt::Off, _) => {
                        "Off: the game runs as it is, however hot the phone is".to_string()
                    }
                    (_, Some(t)) if t.level > 0 => {
                        format!("The phone is {}: the game is held to {} fps", t.state(), t.fps)
                    }
                    (_, Some(t)) if t.status >= 0 => {
                        format!("The phone is {}: the game runs as it is", t.state())
                    }
                    _ => "As the phone heats up: fewer frames per second, and a smaller image at the last step"
                        .to_string(),
                };
                let options = [
                    (settings::ThermalAdapt::Off, "Off"),
                    (settings::ThermalAdapt::Cap, "Frame rate"),
                    (settings::ThermalAdapt::CapAndScale, "Frame rate and resolution"),
                ];
                let _ = pick_row(ui, "Thermal back-off", &help, &mut settings.thermal_adapt, &options);
            }
            let low_before = settings.low_memory();
            let low_differs = started.is_some_and(|s| s.low_memory != low_before);
            let memory_profiles = [
                (MemoryProfile::Auto, "Auto"),
                (MemoryProfile::Low, "Low"),
                (MemoryProfile::Normal, "Normal"),
            ];
            if pick_row(
                ui,
                "Memory profile",
                next_start(low_differs).unwrap_or("Auto uses total RAM; Low and Normal force a profile"),
                &mut settings.memory_profile,
                &memory_profiles,
            ) && !low_before
                && settings.low_memory()
            {
                settings.low_preset_written = false;
            }
            // Phones: the game's own settings turned down for the GPU.
            #[cfg(target_os = "android")]
            {
                let help = match settings.graphics_preset {
                    settings::GraphicsPreset::Off => {
                        "The game's settings are its own; Restore graphics undoes a preset".to_string()
                    }
                    settings::GraphicsPreset::Phone => {
                        "Smaller textures, no reflections, no soft shadows, at the next start".to_string()
                    }
                };
                let options = [
                    (settings::GraphicsPreset::Off, "Off"),
                    (settings::GraphicsPreset::Phone, "Phone"),
                ];
                if pick_row(ui, "Graphics preset", &help, &mut settings.graphics_preset, &options) {
                    // Written over the game's settings at the next start
                    // (the engine reads them then).
                    settings.graphics_preset_written = false;
                }
            }
            let reset_help = if status.graphics_reset {
                "The game's graphics settings are already the defaults"
            } else {
                "The game's own graphics settings, back to the engine's defaults"
            };
            if action_row(ui, "Restore graphics", reset_help, "RESET") && !status.graphics_reset && status.game_dir.is_some() {
                super::popup::ask(super::popup::Request::reset_graphics());
            }
        }
        SettingsTab::Display => {
            info_row(
                ui,
                "Screen",
                "The window, and the display's refresh rate (the app fits every menu to it)",
                &format!("{} × {} · {:.0} Hz", status.window.0, status.window.1, status.refresh_rate),
            );
            if let Some((w, h)) = status.back_buffer {
                info_row(ui, "Game buffer", "What the game draws, before it is stretched", &format!("{w} × {h}"));
            }
            switch_row(
                ui,
                "Reduce motion",
                "Stop the start page's parallax and its scene changes",
                &mut settings.reduce_motion,
            );
            info_row(
                ui,
                "Menu scale",
                "Every menu follows the screen's height; the rows stay at the size of a thumb",
                &format!("{:.0} %", theme::layout_scale(ui.ctx().content_rect()) * 100.0),
            );
        }
        SettingsTab::Audio => {
            slide_row(ui, "Music", "The start page's track, before the game", &mut settings.music_volume, 0.0..=1.0, 0.05, "%", 0);
            switch_row(
                ui,
                "Muffle with the menu",
                "The music keeps playing behind a menu, dulled instead of stopped",
                &mut settings.muffle_music,
            );
            switch_row(ui, "Menu sounds", "The ticks and clicks of moving through the menus", &mut settings.menu_sounds);
            if settings.menu_sounds {
                slide_row(ui, "Sounds volume", "Beside the music, which has its own", &mut settings.sfx_volume, 0.0..=1.0, 0.05, "%", 0);
                let found = crate::sfx::installed(&crate::paths::user_dir().unwrap_or_default(), status.game_dir);
                info_row(
                    ui,
                    "Sound files",
                    "The menu sounds are the player's own: a sfx folder beside scenes, or glue5-sfx beside the game",
                    if found { "FOUND" } else { "NONE" },
                );
            }
        }
        SettingsTab::Controls => {
            pick_row(
                ui,
                "Touch controls",
                "Auto: shown on touch screens, hidden while a controller or keyboard is used",
                &mut settings.touch,
                &[(TouchMode::Auto, "Auto"), (TouchMode::Always, "On"), (TouchMode::Off, "Off")],
            );
            switch_row(
                ui,
                "Phone touchscreen input",
                "On: use mouse-mode input while the phone is out. Off: keep pad mode (D-pad and buttons). A tap may still accept the current item until direct hit-testing is implemented",
                &mut settings.phone_touchscreen,
            );
            switch_row(
                ui,
                "Bigger phone",
                "Experimental: enlarge the phone while touch controls are active. Off keeps the game's own size and position; enlargement can make the phone flicker on some builds",
                &mut settings.phone_enlarge,
            );
            if crate::device::is_galaxy_s21_plus() {
                pick_row(
                    ui,
                    "Layout",
                    "Modern and Classic are Glue5's controls. Five Star switches to the full contextual touch controls for Galaxy S21 Plus.",
                    &mut settings.touch_layout,
                    &[
                        (TouchLayout::Modern, "Modern"),
                        (TouchLayout::Classic, "Classic"),
                        (TouchLayout::FiveStar, "Five Star"),
                    ],
                );
            } else {
                pick_row(
                    ui,
                    "Layout",
                    "Modern: the actions used all the time on screen, the rest on the More button's fan. Classic: every action on screen",
                    &mut settings.touch_layout,
                    &[(TouchLayout::Modern, "Modern"), (TouchLayout::Classic, "Classic")],
                );
            }
            pick_row(
                ui,
                "Car steering",
                "Stick: the left thumb steers where it lands. Arrows: left and right buttons (slide between them)",
                &mut settings.touch_steer,
                &[(TouchSteer::Stick, "Stick"), (TouchSteer::Arrows, "Arrows")],
            );
            slide_row(ui, "Size", "The on-screen controls' size", &mut settings.touch_size, 0.7..=1.5, 0.1, "×", 1);
            slide_row(ui, "Opacity", "How solid they are over the game", &mut settings.touch_opacity, 0.2..=1.0, 0.01, "%", 0);
            slide_row(ui, "Camera speed", "How fast a finger's drag turns the camera", &mut settings.touch_camera, 0.25..=3.0, 0.05, "×", 2);
            pick_row(
                ui,
                "Aim assist",
                "Lock-on follows the nearest target; Snap snaps, then you aim; Slow-down slows over targets",
                &mut settings.touch_aim,
                &[
                    (TouchAim::LockOn, "Lock-on"),
                    (TouchAim::Snap, "Snap"),
                    (TouchAim::SlowDown, "Slow-down"),
                    (TouchAim::Off, "Off"),
                ],
            );
            pick_row(
                ui,
                "Aim & fire",
                "Auto: holding Fire with a gun aims too. Manual: holding Aim shows a second Fire button on the left",
                &mut settings.touch_fire,
                &[(TouchFire::Auto, "Auto"), (TouchFire::Manual, "Manual")],
            );
            if settings.touch_layout == TouchLayout::FiveStar && crate::device::is_galaxy_s21_plus() {
                pick_row(
                    ui,
                    "Gyro aim",
                    "Use the Galaxy S21 Plus gyroscope to fine-tune the camera while aiming, or always while on foot.",
                    &mut settings.touch_gyro_aim,
                    &[(GyroAim::Off, "Off"), (GyroAim::Aiming, "While aiming"), (GyroAim::Always, "Always on foot")],
                );
                slide_row(
                    ui,
                    "Gyro sensitivity",
                    "How strongly phone rotation moves the camera",
                    &mut settings.touch_gyro_sensitivity,
                    0.2..=3.0,
                    0.05,
                    "×",
                    2,
                );
                switch_row(ui, "Invert gyro Y", "Invert vertical camera movement from phone rotation", &mut settings.touch_gyro_invert_y);
                pick_row(
                    ui,
                    "Tilt steering",
                    "Off: touch only. Assist: tilt adds steering. Full: tilt steers when the steering stick is idle.",
                    &mut settings.touch_tilt_steer,
                    &[(TiltSteer::Off, "Off"), (TiltSteer::Assist, "Assist"), (TiltSteer::Full, "Full")],
                );
                slide_row(
                    ui,
                    "Tilt lock",
                    "Phone angle for full steering lock",
                    &mut settings.touch_tilt_full_lock,
                    10.0..=45.0,
                    1.0,
                    "°",
                    0,
                );
            }
            if crate::haptics::SUPPORTED {
                switch_row(ui, "Haptics", "Taps and the game's rumble, on devices that have it", &mut settings.haptics);
            }
        }
        SettingsTab::Gameplay => {
            // The same two flags the start page keeps: what Play starts.
            let mut mode = if settings.online {
                Mode::Online
            } else if settings.sandbox {
                Mode::Sandbox
            } else {
                Mode::Story
            };
            if pick_row(
                ui,
                "Mode",
                "What PLAY starts with: the story, GTA Online, or the whole map without missions",
                &mut mode,
                &[(Mode::Story, "Story"), (Mode::Online, "Online"), (Mode::Sandbox, "Sandbox")],
            ) {
                settings.online = mode == Mode::Online;
                settings.sandbox = mode == Mode::Sandbox;
            }
            super::widgets::row(ui, "Player name", "The name other players see in GTA Online", |ui| {
                text_field(ui, "", TextKind::Name, &mut settings.online_name, "Player name", 24)
            });
            switch_row(ui, "Mods", "The mods in the mods folder run with the game", &mut settings.mods);
            if action_row(ui, "Mods' menus", "The mods' own menus, over the game (F7)", "OPEN") {
                *go = Some(Some(Page::Mods));
            }
        }
        SettingsTab::Files => {
            if !TV {
                let differs = started.is_some_and(|s| s.source != settings.source);
                // A new source applies at the next start; the rows below say
                // what the source in use is.
                let _ = pick_row(
                    ui,
                    "Game files",
                    next_start(differs).unwrap_or("Folder: the game on this device. Server: from a computer that shares it"),
                    &mut settings.source,
                    &[(Source::Folder, "Folder"), (Source::Server, "Server")],
                );
            }
            match settings.source {
                Source::Folder => game_folder(ui, settings, status, actions),
                Source::Server => {
                    let summary = server_summary(settings, status);
                    if action_row(ui, "Server", &format!("{summary} · the others on this network, this device's copy"), "OPEN") {
                        *go = Some(Some(Page::Servers));
                    }
                }
            }
            if action_row(
                ui,
                "Download the whole game",
                "Stream it all to this device while it plays, instead of what it reads",
                if settings.download_all { "ON" } else { "OFF" },
            ) {
                settings.download_all = !settings.download_all;
            }
            if !MOBILE {
                switch_row(
                    ui,
                    "Share this folder",
                    "Other devices on this network may stream the game from this computer",
                    &mut settings.share,
                );
            }
            if status.game_dir.is_some() && action_row(ui, "Clear the local copy", "Deletes the copy this device streams, not your game folder", "CLEAR") {
                super::popup::ask(super::popup::Request::clear_copy());
            }
            #[cfg(target_os = "android")]
            {
                if started.is_none() {
                    if action_row(ui, "Import save", "A GTA V profile folder from this phone", "CHOOSE…") {
                        actions.push(Action::ImportSave);
                    }
                    if action_row(
                        ui,
                        "Import game resources",
                        "A folder whose data/ and b/ paths match the game; existing files at the same path are replaced",
                        "CHOOSE…",
                    ) && status.source == Source::Folder
                        && status.game_dir.is_some()
                    {
                        actions.push(Action::ImportResources);
                    }
                }
            }
        }
        SettingsTab::Network => {
            let asking = match status.friend_requests {
                0 => "Your friends, requests and invites".to_owned(),
                n => format!("{n} waiting: friend requests, game invites"),
            };
            if action_row(ui, "Friends", &asking, "OPEN") {
                *go = Some(Some(Page::Friends));
            }
            switch_row(ui, "Friends' notifications", "Online, requests and invites, over the game", &mut settings.friend_notices);
            if action_row(ui, "Join a friend", "With the key a friend gave you", "OPEN") {
                *go = Some(Some(Page::Friends));
            }
            if let Some((address, state)) = status.stream {
                let (name, _) = stream_state(state);
                info_row(ui, "Server", "The computer this device streams from", &format!("{name} · {address}"));
            }
        }
        SettingsTab::Advanced => {
            let mut overlays = vec![(PerfOverlay::Off, "Off")];
            if cfg!(target_vendor = "apple") {
                overlays.push((PerfOverlay::MetalHud, "Metal HUD"));
            }
            overlays.push((PerfOverlay::Glue5, "Glue 5"));
            let metal_differs = settings.overlay == PerfOverlay::MetalHud && !status.metal_hud;
            // Metal HUD shows from the next launch: the row's help says so.
            let _ = pick_row(
                ui,
                "Performance overlay",
                next_start(metal_differs).unwrap_or("Frames, the GPU and the frame's time, over the game"),
                &mut settings.overlay,
                &overlays,
            );
            if settings.overlay == PerfOverlay::Glue5 {
                pick_row(
                    ui,
                    "Overlay detail",
                    "How much it shows",
                    &mut settings.overlay_detail,
                    &[
                        (OverlayDetail::Minimal, "Minimal"),
                        (OverlayDetail::Standard, "Standard"),
                        (OverlayDetail::Full, "Full"),
                    ],
                );
                pick_row(
                    ui,
                    "Overlay corner",
                    "Where it sits over the game",
                    &mut settings.overlay_corner,
                    &[
                        (Corner::TopLeft, "Top left"),
                        (Corner::TopRight, "Top right"),
                        (Corner::BottomLeft, "Bottom left"),
                        (Corner::BottomRight, "Bottom right"),
                    ],
                );
            }
            switch_row(ui, "GLUE 5 Studio", "With mods, the game serves the Studio's pages", &mut settings.studio);
            if settings.studio {
                switch_row(ui, "Studio on the network", "Other devices may open it, with its pairing code", &mut settings.studio_lan);
                info_row(
                    ui,
                    "Pairing code",
                    "A browser paired once stays paired when the game starts again",
                    if settings.studio_code.is_empty() { "NONE YET" } else { settings.studio_code.as_str() },
                );
            }
            if let Some(drivers) = status.drivers {
                gpu_driver(ui, settings, drivers, started.is_none(), actions);
            }
        }
        SettingsTab::Diagnostics => {
            #[cfg(target_os = "android")]
            {
                ui.label(
                    RichText::new(
                        "Only entries from the GLUE 5 app process are captured; other apps are excluded. These logs may contain app paths or diagnostic data.",
                    )
                    .size(11.5 * scale)
                    .color(theme::palette::TEXT_MID),
                );
                if action_row(ui, "Start a capture", "Ask Shizuku for permission, then begin capturing", "START") {
                    actions.push(Action::StartLogcatCapture);
                }
                if action_row(ui, "Save the capture", "Stop the live stream and open save/share options", "SAVE") {
                    actions.push(Action::StopLogcatCapture);
                }
                if action_row(ui, "Previous session", "The last GLUE 5 process capture, through Shizuku", "CAPTURE") {
                    actions.push(Action::CaptureSessionLogcat);
                }
                if action_row(
                    ui,
                    "Import report",
                    "Copy the Android import paths, outcomes, save-file names and GPU details",
                    "COPY",
                ) {
                    actions.push(Action::CopyImportDiagnostics);
                }
                for (label, marker) in [
                    ("Screen freezes", super::DebugMarker::ScreenFrozen),
                    ("Audio continues", super::DebugMarker::AudioContinues),
                    ("Controls still respond", super::DebugMarker::ControlsRespond),
                    ("Controls stop responding", super::DebugMarker::ControlsUnresponsive),
                ] {
                    if action_row(ui, label, "Adds a timestamped line to the GLUE 5 log", "MARK") {
                        actions.push(Action::MarkDebugEvent(marker));
                    }
                }
            }
            switch_row(
                ui,
                "Debug touch areas",
                "Show the areas I taught the app over the game, and do what they say",
                &mut settings.debug_touch_areas_visible,
            );
            if action_row(ui, "Edit debug touch areas", "Close this page: draw a new area, drag one, or pull its corners", "EDIT") {
                actions.push(Action::EditDebugTouchAreas);
            }
            if action_row(ui, "Add a debug touch area", "A new area to place in the editor", "ADD") {
                let number = settings.debug_touch_areas.len() + 1;
                settings.debug_touch_areas.push(crate::settings::DebugTouchArea {
                    name: format!("Debug area {number}"),
                    ..Default::default()
                });
            }
            let mut remove = None;
            for (i, area) in settings.debug_touch_areas.iter_mut().enumerate() {
                let label = format!("Area {}", i + 1);
                text_field(ui, &label, TextKind::Name, &mut area.name, "End call", 48);
                info_row(
                    ui,
                    "Position",
                    "Left, top, right, bottom of the screen",
                    &format!(
                        "{:.0}%, {:.0}% to {:.0}%, {:.0}%",
                        area.rect[0] * 100.0,
                        area.rect[1] * 100.0,
                        area.rect[2] * 100.0,
                        area.rect[3] * 100.0
                    ),
                );
                pick_row(
                    ui,
                    "Tap does",
                    "A click at its centre, or a pad button",
                    &mut area.action,
                    &[(DebugTouchAction::Click, "Click"), (DebugTouchAction::PadButton, "Button")],
                );
                if area.action == DebugTouchAction::PadButton {
                    pick_row(
                        ui,
                        "Button",
                        "The pad button a tap pulses",
                        &mut area.button,
                        &[
                            (DebugTouchButton::Back, "Back"),
                            (DebugTouchButton::Confirm, "Confirm"),
                            (DebugTouchButton::Up, "Up"),
                            (DebugTouchButton::Down, "Down"),
                            (DebugTouchButton::Left, "Left"),
                            (DebugTouchButton::Right, "Right"),
                        ],
                    );
                }
                pick_row(
                    ui,
                    "Shows on",
                    "The game's screens this area belongs to",
                    &mut area.when,
                    &[
                        (DebugTouchWhen::Always, "Always"),
                        (DebugTouchWhen::Gameplay, "Gameplay"),
                        (DebugTouchWhen::Phone, "Phone"),
                        (DebugTouchWhen::Pointer, "Pointer"),
                    ],
                );
                if action_row(ui, "Delete this area", "It stops showing and doing", "DELETE") {
                    remove = Some(i);
                }
            }
            if let Some(i) = remove
                && i < settings.debug_touch_areas.len()
            {
                settings.debug_touch_areas.remove(i);
            }
            let version = concat!("Glue 5 ", env!("CARGO_PKG_VERSION"), " and the projects it builds on");
            if action_row(ui, "Credits", version, "VIEW") {
                *go = Some(Some(Page::Credits));
            }
            info_row(ui, "Version", "This build", crate::version::LABEL);
            if started.is_some() && !MOBILE && action_row(ui, "Stop the game", "Back to this menu", "QUIT") {
                actions.push(Action::Quit);
            }
        }
    }
}

fn gpu_driver(ui: &mut egui::Ui, settings: &mut Settings, drivers: &Drivers, before_game: bool, actions: &mut Vec<Action>) {
    let mut options = vec![(None, "Phone's")];
    options.extend(drivers.packages.iter().map(|p| (Some(p.id.as_str()), p.name.as_str())));
    let mut chosen = settings.gpu_driver.as_deref().filter(|id| options.iter().any(|(o, _)| *o == Some(id)));
    let shown = if chosen == drivers.in_use.as_deref() { drivers.running.as_str() } else { "Next launch" };
    let r = setting(ui, "GPU driver", Control::Choice, Some(shown), |ui| choice(ui, &mut chosen, &options))
        .on_hover_text("Phone's: the driver it came with. Others: Adreno Tools packages added here");
    if r.changed() {
        settings.gpu_driver = chosen.map(str::to_owned);
    }
    if before_game {
        let subtitle = if drivers.adding { "Adding…" } else { "Adreno Tools packages (.zip): Turnip, Qualcomm" };
        setting(ui, "GPU drivers", Control::Actions, Some(subtitle), |ui| {
            ui.add_enabled_ui(!drivers.adding, |ui| {
                let remove = settings.gpu_driver.clone();
                let labels: &[&str] = if remove.is_some() { &["Remove", "Add…"] } else { &["Add…"] };
                let mut buttons = action_buttons(ui, labels).into_iter();
                if let Some(id) = remove
                    && buttons.next().is_some_and(|b| b.on_hover_text("Delete the chosen driver").clicked())
                {
                    actions.push(Action::RemoveDriver(id));
                }
                if buttons.next().is_some_and(|b| b.clicked()) {
                    actions.push(Action::AddDriver);
                }
            });
        });
    }
}

fn next_start(differs: bool) -> Option<&'static str> {
    differs.then_some("Next start")
}

fn game_folder(ui: &mut egui::Ui, settings: &mut Settings, status: &Status, actions: &mut Vec<Action>) {
    let started = status.started;
    let dir = if started.is_some() { settings.game_dir.as_deref() } else { status.game_dir };
    let mut shown = dir.map_or_else(|| "Not found".to_owned(), |d| d.display().to_string());
    if started.is_some_and(|s| s.source == Source::Folder && Some(s.game_dir.as_path()) != settings.game_dir.as_deref()) {
        shown.push_str(" · next start");
    }
    setting(ui, "Game folder", Control::Action, Some(&shown), |ui| {
        if action_button(ui, if MOBILE { "Find files" } else { "Choose…" }).clicked() {
            actions.push(if MOBILE { Action::FindGame } else { Action::ChooseGame });
        }
    });
    if !MOBILE {
        let shown = match &status.sharing {
            // Turned on while the game runs: it starts with the start page.
            None if settings.share && status.started.is_some() => {
                "Phones, tablets and TVs play it from here · next start".to_owned()
            }
            None => "Phones, tablets and TVs play it from here".to_owned(),
            Some(Sharing::Preparing(_)) => "Preparing…".to_owned(),
            Some(Sharing::On(a, _)) => a.first().map_or_else(|| "No network address".to_owned(), |a| format!("On: {a}")),
            Some(Sharing::Failed(why)) => why.clone(),
        };
        setting(ui, "Share on the network", Control::Toggle, Some(&shown), |ui| toggle(ui, &mut settings.share));
    }
}

#[cfg(test)]
mod tests {
    use super::{Mode, SettingsTab, Status, settings_page, theme};
    use crate::settings::{Settings, Upscaling};
    use crate::ui::menu;
    use egui::{Id, Pos2, Rect, SafeAreaInsets, vec2};
    use std::path::Path;

    /// The shell is the safe area: it starts at the corner, is as wide, and
    /// ends there — egui rounds an area's rect, so a point of slack.
    fn covers(shell: Rect, safe: Rect) -> bool {
        (shell.min - safe.min).length() <= 1.0
            && (shell.width() - safe.width()).abs() <= 1.0
            && (shell.height() - safe.height()).abs() <= 2.0
    }

    /// Every text the frame drew, with the middle of its box — the shapes
    /// nest (`Shape::Vec`), so this walks them.
    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, Pos2)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect().center())),
                egui::Shape::Vec(list) => list.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        shapes.iter().for_each(|s| walk(&s.shape, &mut out));
        out
    }

    fn status<'a>() -> Status<'a> {
        Status {
            game_dir: Some(Path::new("game")),
            problem: None,
            started: None,
            last_failure: None,
            graphics_reset: false,
            window: (1440, 900),
            back_buffer: None,
            upscalers: &[Upscaling::Bicubic, Upscaling::MetalFx, Upscaling::MetalFxTemporal, Upscaling::Fsr],
            frame_generation: true,
            metal_hud: false,
            refresh_rate: 120.0,
            frame_cap: 60,
            thermal: None,
            preparing: None,
            mode: Mode::Story,
            benchmark: None,
            online: false,
            friend_requests: 0,
            source: crate::settings::Source::Folder,
            stream: None,
            packages: None,
            sharing: None,
            mods: false,
            mod_menus: false,
            studio: None,
            servers: &[],
            searching: false,
            stream_alert: None,
            drivers: None,
        }
    }

    /// The glyphs the shell and the lists paint: the tabs' arrows, the icons
    /// of the settings' rows.
    #[test]
    fn the_shells_icons_are_in_the_font() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        ctx.begin_pass(Default::default());
        ctx.end_pass().textures_delta.clear();
        assert!(ctx.fonts_mut(|f| f.has_glyphs(&egui::FontId::proportional(14.0), "⚙🗁🖧🗙★♡🛠›")));
        // Every family the shell names has a font behind it: a chain whose
        // first name has no data panics on its first lookup, in the app too.
        for family in [theme::fonts::strong(), theme::fonts::heavy(), egui::FontFamily::Name("condensed".into())] {
            let found = ctx.fonts_mut(|f| f.has_glyphs(&egui::FontId::new(14.0, family.clone()), "GLUE 5"));
            assert!(found, "{family} is named by the shell but has no font behind it");
        }
    }

    #[test]
    fn settings_fill_the_safe_area_at_all_sizes() {
        for size in
            [vec2(1440.0, 900.0), vec2(768.0, 1024.0), vec2(390.0, 844.0), vec2(320.0, 480.0), vec2(844.0, 390.0)]
        {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            for tab in [
                SettingsTab::Graphics,
                SettingsTab::Display,
                SettingsTab::Audio,
                SettingsTab::Controls,
                SettingsTab::Gameplay,
                SettingsTab::Files,
                SettingsTab::Network,
                SettingsTab::Advanced,
                SettingsTab::Diagnostics,
            ] {
                let mut settings = Settings::default();
                let mut tab = tab;
                for _ in 0..4 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(screen),
                            safe_area_insets: Some(SafeAreaInsets(egui::epaint::MarginF32 {
                                top: 24.0,
                                bottom: 16.0,
                                left: 8.0,
                                right: 8.0,
                            })),
                            ..Default::default()
                        },
                        |root| {
                            settings_page(
                                root.ctx(),
                                &mut settings,
                                &status(),
                                None,
                                &mut tab,
                                &mut None,
                                &mut Vec::new(),
                                &mut None,
                            );
                        },
                    );
                    output.textures_delta.clear();
                }
                // Full screen: the shell is the safe area, not a window in it.
                let shell = ctx.memory(|m| m.area_rect(Id::new(("shell", "SETTINGS")))).unwrap();
                let safe = ctx.content_rect();
                assert!(covers(shell, safe), "{size:?} {tab:?}: the shell {shell:?} is not the safe area {safe:?}");
            }
        }
    }

    #[test]
    fn a_phone_reaches_the_last_graphics_row_by_scrolling() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut settings = Settings::default();
        let mut tab = SettingsTab::Graphics;
        let mut seen = false;
        for pass in 0..8 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(874.0, 402.0))),
                    time: Some(pass as f64),
                    events: if pass == 4 {
                        vec![
                            egui::Event::PointerMoved(Pos2::new(300.0, 300.0)),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: vec2(0.0, -600.0),
                                phase: egui::TouchPhase::Move,
                                modifiers: egui::Modifiers::default(),
                            },
                        ]
                    } else {
                        Vec::new()
                    },
                    safe_area_insets: Some(SafeAreaInsets(egui::epaint::MarginF32 {
                        left: 62.0,
                        right: 62.0,
                        top: 0.0,
                        bottom: 21.0,
                    })),
                    ..Default::default()
                },
                |root| {
                    settings_page(
                        root.ctx(),
                        &mut settings,
                        &status(),
                        None,
                        &mut tab,
                        &mut None,
                        &mut Vec::new(),
                        &mut None,
                    );
                },
            );
            output.textures_delta.clear();
            if pass == 7 {
                // Every row that is drawn sits inside what is on screen.
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.text() == "Restore graphics"
                    {
                        seen = shape.clip_rect.contains_rect(text.visual_bounding_rect());
                    }
                }
            }
        }
        assert!(seen, "the last graphics row is not reachable by scrolling");
    }

    /// The PLAY tab: the shell's tab strip, and the four ways into the game as
    /// rows of the list.
    #[test]
    fn the_play_tab_lists_the_four_ways_into_the_game() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut settings = Settings::default();
        let mut status = status();
        status.game_dir = None;
        let mut selected = Mode::Story;
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
        let mut drawn = Vec::new();
        let mut shapes = 0;
        // The shell needs a frame to place itself; the fourth is the one read.
        for _ in 0..4 {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |root| {
                menu::show(root.ctx(), &mut settings, &status, &mut selected, false, None, &mut actions, &mut None);
            });
            assert!(actions.is_empty(), "drawing the tab acted on its own");
            shapes = output.shapes.len();
            drawn = texts(&output.shapes).into_iter().map(|(text, _)| text).collect();
            output.textures_delta.clear();
        }
        // The tab strip, and the four ways into the game.
        for wanted in ["PLAY", "SETTINGS", "FRIENDS", "SERVERS", "MODS", "CREDITS"] {
            assert!(drawn.iter().any(|t| t == wanted), "{wanted} is not in the tab strip ({shapes} shapes): {drawn:?}");
        }
        for wanted in ["Story mode", "GTA Online", "Sandbox", "Benchmark"] {
            assert!(drawn.iter().any(|t| t == wanted), "{wanted} is not in the list ({shapes} shapes): {drawn:?}");
        }
        // The chosen one is repeated as the single action of the list.
        assert!(drawn.iter().any(|t| t == "PLAY STORY MODE"), "no PLAY row: {drawn:?}");
    }

    #[test]
    fn phone_landscape_shows_all_four_modes_without_clipping() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut settings = Settings::default();
        let mut selected = Mode::Story;
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(844.0, 390.0));
        let mut visible = Vec::new();
        for _ in 0..4 {
            let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |root| {
                crate::ui::shell::begin(Some(crate::ui::Page::Start));
                menu::show(root.ctx(), &mut settings, &status(), &mut selected, false, None, &mut Vec::new(), &mut None);
            });
            visible = output.shapes.iter().filter_map(|clipped| {
                if let egui::Shape::Text(text) = &clipped.shape
                    && clipped.clip_rect.contains_rect(text.visual_bounding_rect())
                {
                    Some(text.galley.text().to_owned())
                } else {
                    None
                }
            }).collect::<Vec<_>>();
            output.textures_delta.clear();
        }
        for name in ["Story mode", "GTA Online", "Sandbox", "Benchmark", "PLAY STORY MODE"] {
            assert!(visible.iter().any(|text| text == name), "{name} is clipped or missing: {visible:?}");
        }
    }

    #[test]
    fn tapping_a_mode_label_selects_it_and_updates_the_play_action() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut settings = Settings::default();
        let mut selected = Mode::Story;
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(844.0, 390.0));
        let mut target = None;
        let mut drawn = Vec::new();
        for pass in 0..7 {
            let events = match (pass, target) {
                (4, Some(pos)) => vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                    pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default(),
                }],
                (5, Some(pos)) => vec![egui::Event::PointerButton {
                    pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default(),
                }],
                _ => Vec::new(),
            };
            let mut output = ctx.run_ui(egui::RawInput {
                screen_rect: Some(screen), events, time: Some(pass as f64 * 0.1), ..Default::default()
            }, |root| {
                crate::ui::shell::begin(Some(crate::ui::Page::Start));
                menu::show(root.ctx(), &mut settings, &status(), &mut selected, false, None, &mut Vec::new(), &mut None);
            });
            drawn = texts(&output.shapes);
            if pass == 3 {
                target = drawn.iter().find(|(text, _)| text == "Sandbox").map(|(_, at)| *at);
            }
            output.textures_delta.clear();
        }
        assert!(target.is_some(), "the Sandbox row must be visible");
        assert_eq!(selected, Mode::Sandbox);
        assert!(settings.sandbox);
        assert!(drawn.iter().any(|(text, _)| text == "PLAY SANDBOX"));
    }

    #[test]
    fn credits_fit_the_safe_area() {
        for size in [vec2(1440.0, 900.0), vec2(390.0, 844.0), vec2(844.0, 390.0), vec2(320.0, 480.0)] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            for _ in 0..4 {
                let raw =
                    egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
                ctx.run_ui(raw, |root| {
                    assert!(!crate::ui::credits_page(root.ctx()));
                })
                .textures_delta
                .clear();
            }
            let window = ctx.memory(|m| m.area_rect(Id::new(("shell", "CREDITS")))).unwrap();
            assert!(covers(window, ctx.content_rect()), "{size:?}: {window:?}");
        }
    }

    #[test]
    fn friends_fit_the_safe_area() {
        for size in [vec2(1440.0, 900.0), vec2(390.0, 844.0), vec2(844.0, 390.0), vec2(320.0, 480.0)] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let mut page = crate::ui::FriendsPage::default();
            for _ in 0..4 {
                let raw =
                    egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
                ctx.run_ui(raw, |root| {
                    let game = crate::ui::friends::Context { started: false, online: false };
                    assert!(!crate::ui::friends_page(root.ctx(), &mut page, game, &mut Vec::new()));
                })
                .textures_delta
                .clear();
            }
            let window = ctx.memory(|m| m.area_rect(Id::new(("shell", "FRIENDS")))).unwrap();
            assert!(covers(window, ctx.content_rect()), "{size:?}: {window:?}");
        }
    }

    #[test]
    fn the_play_tab_reflows_when_the_same_window_is_resized() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        for size in
            [vec2(1440.0, 900.0), vec2(390.0, 844.0), vec2(320.0, 480.0), vec2(900.0, 500.0), vec2(1440.0, 900.0)]
        {
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            let mut settings = Settings::default();
            let mut selected = Mode::Story;
            for _ in 0..4 {
                let mut output =
                    ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |root| {
                        menu::show(
                            root.ctx(),
                            &mut settings,
                            &status(),
                            &mut selected,
                            false,
                            None,
                            &mut Vec::new(),
                            &mut None,
                        );
                    });
                output.textures_delta.clear();
            }
            // The shell fills the safe area, and nothing is drawn outside it.
            let shell = ctx.memory(|m| m.area_rect(Id::new(("shell", "PLAY")))).unwrap();
            let safe = ctx.content_rect();
            assert!(covers(shell, safe), "{size:?}: {shell:?} does not fill {safe:?}");
        }
    }

    /// The Files settings and the Servers page in each state of the stream
    /// (and of sharing) lay out within the safe area.
    #[test]
    fn streaming_states_fit() {
        use crate::settings::Source;
        use glue5_stream::Phase;
        let base = glue5_stream::Status {
            phase: Phase::Ready,
            connected: true,
            error: None,
            cache_bytes: 3_700_000_000,
            boot: (300 << 20, 550 << 20),
            waiting: 0,
            total_bytes: 21_000_000_000,
            filling: false,
            complete: false,
            no_room: false,
        };
        let states = [
            glue5_stream::Status { phase: Phase::Connecting, error: Some("connection refused".into()), ..base.clone() },
            glue5_stream::Status { phase: Phase::Syncing { what: "text", done: 3 << 20, total: 9 << 20 }, ..base.clone() },
            base.clone(),
            glue5_stream::Status { filling: true, no_room: true, ..base.clone() },
            glue5_stream::Status { complete: true, connected: false, ..base.clone() },
            glue5_stream::Status { phase: Phase::Failed("the server has game build x".into()), ..base.clone() },
        ];
        use glue5_stream::Check;
        let found = |name: &str, check| (glue5_stream::discovery::Found { name: name.into(), address: format!("{name}.local:47100") }, check);
        let found = [
            found("Mac", Some(Check::Ready { ms: 3, at: vec!["192.168.1.20:47100".parse().unwrap()] })),
            found("A computer with a very long name that does not fit", Some(Check::Unusable("the server has game build x, this app runs y".into()))),
            found("Linux", Some(Check::Unreachable("connection refused".into()))),
            found("Asking", None),
        ];
        for size in [vec2(1440.0, 900.0), vec2(390.0, 844.0), vec2(844.0, 390.0)] {
            for (i, stream) in states.iter().enumerate() {
                let ctx = egui::Context::default();
                theme::install(&ctx);
                let screen = Rect::from_min_size(Pos2::ZERO, size);
                let mut settings = Settings::default();
                (settings.source, settings.server) = (Source::Server, "192.168.1.20".into());
                let mut status = status();
                status.source = Source::Server;
                status.stream = Some(("192.168.1.20", stream));
                status.servers = &found;
                status.sharing = Some(crate::sharing::State::On(vec!["10.0.0.2".into()], true));
                let mut tab = SettingsTab::Files;
                let mut servers = crate::ui::ServersPage::default();
                for page in 0..12 {
                    let mut output =
                        ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |root| {
                            let (actions, go) = (&mut Vec::new(), &mut None);
                            if page < 6 {
                                settings_page(root.ctx(), &mut settings, &status, None, &mut tab, &mut None, actions, go);
                            } else {
                                crate::ui::servers_page(root.ctx(), &mut servers, &mut settings, &status, actions);
                            }
                        });
                    output.textures_delta.clear();
                }
                for title in ["SETTINGS", "SERVERS"] {
                    let window = ctx.memory(|m| m.area_rect(Id::new(("shell", title)))).unwrap();
                    assert!(covers(window, ctx.content_rect()), "{size:?}, state {i}, {title}: {window:?}");
                }
            }
        }
    }

    /// A server found by name is known under the addresses its name stands for.
    #[test]
    fn a_server_found_is_known_by_its_address_too() {
        use glue5_stream::Check;
        let found = glue5_stream::discovery::Found { name: "Mac".into(), address: "mac.local:47100".into() };
        let at = vec!["10.0.0.2:47100".parse().unwrap(), "[fe80::1]:47100".parse().unwrap()];
        let server = (found, Some(Check::Ready { ms: 2, at }));
        for (address, same) in
            [("mac.local:47100", true), ("10.0.0.2:47100", true), ("10.0.0.2", true), ("10.0.0.2:47101", false), ("10.0.0.3", false)]
        {
            assert_eq!(super::is_server(&server, address), same, "{address}");
        }
        assert!(!super::is_server(&(server.0.clone(), None), "10.0.0.2"));
    }

    /// The PLAY tab's "Game files" row asks for the folder picker (on a phone
    /// its own picker); the servers page, opened, searches the network.
    #[test]
    fn the_play_tabs_files_row_asks_for_the_game() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut settings = Settings::default();
        let mut status = status();
        status.game_dir = None;
        let mut selected = Mode::Story;
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
        let mut at = None;
        let mut actions = Vec::new();
        for pass in 0..4 {
            let events = match (pass, at) {
                (3, Some(pos)) => vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
                ],
                _ => Vec::new(),
            };
            let mut output =
                ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |root| {
                    menu::show(root.ctx(), &mut settings, &status, &mut selected, false, None, &mut actions, &mut None);
                });
            at = at.or_else(|| {
                texts(&output.shapes).into_iter().find_map(|(text, at)| (text == "FIND FILES").then_some(at))
            });
            output.textures_delta.clear();
        }
        assert!(at.is_some(), "the files row is not drawn");
        assert!(
            actions.iter().any(|a| matches!(a, crate::ui::Action::ChooseGame | crate::ui::Action::FindGame)),
            "the files row did not ask for the game: {} actions",
            actions.len()
        );

        let mut page = crate::ui::ServersPage::default();
        page.opened();
        let mut search = Vec::new();
        ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |root| {
            crate::ui::servers_page(root.ctx(), &mut page, &mut settings, &status, &mut search);
        })
        .textures_delta
        .clear();
        assert!(matches!(search[..], [crate::ui::Action::FindServers]));
    }
}
