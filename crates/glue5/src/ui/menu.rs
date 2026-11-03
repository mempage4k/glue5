//! The start page: the shell's PLAY tab. The four ways into the game as rows,
//! the chosen one repeated as the one action of the list, and what the app
//! knows about it below (the game's files, the source, the last run) — the
//! pause menu's list, in place of cards and a bottom rail.
use egui::Response;

use super::theme::{density, fonts, palette};
use super::widgets::{action_value, choice, info_text, row, value_text};
use super::{Action, Mode, Page, Status, WARN, shell};
use crate::settings::{Settings, Source};
use crate::sfx::{self, Sound};

/// The four modes: their name, their line in the list, and the description
/// panel's help.
const MODES: [(Mode, &str, &str); 4] = [
    (Mode::Story, "Story mode", "The story of Los Santos: three lives, one city"),
    (Mode::Online, "GTA Online", "Freemode with friends and rivals: your own character"),
    (Mode::Sandbox, "Sandbox", "The whole map, no missions, nothing to prove"),
    (Mode::Benchmark, "Benchmark", "A guided flight over the city, then the frame times"),
];

pub(super) fn show(
    ctx: &egui::Context,
    settings: &mut Settings,
    status: &Status,
    selected: &mut Mode,
    _gamepad_prompts: bool,
    starting: Option<Mode>,
    actions: &mut Vec<Action>,
    go: &mut Option<Option<Page>>,
) {
    show_with_spinner(ctx, settings, status, selected, None, _gamepad_prompts, starting, actions, go);
}

pub(super) fn show_with_spinner(
    ctx: &egui::Context,
    settings: &mut Settings,
    status: &Status,
    selected: &mut Mode,
    spinner: Option<&egui::TextureHandle>,
    _gamepad_prompts: bool,
    starting: Option<Mode>,
    actions: &mut Vec<Action>,
    go: &mut Option<Option<Page>>,
) {
    // The start page keeps what Play starts, as it always has.
    settings.online = *selected == Mode::Online;
    settings.sandbox = *selected == Mode::Sandbox;
    let prepared = status.game_dir.is_some() && status.preparing.is_none();
    let going = starting.or(status.preparing.map(|_| status.mode));
    let loading_label = going.map(|mode| {
        let which = MODES.iter().find(|m| m.0 == mode).map_or("", |m| m.1);
        status
            .preparing
            .map_or_else(|| format!("STARTING {}", which.to_uppercase()), |p| format!("PREPARING {:.0}%", p * 100.0))
    });

    shell::frame(ctx, "PLAY", |ui| {
        let scale = density(ui);
        for (mode, name, help) in MODES {
            let here = *selected == mode;
            let response: Response = row(ui, name, help, |ui| {
                if here {
                    action_value(ui, "SELECTED")
                } else {
                    value_text(ui, "›")
                }
            });
            if response.clicked() && !here {
                *selected = mode;
                settings.online = mode == Mode::Online;
                settings.sandbox = mode == Mode::Sandbox;
                sfx::want(Sound::Focus);
            }
        }

        // Describe the actual chosen mode, not an unselected row under the
        // pointer. Other controls below can still provide their own help.
        if let Some((_, name, help)) = MODES.iter().find(|(mode, _, _)| *mode == *selected) {
            super::widgets::describe(ui.ctx(), name, help);
        }

        // GTA Online's own line: the name other players see.
        if *selected == Mode::Online {
            row(ui, "Player name", "The name other players see, made up on the first visit", |ui| {
                super::widgets::text_field(ui, "", super::TextKind::Name, &mut settings.online_name, "Player name", 24)
            });
        }

        ui.add_space(10.0 * scale);
        // The one action of the list: what A does, whatever line is lit.
        let label = if let Some(mode) = going {
            let which = MODES.iter().find(|m| m.0 == mode).map_or("", |m| m.1);
            status.preparing.map_or_else(|| format!("STARTING {}", which.to_uppercase()), |p| format!("PREPARING  {:.0}%", p * 100.0))
        } else {
            format!("PLAY {}", MODES.iter().find(|m| m.0 == *selected).map_or("", |m| m.1).to_uppercase())
        };
        let loading = going.is_some();
        let play = row(ui, &label, "Starts the game with the line selected above", |ui| {
            if prepared && !loading { action_value(ui, "A") } else { value_text(ui, if loading { "" } else { "—" }) }
        });
        if play.clicked() {
            if prepared && going.is_none() {
                // The one loud sound: the game is about to start.
                sfx::want(Sound::Launch);
                actions.push(Action::Play(*selected));
            } else {
                sfx::want(Sound::Error);
            }
        }
        if status.preparing.is_none() && status.game_dir.is_none() {
            ui.add_space(4.0 * scale);
            ui.label(egui::RichText::new("Choose a game folder below, or connect to a server").size(11.5 * scale).color(WARN));
        }

        ui.add_space(10.0 * scale);
        // What the app knows: the game's files, where they come from, how the
        // last run went. Two rows at most in normal use.
        let files = status.game_dir.map_or_else(
            || match status.source {
                Source::Server => "Not here: from a server".to_owned(),
                Source::Folder => "Not found".to_owned(),
            },
            |dir| dir.file_name().map_or_else(|| "Found".to_owned(), |name| name.to_string_lossy().into_owned()),
        );
        let folder_help = status.problem.unwrap_or("The game's folder, on this device");
        // The folder's own name, when there is one: it is what the player
        // recognizes, and the path itself stays out of the menu.
        let action = if status.game_dir.is_some() { format!("{files} ›") } else { "FIND FILES".to_owned() };
        if row(ui, "Game files", folder_help, |ui| action_value(ui, &action)).clicked() {
            actions.push(if crate::paths::MOBILE { Action::FindGame } else { Action::ChooseGame });
        }
        if crate::paths::TV {
            info_text(ui, "Source", "This device always streams the game", "SERVER");
        } else {
            let before = settings.source;
            row(ui, "Source", "Folder: the game on this device. Server: from a computer that shares it", |ui| {
                choice(ui, &mut settings.source, &[(Source::Folder, "Folder"), (Source::Server, "Server")])
            });
            if settings.source != before && settings.source == Source::Server {
                *go = Some(Some(Page::Servers));
            }
        }
        if let Some((address, state)) = status.stream {
            let (name, _) = super::pages::stream_state(state);
            if row(ui, "Server", &format!("{name}: {address}"), |ui| action_value(ui, "OPEN")).clicked() {
                *go = Some(Some(Page::Servers));
            }
        }
        if let Some(failure) = status.last_failure {
            info_text(ui, "Last session", failure, "STOPPED");
        }
        if let Some(result) = status.benchmark
            && *selected == Mode::Benchmark
        {
            info_text(
                ui,
                "Last run",
                &format!("{:.0} average, {:.0} lowest, {:.0} % under 33 ms", result.avg_fps, result.min_fps, result.smooth * 100.0),
                "BENCHMARK",
            );
        }
    });
    if let Some(label) = loading_label.as_deref() {
        loading_prompt(ctx, spinner, label);
    }
}

/// Where the loading prompt sits: thin against the safe area's bottom right,
/// on the scene beside the shell's panel (the menu's controls are in the
/// panel, on the left).
fn loading_prompt_rect(safe: egui::Rect, size: egui::Vec2, scale: f32) -> egui::Rect {
    let margin = 16.0 * scale;
    let min = egui::pos2(
        (safe.right() - size.x - margin).max(safe.left()),
        (safe.bottom() - size.y - margin).max(safe.top()),
    );
    egui::Rect::from_min_size(min, size)
}

/// The game's loading prompt while Play prepares or starts: one thin black
/// strap at the bottom right, its words at the left and the turning spinner
/// at its right. It answers to nothing.
fn loading_prompt(ctx: &egui::Context, spinner: Option<&egui::TextureHandle>, text: &str) {
    let safe = ctx.content_rect();
    if safe.width() <= 0.0 || safe.height() <= 0.0 {
        return;
    }
    let scale = super::theme::layout_scale(safe);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("loading-prompt")));
    let pad_x = 12.0 * scale;
    let spinner_size = spinner.map_or(0.0, |_| 17.0 * scale);
    let spinner_gap = if spinner.is_some() { 10.0 * scale } else { 0.0 };
    let max_width = (320.0 * scale).min((safe.width() - 32.0 * scale).max(1.0));
    let text_width = (max_width - pad_x * 2.0 - spinner_size - spinner_gap).max(1.0);
    let galley = painter.layout(
        text.to_owned(),
        egui::FontId::new(12.0 * scale, fonts::strong()),
        palette::TEXT_HI,
        text_width,
    );
    let size = egui::vec2(
        (galley.size().x + pad_x * 2.0 + spinner_size + spinner_gap).min(max_width),
        (galley.size().y + 16.0 * scale).max(32.0 * scale),
    );
    let rect = loading_prompt_rect(safe, size, scale);
    painter.rect_filled(rect, 0, palette::BLOCK);
    painter.rect_stroke(rect, 0, egui::Stroke::new(1.0, palette::LINE), egui::StrokeKind::Inside);
    let text_h = galley.size().y;
    painter.galley(egui::pos2(rect.left() + pad_x, rect.center().y - text_h * 0.5), galley, palette::TEXT_HI);
    if let Some(spinner) = spinner {
        let spinner_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - pad_x - spinner_size * 0.5, rect.center().y),
            egui::Vec2::splat(spinner_size),
        );
        let time = ctx.input(|i| i.time) as f32;
        super::spinner::paint(&painter, spinner, spinner_rect, time * 5.2);
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_prompt_hugs_the_bottom_right_of_the_safe_area() {
        let safe = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(410.0, 220.0));
        let rect = loading_prompt_rect(safe, egui::vec2(180.0, 32.0), 1.0);
        assert_eq!(rect.left(), 214.0);
        assert_eq!(rect.top(), 172.0);
        assert_eq!(rect.right(), 394.0);
        assert_eq!(rect.bottom(), 204.0);
    }
}
