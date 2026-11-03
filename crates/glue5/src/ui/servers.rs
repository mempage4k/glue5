//! Servers: where the game's files come from when they stream. The server
//! in use and how it is doing; the computers sharing the game on this
//! network, each asked whether it serves this game; one by its address;
//! what this device keeps of the game.
use std::time::{Duration, Instant};

use egui::{Align, Layout, RichText, Sense, Stroke, pos2, vec2};
use glue5_stream::{Check, GamePackage, PackageManifest, TransferPhase};

use super::pages::{bytes, card_title, is_server, rail, server_name, sheet, small, stream_progress, stream_state};
use super::widgets::{Control, action_button, action_buttons, focus, setting, text_field, toggle};
use super::{Action, PackageTransfer, Packages, Status, TextKind, WARN, theme};
use crate::paths;
use crate::settings::{Settings, Source};

/// While the page is open the network is searched this often: computers
/// that start or stop sharing come and go by themselves.
const SEARCH_EVERY: Duration = Duration::from_secs(5);

/// What the page keeps between frames.
#[derive(Default)]
pub struct ServersPage {
    /// An address being typed.
    address: String,
    /// When this page last started a search; none: just opened.
    searched: Option<Instant>,
}

impl ServersPage {
    pub(super) fn opened(&mut self) {
        self.searched = None;
    }
}

/// True when closed.
pub(super) fn servers_page(
    ctx: &egui::Context,
    page: &mut ServersPage,
    settings: &mut Settings,
    status: &Status,
    actions: &mut Vec<Action>,
) -> bool {
    if !status.searching && page.searched.is_none_or(|t| t.elapsed() >= SEARCH_EVERY) {
        actions.push(Action::FindServers);
        page.searched = Some(Instant::now());
    }
    sheet(ctx, "servers-window", "SERVERS", |ui| {
        in_use(ui, settings, status);
        optional_content(ui, settings, status, actions);
        network(ui, settings, status, actions);
        by_address(ui, page, actions);
        this_device(ui, settings, status);
        ui.add_space(4.0 * theme::density(ui));
        muted(
            ui,
            "The game's files come from the server as the game reads them, and stay on this device: later starts \
             ask it only for what is not here yet.",
        );
    })
}

/// The server in use: its name, address and state, what it is doing.
fn in_use(ui: &mut egui::Ui, settings: &Settings, status: &Status) {
    let server = settings.server.trim();
    panel(ui, |ui| {
        if server.is_empty() {
            card_title(ui, "No server yet", Some(("Not chosen", WARN)));
            small(ui, "Choose a computer on this network, or type its address.", theme::MUTED);
            return;
        }
        let stream = status.stream.filter(|(s, _)| *s == server).map(|(_, s)| s);
        let name = server_name(status, server);
        card_title(ui, &name, stream.map(stream_state));
        if name != server {
            small(ui, server, theme::MUTED);
        }
        match stream {
            _ if settings.source != Source::Server => {
                small(ui, "The game's files come from this device's folder (Settings › Game › Game files).", WARN)
            }
            Some(s) => {
                if s.cache_bytes > 0 {
                    small(ui, &format!("{} on this device", bytes(s.cache_bytes)), theme::MUTED);
                }
                stream_progress(ui, s);
            }
            // The game runs from another server, or from the folder.
            None if status.started.is_some() => small(ui, "From the next start: the game runs on.", WARN),
            None => small(ui, "Connecting…", theme::MUTED),
        }
    });
}

/// The computers sharing the game on this network, searched all the while.
fn network(ui: &mut egui::Ui, settings: &Settings, status: &Status, actions: &mut Vec<Action>) {
    section(ui, "ON THIS NETWORK", scanning);
    let current = settings.server.trim();
    for server in status.servers {
        let (found, check) = server;
        let using = settings.source == Source::Server && !current.is_empty() && is_server(server, current);
        let (what, usable) = match check {
            None => (format!("{} · asking…", found.address), true),
            Some(Check::Ready { ms: 0, .. }) => (format!("{} · under 1 ms", found.address), true),
            Some(Check::Ready { ms, .. }) => (format!("{} · {ms} ms", found.address), true),
            // Another game build, another version of the app.
            Some(Check::Unusable(why)) => (why.clone(), false),
            Some(Check::Unreachable(_)) => (format!("{} · not answering", found.address), true),
        };
        ui.push_id(&found.address, |ui| {
            setting(ui, &found.name, Control::Action, Some(&what), |ui| {
                if using {
                    ui.add_enabled_ui(false, |ui| action_button(ui, "In use"));
                } else if ui.add_enabled_ui(usable, |ui| action_button(ui, "Use")).inner.clicked() {
                    actions.push(Action::UseServer(found.address.clone()));
                }
            });
        });
    }
    if status.servers.is_empty() {
        muted(
            ui,
            "Looking for computers sharing the game. On the one with the game: Glue 5 › Settings › Game › Share on \
             the network, or glue5-serve; both on the same network.",
        );
        if cfg!(target_os = "ios") {
            muted(ui, "None shows up: allow Glue 5 in Settings › Privacy & Security › Local Network.");
        }
    }
}

/// An arc turning: the network is searched. Ten frames a second, not every
/// frame (the page draws only when something moves).
fn scanning(ui: &mut egui::Ui) {
    let scale = theme::density(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0) * scale, Sense::hover());
    let turn = (ui.input(|i| i.time) * 10.0).floor() as f32 / 10.0 * std::f32::consts::TAU * 0.8;
    let (c, r) = (rect.center(), 7.0 * scale);
    let points = (0..=16).map(|i| {
        let a = turn + i as f32 / 16.0 * std::f32::consts::TAU * 0.7;
        pos2(c.x + r * a.cos(), c.y + r * a.sin())
    });
    ui.painter().add(egui::Shape::line(points.collect(), Stroke::new(2.0 * scale, theme::MUTED)));
    ui.ctx().request_repaint_after(Duration::from_millis(100));
}

/// A server by its address: typed, then Connect (or Enter).
fn by_address(ui: &mut egui::Ui, page: &mut ServersPage, actions: &mut Vec<Action>) {
    let scale = theme::density(ui);
    section(ui, "BY ADDRESS", |_| {});
    ui.horizontal(|ui| {
        let button = 104.0 * scale;
        let field = ui.available_width() - button - ui.spacing().item_spacing.x;
        let typed = ui
            .allocate_ui(vec2(field, 44.0 * scale), |ui| {
                text_field(ui, "", TextKind::Address, &mut page.address, "192.168.1.20 or name.local", 120)
            })
            .inner;
        let enter = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ready = !page.address.trim().is_empty();
        let connect = egui::Button::new(RichText::new("Connect").size(14.0 * scale))
            .corner_radius(14)
            .min_size(vec2(button, 44.0 * scale));
        let clicked = focus(ui.add_enabled(ready, connect)).clicked();
        if ready && (clicked || enter) {
            actions.push(Action::UseServer(page.address.trim().to_owned()));
            page.address.clear();
        }
    });
    muted(ui, "For a server the search does not reach (another network, a VPN); port 47100 unless another is given.");
}

/// What this device keeps: the whole game or what was read, and deleting it.
fn this_device(ui: &mut egui::Ui, settings: &mut Settings, status: &Status) {
    section(ui, "ON THIS DEVICE", |_| {});
    let stream = status.stream.map(|(_, s)| s);
    if paths::can_download_all() {
        let mut shown = match stream {
            Some(s) if s.complete => "All here: plays without the server".to_owned(),
            Some(s) if s.no_room => format!("Waiting for space: {} of {}", bytes(s.cache_bytes), bytes(s.total_bytes)),
            Some(s) if s.filling => format!("{} of {}", bytes(s.cache_bytes), bytes(s.total_bytes)),
            Some(s) if s.total_bytes > 0 => format!("{}, to play without the server", bytes(s.total_bytes)),
            _ => "To play without the server".to_owned(),
        };
        // The stream follows the setting when the game no longer runs on it.
        if status.started.is_some() && stream.is_some_and(|s| (s.filling || s.complete) != settings.download_all) {
            shown.push_str(" · next start");
        }
        setting(ui, "Download the whole game", Control::Toggle, Some(&shown), |ui| toggle(ui, &mut settings.download_all))
            .on_hover_text("At the lowest priority: the game's own reads go first");
    }
    let copy = stream.map_or(0, |s| s.cache_bytes);
    let what = match copy {
        0 => "Nothing yet".to_owned(),
        n if stream.is_some_and(|s| s.complete) => format!("{}: the whole game", bytes(n)),
        n => format!("{}, kept for the next starts", bytes(n)),
    };
    setting(ui, "Copy on this device", Control::Action, Some(&what), |ui| {
        ui.add_enabled_ui(status.started.is_none() && copy > 0, |ui| {
            if action_button(ui, "Delete").clicked() {
                super::popup::ask(super::popup::Request::clear_copy());
            }
        })
        .response
    })
    .on_hover_text("What came from the server; it is taken again when needed");
}

/// Optional content the server in use offers: only what its validated
/// manifest lists, what of it is here, and the one transfer's way. This is
/// separate from downloading the whole game; a server with no manifest is
/// ordinary and the base stream carries on without this section's help.
fn optional_content(ui: &mut egui::Ui, settings: &Settings, status: &Status, actions: &mut Vec<Action>) {
    let server = settings.server.trim();
    let Some(packages) = status.packages else { return };
    if server.is_empty() || settings.source != Source::Server {
        return;
    }
    // Only for the server the stream is really from.
    if status.stream.filter(|(s, _)| *s == server).is_none() {
        return;
    }
    section(ui, "OPTIONAL CONTENT", |_| {});
    let Some(manifest) = &packages.manifest else {
        muted(ui, "This server offers no optional content.");
        return;
    };
    let offered = offered_packages(manifest);
    if offered.is_empty() {
        muted(ui, "This server offers no optional content.");
        return;
    }
    if packages.transfer.as_ref().is_some_and(|transfer| transfer.active) {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
    for package in offered {
        package_row(ui, package, packages, actions);
    }
}

/// What the manifest offers as a choice: its optional packages. What one of
/// them needs still comes with it, without becoming a choice of its own.
fn offered_packages(manifest: &PackageManifest) -> Vec<&GamePackage> {
    manifest.packages.iter().filter(|package| package.optional).collect()
}

/// How one package stands, for its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackageState {
    Installed,
    Unavailable,
    /// It can be asked for (another transfer may still block the asking).
    Offered,
    /// The transfer running is this package's.
    Transferring,
    /// This package's last transfer failed (kept with its error).
    Failed,
}

fn package_state(package: &GamePackage, packages: &Packages) -> PackageState {
    if packages.installed.iter().any(|id| id == &package.id) {
        return PackageState::Installed;
    }
    if let Some(transfer) = &packages.transfer
        && transfer.package_id == package.id
    {
        return if transfer.active { PackageState::Transferring } else { PackageState::Failed };
    }
    let unavailable = !package.available
        || packages
            .manifest
            .as_ref()
            .is_some_and(|manifest| manifest.install_order(&package.id).is_err());
    if unavailable {
        return PackageState::Unavailable;
    }
    PackageState::Offered
}

/// A package's second line: what the manifest says it is, and its size.
fn package_subtitle(package: &GamePackage) -> String {
    let mut parts = Vec::new();
    if !package.description.trim().is_empty() {
        parts.push(package.description.trim().to_owned());
    }
    parts.push(bytes(package.total_size));
    if !package.dependencies.is_empty() {
        parts.push("what it needs comes with it".to_owned());
    }
    parts.join(" · ")
}

fn package_row(ui: &mut egui::Ui, package: &GamePackage, packages: &Packages, actions: &mut Vec<Action>) {
    let state = package_state(package, packages);
    let subtitle = package_subtitle(package);
    ui.push_id(&package.id, |ui| {
        match state {
            PackageState::Installed => {
                setting(ui, &package.name, Control::Action, Some(&subtitle), |ui| {
                    ui.add_enabled_ui(false, |ui| action_button(ui, "Installed"));
                });
            }
            PackageState::Unavailable => {
                setting(ui, &package.name, Control::Action, Some(&subtitle), |ui| {
                    ui.add_enabled_ui(false, |ui| action_button(ui, "Unavailable"));
                });
            }
            PackageState::Offered | PackageState::Failed => {
                let busy = packages.transfer.as_ref().is_some_and(|transfer| transfer.active);
                setting(ui, &package.name, Control::Action, Some(&subtitle), |ui| {
                    if ui.add_enabled_ui(!busy, |ui| action_button(ui, "Download")).inner.clicked() {
                        actions.push(Action::DownloadPackage(package.id.clone()));
                    }
                });
                if state == PackageState::Failed
                    && let Some(error) = packages.transfer.as_ref().and_then(|transfer| transfer.error.as_ref())
                {
                    small(ui, error, WARN);
                }
            }
            PackageState::Transferring => {
                let transfer = packages.transfer.as_ref().expect("the package state says a transfer is running");
                let paused = transfer.progress.phase == TransferPhase::Paused;
                setting(ui, &package.name, Control::Actions, Some(&subtitle), |ui| {
                    let texts: [&str; 2] = if paused { ["Resume", "Cancel"] } else { ["Pause", "Cancel"] };
                    let clicked = action_buttons(ui, &texts);
                    if clicked[0].clicked() {
                        actions.push(if paused {
                            Action::ResumePackageDownload
                        } else {
                            Action::PausePackageDownload
                        });
                    }
                    if clicked[1].clicked() {
                        actions.push(Action::CancelPackageDownload);
                    }
                });
                transfer_progress(ui, transfer);
            }
        }
    });
}

/// The running transfer's line and rail, from its latest word. The bytes are
/// the whole job's: the package asked for and what it needs.
fn transfer_progress(ui: &mut egui::Ui, transfer: &PackageTransfer) {
    let progress = &transfer.progress;
    let done = progress.bytes_done as f32 / progress.bytes_total.max(1) as f32;
    let of = format!("{} of {} ({:.0}%)", bytes(progress.bytes_done), bytes(progress.bytes_total), done * 100.0);
    let line = match progress.phase {
        TransferPhase::Downloading => format!("Downloading: {of}"),
        TransferPhase::Paused => format!("Paused: {of}"),
        TransferPhase::Verifying => format!("Checking what is already here: {of}"),
        TransferPhase::Installed => "Installed.".to_owned(),
        TransferPhase::Cancelled => "Cancelled.".to_owned(),
    };
    small(ui, &line, theme::MUTED);
    rail(ui, done);
}

/// A section's title, and on its right what `right` adds (a button).
fn section(ui: &mut egui::Ui, title: &str, right: impl FnOnce(&mut egui::Ui)) {
    let scale = theme::density(ui);
    ui.add_space(6.0 * scale);
    ui.horizontal(|ui| {
        ui.label(theme::heading(title, 17.0 * scale).color(theme::MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

/// The server in use's box: the settings rows' look.
fn panel(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let scale = theme::density(ui);
    egui::Frame::new()
        .fill(theme::expressive::SURFACE)
        .corner_radius(14)
        .inner_margin((14.0 * scale) as i8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0 * scale;
            add(ui);
        });
}

fn muted(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0 * theme::density(ui)).color(theme::MUTED));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(id: &str) -> GamePackage {
        GamePackage {
            id: id.to_owned(),
            name: format!("Package {id}"),
            description: String::new(),
            kind: "example".to_owned(),
            version: "1".to_owned(),
            target_build: "build".to_owned(),
            catalog_hash: "hash".to_owned(),
            protocol_min: 1,
            protocol_max: 1,
            dependencies: Vec::new(),
            files: Vec::new(),
            total_size: 2048,
            optional: true,
            available: true,
            locale: None,
            created_at: None,
        }
    }

    fn transfer(id: &str, active: bool) -> PackageTransfer {
        PackageTransfer {
            package_id: id.to_owned(),
            progress: glue5_stream::TransferProgress {
                package_id: id.to_owned(),
                phase: TransferPhase::Downloading,
                file_path: None,
                file_done: 0,
                file_total: 0,
                bytes_done: 0,
                bytes_total: 0,
            },
            error: (!active).then(|| "stopped".to_owned()),
            active,
        }
    }

    #[test]
    fn package_states_come_from_the_manifest_snapshot() {
        let package = package("voice");
        let mut packages = Packages { manifest: None, installed: Vec::new(), transfer: None };
        assert_eq!(package_state(&package, &packages), PackageState::Offered);

        packages.transfer = Some(transfer("voice", true));
        assert_eq!(package_state(&package, &packages), PackageState::Transferring);

        packages.transfer = Some(transfer("voice", false));
        assert_eq!(package_state(&package, &packages), PackageState::Failed);

        packages.transfer = Some(transfer("other", true));
        packages.installed.push("voice".to_owned());
        assert_eq!(package_state(&package, &packages), PackageState::Installed);

        let mut unavailable = package("voice");
        unavailable.available = false;
        packages.installed.clear();
        packages.transfer = None;
        assert_eq!(package_state(&unavailable, &packages), PackageState::Unavailable);
    }

    #[test]
    fn a_package_whose_dependency_is_unavailable_is_unavailable() {
        let mut root = package("root");
        root.dependencies = vec!["dep".to_owned()];
        let mut dep = package("dep");
        dep.available = false;
        let packages = Packages {
            manifest: Some(glue5_stream::PackageManifest {
                schema_version: 1,
                server_id: "server".to_owned(),
                build: "build".to_owned(),
                catalog_hash: "hash".to_owned(),
                protocol_min: 1,
                protocol_max: 1,
                packages: vec![root.clone(), dep],
            }),
            installed: Vec::new(),
            transfer: None,
        };
        assert_eq!(package_state(&root, &packages), PackageState::Unavailable);
    }

    #[test]
    fn only_optional_packages_are_offered() {
        let voice = package("voice");
        let mut needed = package("needed");
        needed.optional = false;
        let manifest = glue5_stream::PackageManifest {
            schema_version: 1,
            server_id: "server".to_owned(),
            build: "build".to_owned(),
            catalog_hash: "hash".to_owned(),
            protocol_min: 1,
            protocol_max: 1,
            packages: vec![voice, needed],
        };
        let offered = offered_packages(&manifest);
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].id, "voice");
    }

    #[test]
    fn package_subtitles_say_only_what_the_manifest_says() {
        let mut package = package("voice");
        assert_eq!(package_subtitle(&package), bytes(package.total_size));

        package.description = "Optional words".to_owned();
        package.dependencies = vec!["base".to_owned()];
        assert_eq!(
            package_subtitle(&package),
            format!("Optional words · {} · what it needs comes with it", bytes(package.total_size))
        );
    }
}
