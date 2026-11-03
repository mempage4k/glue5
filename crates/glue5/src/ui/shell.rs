//! The one shell every menu is drawn in, modeled on the GTA V pause menu: a
//! header, a strip of tabs, a list of rows with a description beside it, and a
//! footer of buttons — full screen, never a floating window.
//!
//! The pages supply the rows ([`super::pages::sheet`]); the shell draws the
//! chrome, the scene behind it, and says when a tab was asked for. The tab
//! strip is the app's pages, so the same chrome carries the start page, the
//! settings, the friends, the servers, the mods and the credits.
use std::cell::RefCell;

use egui::*;

use super::theme::{self, fonts, metrics, palette};
use super::{Page, widgets};

/// The tabs, in the pause menu's order: what the player came to do first, then
/// the lists, the mods, and the credits last.
pub(super) const TABS: [(Page, &str); 6] =
    [(Page::Start, "PLAY"), (Page::Settings, "SETTINGS"), (Page::Friends, "FRIENDS"), (Page::Servers, "SERVERS"), (Page::Mods, "MODS"), (Page::Credits, "CREDITS")];

thread_local! {
    /// The page being drawn, for the tab strip to mark.
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
    /// A tab was clicked: go there (`None`: back to the game).
    static NAV: RefCell<Option<Option<Page>>> = const { RefCell::new(None) };
}

/// Before the pages are drawn: which one is open.
pub(super) fn begin(page: Option<Page>) {
    PAGE.with_borrow_mut(|current| *current = page);
}

pub(super) fn current() -> Option<Page> {
    PAGE.with_borrow(|page| *page)
}

/// The full output rectangle from the SDL window, kept distinct from egui's
/// safe content rectangle on devices with display cutouts or system bars.
const OUTPUT_RECT: &str = "glue5-full-output-rect";

pub(super) fn set_output_rect(ctx: &Context, rect: Rect) {
    ctx.data_mut(|data| data.insert_temp(Id::new(OUTPUT_RECT), rect));
}

pub(super) fn output_rect(ctx: &Context) -> Rect {
    ctx.data(|data| data.get_temp(Id::new(OUTPUT_RECT)))
        .unwrap_or_else(|| ctx.viewport_rect())
}

/// Asked for by a tab (or by a page's own link).
pub(super) fn nav(page: Option<Page>) {
    NAV.with_borrow_mut(|nav| *nav = Some(page));
}

/// What a tab asked for, once per frame.
pub(super) fn take_nav() -> Option<Option<Page>> {
    NAV.with_borrow_mut(Option::take)
}

/// The same edge-to-edge panel is used for paint, scene clipping and the
/// game-frame blur. Interactive controls remain inside `safe`.
pub(super) fn panel_rect(viewport: Rect, safe: Rect) -> Rect {
    let right = (viewport.left() + viewport.width() * 0.62).clamp(safe.left(), safe.right());
    Rect::from_min_max(viewport.min, pos2(right, viewport.bottom()))
}

/// The scene showing right of the shell panel. A press there is the player's
/// way back, as pressing the game behind a floating menu is.
fn outside_panel(area: Rect, panel: Rect) -> Option<Rect> {
    (panel.right() < area.right()).then(|| Rect::from_min_max(pos2(panel.right(), area.top()), area.max))
}

/// Only the pages whose rows need a separate help column reserve one. The
/// social, server, mod and credits pages already carry their explanations
/// inline and need the full width for controls.
fn has_help_column(title: &str) -> bool {
    matches!(title, "PLAY" | "SETTINGS")
}

/// The shell around a page: the scene behind it, the header, the tabs, the
/// page's rows in the list column (scrolling), the description of the line the
/// pointer is on, and the footer. True when the player asked to go back.
pub(super) fn frame(ctx: &Context, title: &str, add: impl FnOnce(&mut Ui)) -> bool {
    let area = ctx.content_rect();
    let scale = theme::layout_scale(area);
    let viewport = output_rect(ctx);
    let full_panel = panel_rect(viewport, area);
    let panel_w = full_panel.right() - area.left();
    let pad = metrics::PAD * scale;
    let mut back = false;
    let page = current();
    egui::Area::new(Id::new(("shell", title)))
        .order(Order::Middle)
        .fixed_pos(area.min)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_width(area.width());
            ui.set_height(area.height());
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            // Nothing the pages draw leaves the screen.
            ui.set_clip_rect(area);
            let painter = ui.painter().clone();
            // Translucent black — the pause menu's block colour (0, 0, 0 at
            // 186/255, measured in the game's movies): the scene beneath has
            // been lightly softened inside this same rectangle and stays
            // sharp outside it. Preserve the full output clip, including
            // safe-area insets, so the panel reaches the phone's edges.
            let mut backdrop = ctx.layer_painter(ui.layer_id());
            backdrop.set_clip_rect(viewport);
            backdrop.rect_filled(full_panel, 0, palette::BLOCK);
            let panel = Rect::from_min_size(area.min, vec2(panel_w, area.height()));
            // A press on the scene beside the panel goes back. PLAY is the
            // root: it has nowhere to go back to. A warning over the page
            // still owns its presses (the frame drops this page's answers
            // while one is on show).
            if page != Some(Page::Start)
                && let Some(outside) = outside_panel(area, panel)
                && ui.interact(outside, Id::new(("shell-outside", title)), Sense::click()).clicked()
            {
                back = true;
            }

            let header_h = metrics::HEADER * scale;
            let tabs_h = metrics::TABS * scale;
            let footer_h = metrics::FOOTER * scale;
            let header = Rect::from_min_size(panel.min, vec2(panel.width(), header_h));
            let tabs = Rect::from_min_size(pos2(panel.left(), header.bottom()), vec2(panel.width(), tabs_h));
            let footer = Rect::from_min_size(pos2(panel.left(), panel.bottom() - footer_h), vec2(panel.width(), footer_h));
            let body = Rect::from_min_max(pos2(panel.left() + pad, tabs.bottom() + pad * 0.5), pos2(panel.right() - pad, footer.top() - pad * 0.5));

            // --- header: the product, the page, the build -------------------
            let brand = painter.layout_no_wrap("GLUE 5".to_owned(), FontId::new(17.0 * scale, fonts::heavy()), palette::TEXT_HI);
            painter.galley(pos2(header.left() + pad, header.center().y - brand.size().y * 0.5), brand.clone(), palette::TEXT_HI);
            painter.text(
                pos2(header.left() + pad + brand.size().x + 12.0 * scale, header.center().y),
                Align2::LEFT_CENTER,
                title,
                FontId::new(12.0 * scale, fonts::strong()),
                palette::TEXT_MID,
            );
            painter.text(
                pos2(header.right() - pad, header.center().y),
                Align2::RIGHT_CENTER,
                crate::version::LABEL,
                FontId::new(10.5 * scale, fonts::regular()),
                palette::TEXT_LOW,
            );
            painter.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0, palette::LINE));

            // --- the tabs ---------------------------------------------------
            let tab_count = TABS.len() as f32;
            let tab_w = ((tabs.width() - pad * 2.0) / tab_count).min(112.0 * scale);
            for (index, (target, name)) in TABS.iter().enumerate() {
                let rect = Rect::from_min_size(pos2(tabs.left() + pad + index as f32 * tab_w, tabs.top() + 4.0 * scale), vec2(tab_w, tabs.height() - 8.0 * scale));
                let here = page == Some(*target);
                let response = ui.interact(rect, Id::new(("shell-tab", name)), Sense::click());
                response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, here, *name));
                // Tabs are blocks too, with the pause menu's ~2 px of air
                // between them; the current one inverts to white.
                let tab_block = rect.shrink2(vec2(1.0 * scale, 0.0));
                if here {
                    painter.rect_filled(tab_block, 0, palette::TEXT_HI);
                } else if response.hovered() || response.has_focus() {
                    painter.rect_filled(tab_block, 0, palette::INK_800);
                } else {
                    painter.rect_filled(tab_block, 0, palette::BLOCK);
                }
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    *name,
                    FontId::new(11.5 * scale, fonts::strong()),
                    if here { palette::INK_950 } else { palette::TEXT_MID },
                );
                if response.has_focus() && !here {
                    painter.rect_stroke(rect, 0, Stroke::new(1.0, palette::LIME), StrokeKind::Inside);
                }
                if response.clicked() && !here {
                    // `Ui::show` plays the page turn's own sound.
                    nav(Some(*target));
                }
            }
            painter.line_segment([tabs.left_bottom(), tabs.right_bottom()], Stroke::new(1.0, palette::LINE));

            // PLAY and SETTINGS use the dedicated help column. The other
            // Android pages already put explanations inline; giving them a
            // second, empty description column needlessly clipped their rows
            // and multi-button controls.
            let has_description = has_help_column(title);
            let list_w = if has_description { body.width() * 0.72 } else { body.width() };
            let list = Rect::from_min_size(body.min, vec2(list_w.max(1.0), body.height()));
            let desc = if has_description {
                Rect::from_min_max(pos2(list.right() + metrics::GAP * scale, body.top()), body.max)
            } else {
                Rect::NOTHING
            };
            ui.scope_builder(UiBuilder::new().max_rect(list).layout(Layout::top_down(Align::Min)), |ui| {
                theme::apply_density(ui, scale);
                ui.set_width(list.width());
                ui.spacing_mut().item_spacing.y = 0.0;
                ScrollArea::vertical()
                    .id_salt(("shell-list", title))
                    .auto_shrink([false, false])
                    .max_height(list.height())
                    .show(ui, |ui| {
                        ui.set_width(list.width());
                        add(ui);
                    });
            });

            if has_description {
                painter.line_segment(
                    [pos2(desc.left() - metrics::GAP * scale * 0.5, desc.top()), pos2(desc.left() - metrics::GAP * scale * 0.5, desc.bottom())],
                    Stroke::new(1.0, palette::SEPARATOR),
                );
                description(ctx, &painter, desc, scale);
            }

            // --- footer -----------------------------------------------------
            painter.line_segment([footer.left_top(), footer.right_top()], Stroke::new(1.0, palette::LINE));
            // PLAY is the app's root, not a dismissible overlay. Do not show
            // a Close/Back control that has nowhere to go on that page.
            let can_go_back = page != Some(Page::Start);
            let hint = if can_go_back {
                "A   SELECT      B   BACK      SCROLL      Y   SETTINGS"
            } else {
                "A   SELECT      SCROLL      Y   SETTINGS"
            };
            painter.text(pos2(footer.left() + pad, footer.center().y), Align2::LEFT_CENTER, hint, FontId::new(10.5 * scale, fonts::strong()), palette::TEXT_LOW);
            if can_go_back {
                let close = Rect::from_min_size(pos2(footer.right() - pad - 30.0 * scale, footer.top() + 4.0 * scale), vec2(30.0 * scale, footer.height() - 8.0 * scale));
                let response = ui
                    .scope_builder(UiBuilder::new().max_rect(close).layout(Layout::centered_and_justified(Direction::LeftToRight)), |ui| {
                        ui.spacing_mut().interact_size.y = (close.height() - 2.0 * scale).max(1.0);
                        widgets::close_button(ui)
                    })
                    .inner;
                if response.clicked() {
                    back = true;
                }
            }
        });
    back
}

/// The description of the line under the pointer (its name and its help, which
/// the rows leave in egui's memory): the panel's right column, as the pause
/// menu's help box.
fn description(ctx: &Context, painter: &Painter, rect: Rect, scale: f32) {
    let Some((label, help)) = widgets::description(ctx) else {
        painter.text(
            pos2(rect.left(), rect.top()),
            Align2::LEFT_TOP,
            "MOVE THE POINTER OR THE SELECTION\nFOR A DESCRIPTION",
            FontId::new(10.5 * scale, fonts::strong()),
            palette::TEXT_LOW,
        );
        return;
    };
    let mut y = rect.top();
    let title = painter.layout(label, FontId::new(14.0 * scale, fonts::heavy()), palette::TEXT_HI, rect.width());
    let height = title.size().y;
    painter.galley(pos2(rect.left(), y), title, palette::TEXT_HI);
    y += height + 8.0 * scale;
    if !help.is_empty() {
        let body = painter.layout(help, FontId::new(12.0 * scale, fonts::regular()), palette::TEXT_MID, rect.width());
        let body_h = body.size().y;
        painter.galley(pos2(rect.left(), y), body, palette::TEXT_MID);
        y += body_h;
    }
    // The rest of the column is the page's, if it has anything to say there.
    let _ = y;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_reaches_viewport_edges_even_with_safe_area_insets() {
        use egui::{RawInput, SafeAreaInsets, Shape};

        let ctx = Context::default();
        theme::install(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(844.0, 390.0));
        for _ in 0..4 {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(screen),
                    safe_area_insets: Some(SafeAreaInsets(egui::epaint::MarginF32 {
                        left: 54.0,
                        right: 30.0,
                        top: 20.0,
                        bottom: 18.0,
                    })),
                    ..Default::default()
                },
                |root| {
                    set_output_rect(root.ctx(), screen);
                    frame(root.ctx(), "PLAY", |_| {});
                },
            );
            // Nothing here paints: the font atlas' delta is dropped as it is
            // in the other tests that run a frame (egui panics otherwise).
            output.textures_delta.clear();
            let panel = panel_rect(screen, ctx.content_rect());
            let acrylic = output.shapes.iter().find(|shape| match &shape.shape {
                Shape::Rect(rect) => {
                    rect.rect == panel
                        && rect.fill == palette::BLOCK
                }
                _ => false,
            });
            if let Some(acrylic) = acrylic {
                assert_eq!(acrylic.clip_rect, screen, "acrylic must not be clipped to the safe area");
                return;
            }
        }
        panic!("acrylic panel was not painted");
    }

    #[test]
    fn only_play_and_settings_reserve_a_help_column() {
        assert!(has_help_column("PLAY"));
        assert!(has_help_column("SETTINGS"));
        for title in ["FRIENDS", "SERVERS", "MODS", "CREDITS"] {
            assert!(!has_help_column(title), "{title} already explains its rows inline");
        }
    }

    #[test]
    fn outside_panel_is_the_scene_right_of_the_panel() {
        let area = Rect::from_min_max(pos2(10.0, 20.0), pos2(410.0, 220.0));
        let panel = Rect::from_min_max(pos2(10.0, 20.0), pos2(260.0, 220.0));
        assert_eq!(outside_panel(area, panel), Some(Rect::from_min_max(pos2(260.0, 20.0), pos2(410.0, 220.0))));
        assert_eq!(outside_panel(area, area), None);
    }

    #[test]
    fn shell_panel_bleeds_to_the_output_edges_but_stays_safe_on_the_right() {
        let output = Rect::from_min_size(Pos2::ZERO, vec2(1568.0, 704.0));
        let safe = Rect::from_min_max(pos2(100.0, 52.0), pos2(1468.0, 650.0));
        let panel = panel_rect(output, safe);
        assert_eq!(panel.left_top(), output.left_top());
        assert_eq!(panel.left_bottom(), output.left_bottom());
        assert!((panel.right() - 972.16).abs() < 0.1);
        assert_eq!(panel.bottom(), 704.0);
    }
}
