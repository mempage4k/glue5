//! The shell's rows and controls: one line per setting, its label at the
//! left, its value at the right, each line a translucent black block with
//! ~2 px of air around it; the selected line goes solid light with dark
//! text, as the pause menu's does.
//!
//! Older pages still call the older helpers (`setting`, `segmented`, ...),
//! which now draw with the same flat surfaces, square corners and 1 px rules.
use super::TextKind;
use super::theme::{MUTED, density, fonts, metrics, palette};
use crate::sfx::{self, Sound};
use egui::*;

/// Where the shell keeps the line the description panel is about, and the
/// state of the line being drawn (its text goes dark on a selected line).
const SELECTED: &str = "shell: selected row";
const SELECTED_TEXT: &str = "shell: selected row text";
const ROW_LIGHT: &str = "shell: row is light";

pub(super) fn focus(r: Response) -> Response {
    if r.gained_focus() {
        r.scroll_to_me(None);
    }
    r
}

/// Text and rules inside a line that is lit: dark on light.
fn light(ui: &Ui) -> bool {
    ui.ctx().data(|d| d.get_temp::<bool>(Id::new(ROW_LIGHT))).unwrap_or(false)
}

fn colors(ui: &Ui) -> (Color32, Color32, Color32) {
    if light(ui) {
        (palette::INK_950, Color32::from_rgb(70, 72, 76), palette::INK_950)
    } else {
        (palette::TEXT_HI, palette::TEXT_MID, palette::LIME)
    }
}

/// One line of the shell: `label` at the left, `add` drawing the value at the
/// right. Returns what `add` returned, plus the line's own hover: focused
/// (`|`). While the line is under the pointer or holds the focus it is lit,
/// and its help is what the description panel shows.
pub(super) fn row(ui: &mut Ui, label: &str, help: &str, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    let scale = density(ui);
    let height = metrics::ROW * scale;
    let width = ui.available_width();
    let id = ui.id().with(("row", label));
    let (rect, hovered) = ui.allocate_exact_size(vec2(width, height), Sense::click());

    let mut lit = hovered.hovered();
    let last: Option<Id> = ui.ctx().data(|d| d.get_temp(Id::new(SELECTED)));
    if !lit {
        // The focus may be on a control inside this line (a slider, a button):
        // the next frame then lights the line, and the one after that has the
        // help in place. One frame: invisible, and a repaint is asked for.
        lit = last == Some(id) && ui.ctx().memory(|m| m.focused()).is_some();
    }

    let background = if lit { palette::TEXT_HI } else if hovered.hovered() { palette::INK_800 } else { palette::BLOCK };
    // The block leaves ~1 px of air top and bottom, so stacked rows read as
    // the pause menu's separate blocks (its 27 px row, 25 px highlight).
    // The interact rect keeps the full height: only the paint shrinks.
    ui.painter().rect_filled(rect.shrink2(vec2(0.0, 1.0 * scale)), 0, background);

    let inset = 12.0 * scale;
    let control_width = (rect.width() * 0.46).max(96.0 * scale);
    let label_rect = Rect::from_min_max(pos2(rect.left() + inset, rect.top()), pos2(rect.right() - control_width - inset, rect.bottom()));
    let galley = WidgetText::from(RichText::new(label).font(FontId::new(13.5 * scale, fonts::strong())).color(if lit { palette::INK_950 } else { palette::TEXT_HI }))
        .into_galley(ui, Some(TextWrapMode::Truncate), label_rect.width(), FontId::new(13.5 * scale, fonts::strong()));
    ui.painter().galley(pos2(label_rect.left(), label_rect.center().y - galley.size().y * 0.5), galley, palette::TEXT_HI);

    let control = Rect::from_min_max(pos2(rect.right() - control_width - inset, rect.top()), pos2(rect.right() - inset, rect.bottom()));
    let response = ui
        .scope_builder(UiBuilder::new().max_rect(control).layout(Layout::right_to_left(Align::Center)), |ui| {
            ui.ctx().data_mut(|d| d.insert_temp(Id::new(ROW_LIGHT), lit));
            let response = add(ui);
            ui.ctx().data_mut(|d| d.insert_temp(Id::new(ROW_LIGHT), false));
            response
        })
        .inner;

    let lit_now = hovered.hovered() || response.hovered() || response.has_focus();
    if lit_now {
        ui.ctx().data_mut(|d| {
            d.insert_temp(Id::new(SELECTED), id);
            d.insert_temp(Id::new(SELECTED_TEXT), (label.to_owned(), help.to_owned()));
        });
    }
    if lit_now != lit {
        // The line goes light (or dark) now; the frame that would show it
        // comes with this request, not with the next key press.
        ui.ctx().request_repaint();
    }
    if lit_now && !lit {
        // The selection moved onto this line: the tick the pause menu makes.
        sfx::want(Sound::Focus);
    }
    hovered | response
}

/// The label and the help of the line the pointer is on or the focus is in,
/// for the description panel (drawn after the lines, in its own column).
pub(super) fn describe(ctx: &Context, label: &str, help: &str) {
    ctx.data_mut(|d| d.insert_temp(Id::new(SELECTED_TEXT), (label.to_owned(), help.to_owned())));
}

pub(super) fn description(ctx: &Context) -> Option<(String, String)> {
    ctx.data(|d| d.get_temp(Id::new(SELECTED_TEXT)))
}

/// A field being edited stays in sight: while it has the focus and is not
/// all inside the visible part of its scroll area (the page ended above the
/// on-screen keyboard, ...), the area scrolls to it, frame after frame
/// until it is (it follows the sheet as it shrinks).
pub(super) fn keep_in_view(ui: &Ui, focused: bool, rect: Rect) {
    if focused && !ui.clip_rect().contains_rect(rect) {
        ui.scroll_to_rect(rect, None);
    }
}

pub(super) fn close_button(ui: &mut Ui) -> Response {
    let scale = density(ui);
    // A row's square at the most, and never more than the space it is given:
    // the footer is shorter than a row, and the shell ends at the screen.
    let wanted = metrics::ROW * scale;
    let size = ui.available_size().min(vec2(wanted, wanted));
    let (r, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), "Close"));
    let c = r.center();
    let color = if response.hovered() || response.has_focus() { palette::TEXT_HI } else { palette::TEXT_MID };
    for sign in [-1.0, 1.0] {
        ui.painter().line_segment(
            [c + vec2(-5.0 * scale, -5.0 * scale * sign), c + vec2(5.0 * scale, 5.0 * scale * sign)],
            Stroke::new(1.4, color),
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(r, 0, Stroke::new(1.0, palette::LIME), StrokeKind::Inside);
    }
    focus(response).on_hover_text("Close")
}

#[derive(Clone, Copy)]
pub(super) enum Control {
    Slider,
    Choice,
    Toggle,
    Action,
    /// A few buttons side by side (`action_buttons`).
    Actions,
}

/// The older card row: a bordered block with the label at the left and the
/// control at its right. Pages not yet moved to `row` still use it.
pub(super) fn setting<R>(
    ui: &mut Ui,
    label: &str,
    kind: Control,
    subtitle: Option<&str>,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let scale = density(ui);
    ui.push_id(label, |ui| {
        let (fill, outline) = match kind {
            Control::Action | Control::Actions => (palette::INK_800, palette::LINE),
            Control::Slider | Control::Choice | Control::Toggle => (Color32::TRANSPARENT, palette::SEPARATOR),
        };
        Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, outline))
            .inner_margin((12.0 * scale) as i8)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let available = ui.available_width();
                let width = match kind {
                    Control::Slider => (available * 0.55).min(260.0 * scale),
                    Control::Choice => (available * 0.60).min(180.0 * scale),
                    Control::Toggle => 44.0 * scale,
                    Control::Action => (available * 0.55).min(180.0 * scale),
                    Control::Actions => (available * 0.62).min(320.0 * scale),
                };
                let label_width = (available - width - ui.spacing().item_spacing.x).max(1.0);
                ui.horizontal_top(|ui| {
                    let (rect, response) = ui.allocate_exact_size(vec2(label_width, metrics::ROW * scale), Sense::hover());
                    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), label));
                    paint_label(
                        ui,
                        rect,
                        label,
                        13.5 * scale,
                        if subtitle.is_some() { -7.0 * scale } else { 0.0 },
                        palette::TEXT_HI,
                        fonts::strong(),
                    );
                    if let Some(text) = subtitle {
                        paint_label(ui, rect, text, 11.0 * scale, 9.0 * scale, palette::TEXT_MID, fonts::regular());
                        response.on_hover_text(text);
                    } else {
                        response.on_hover_text(label);
                    }
                    ui.allocate_ui_with_layout(vec2(width, metrics::ROW * scale), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().slider_width = (width - 56.0 * scale).max(24.0 * scale);
                        ui.spacing_mut().interact_size.y = metrics::ROW * scale;
                        add(ui)
                    })
                    .inner
                })
                .inner
            })
            .inner
    })
    .inner
}

fn paint_label(ui: &Ui, rect: Rect, text: &str, size: f32, offset: f32, color: Color32, family: FontFamily) {
    let galley = WidgetText::from(RichText::new(text).size(size).color(color)).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        rect.width(),
        FontId::new(size, family),
    );
    let pos = pos2(rect.left(), rect.center().y + offset - galley.size().y * 0.5);
    ui.painter().galley(pos, galley, color);
}

/// A line of text in a field of its own, its name on the left (if any).
/// `kind` picks the system keyboard on touch screens, where an empty code or
/// address field also offers to paste (their keyboard has no way to).
pub(super) fn text_field(ui: &mut Ui, label: &str, kind: TextKind, text: &mut String, hint: &str, max: usize) -> Response {
    let scale = density(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), metrics::ROW * scale), Sense::hover());
    ui.painter().rect_filled(rect, 0, palette::INK_800);
    let field = ui.interact(rect, ui.id().with(label), Sense::click());
    let mut inner = rect.shrink2(vec2(12.0 * scale, 0.0));
    let label_width = if label.is_empty() { 0.0 } else { 52.0 * scale };
    ui.painter().text(inner.left_center(), Align2::LEFT_CENTER, label, FontId::new(12.0 * scale, fonts::strong()), MUTED);
    if crate::paths::TOUCH && matches!(kind, TextKind::Code | TextKind::Address) && text.is_empty() {
        let paste = Rect::from_min_max(pos2(inner.right() - 56.0 * scale, rect.top()), pos2(rect.right(), rect.bottom()));
        let response = ui.interact(paste, ui.id().with((label, "paste")), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), "Paste"));
        let color = if response.hovered() || response.has_focus() { palette::TEXT_HI } else { MUTED };
        ui.painter().text(paste.center(), Align2::CENTER_CENTER, "Paste", FontId::new(12.0 * scale, fonts::strong()), color);
        if response.clicked()
            && let Some(pasted) = super::clipboard_text()
        {
            *text = pasted.trim().chars().take(max).collect();
        }
        inner.max.x = paste.left();
    }
    let edit_rect = Rect::from_min_max(pos2(inner.left() + label_width, inner.top()), inner.max);
    let edit = ui
        .scope_builder(UiBuilder::new().max_rect(edit_rect).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.add(
                TextEdit::singleline(text)
                    .frame(Frame::NONE)
                    .hint_text(RichText::new(hint).color(MUTED))
                    .char_limit(max)
                    .font(FontId::new(14.0 * scale, fonts::regular()))
                    .margin(Vec2::ZERO)
                    .desired_width(edit_rect.width()),
            )
        })
        .inner;
    if field.clicked() {
        edit.request_focus();
    }
    if edit.has_focus() {
        ui.painter().rect_stroke(rect, 0, Stroke::new(1.0, palette::LIME), StrokeKind::Inside);
        ui.ctx().data_mut(|d| d.insert_temp(Id::new(super::TEXT_KIND), kind));
    }
    focus(edit)
}

/// Options side by side, the chosen one solid light (older pages' tabs).
/// Returns whether the value changed.
pub(super) fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)], height: f32) -> bool {
    let scale = density(ui);
    let previous = *value;
    Frame::new()
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, palette::LINE))
        .inner_margin(0)
        .show(ui, |ui| {
            let w = ui.available_width() / options.len() as f32;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (index, &(option, label)) in options.iter().enumerate() {
                    let on = *value == option;
                    let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
                    let response = ui.interact(rect, ui.id().with(label), Sense::click());
                    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), on, label));
                    if index > 0 {
                        ui.painter().line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0, palette::LINE));
                    }
                    let lit = on || response.hovered() || response.has_focus();
                    if on {
                        ui.painter().rect_filled(rect, 0, palette::TEXT_HI);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 0, palette::INK_800);
                    }
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::new(12.0 * scale, fonts::strong()),
                        if lit { palette::INK_950 } else { palette::TEXT_MID },
                    );
                    if response.has_focus() {
                        ui.painter().rect_stroke(rect, 0, Stroke::new(1.0, palette::LIME), StrokeKind::Inside);
                    }
                    if focus(response).clicked() {
                        *value = option;
                        sfx::want(Sound::Tab);
                    }
                }
            });
        });
    *value != previous
}

pub(super) fn action_button(ui: &mut Ui, text: &str) -> Response {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| button(ui, text)).inner
}

/// The same button where the layout puts it (in a card's text, not a row's end).
pub(super) fn inline_button(ui: &mut Ui, text: &str) -> Response {
    button(ui, text)
}

/// Buttons side by side at the end of a row, in the order of `texts`:
/// their responses in that order. (`action_button` twice would draw the
/// second over the first.)
pub(super) fn action_buttons(ui: &mut Ui, texts: &[&str]) -> Vec<Response> {
    let mut responses = ui
        .with_layout(Layout::right_to_left(Align::Center), |ui| {
            texts.iter().rev().map(|text| button(ui, text)).collect::<Vec<_>>()
        })
        .inner;
    responses.reverse();
    responses
}

fn button(ui: &mut Ui, text: &str) -> Response {
    let scale = density(ui);
    let (ink, _, _) = colors(ui);
    let response = focus(
        ui.add(
            Button::new(RichText::new(text).font(FontId::new(12.5 * scale, fonts::strong())).color(ink))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::new(1.0, if light(ui) { palette::INK_950 } else { palette::LINE }))
                .corner_radius(0)
                .min_size(vec2(0.0, 30.0 * scale)),
        ),
    );
    if response.clicked() {
        sfx::want(Sound::Confirm);
    }
    response
}

pub(super) fn choice<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> Response {
    let scale = density(ui);
    let previous = *value;
    let index = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let (ink, _, accent) = colors(ui);
    let mut response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().interact_size = vec2(22.0 * scale, metrics::ROW * scale);
            let arrow = |ui: &mut Ui, text: &str| {
                let (rect, response) = ui.allocate_exact_size(vec2(22.0 * scale, metrics::ROW * scale), Sense::click());
                let lit = response.hovered() || response.has_focus();
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::new(13.0 * scale, fonts::strong()),
                    if lit { accent } else { ink },
                );
                if response.has_focus() {
                    ui.painter().rect_stroke(rect, 0, Stroke::new(1.0, accent), StrokeKind::Inside);
                }
                focus(response)
            };
            let left = arrow(ui, "‹").on_hover_text("Previous value");
            let value_width = (ui.available_width() - 44.0 * scale).max(1.0);
            ui.allocate_ui_with_layout(vec2(value_width, metrics::ROW * scale), Layout::right_to_left(Align::Center), |ui| {
                ui.add(Label::new(RichText::new(options[index].1).font(FontId::new(13.0 * scale, fonts::strong())).color(ink)).truncate());
            });
            let right = arrow(ui, "›").on_hover_text("Next value");
            if left.clicked() {
                *value = options[(index + options.len() - 1) % options.len()].0;
                sfx::want(Sound::Adjust);
            }
            if right.clicked() {
                *value = options[(index + 1) % options.len()].0;
                sfx::want(Sound::Adjust);
            }
            left | right
        })
        .inner;
    if *value != previous {
        response.mark_changed();
    }
    response
}

pub(super) fn slider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
    suffix: &str,
    decimals: usize,
) -> Response {
    let scale = density(ui);
    let (ink, dim, accent) = colors(ui);
    ui.horizontal(|ui| {
        let start = *range.start();
        let end = *range.end();
        let (r, mut response) = ui.allocate_exact_size(vec2(ui.spacing().slider_width, metrics::ROW * scale), Sense::click_and_drag());
        let old = *value;
        if response.clicked() {
            response.request_focus();
        }
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - r.left() - 5.0 * scale) / (r.width() - 10.0 * scale).max(1.0)).clamp(0.0, 1.0);
            *value = ((start + t * (end - start)) / step as f32).round() * step as f32;
        }
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(response.id, EventFilter { horizontal_arrows: true, ..Default::default() })
            });
            let delta = ui.input(|i| i.num_presses(Key::ArrowRight) as f32 - i.num_presses(Key::ArrowLeft) as f32);
            *value += delta * step as f32;
        }
        *value = value.clamp(start, end);
        if *value != old {
            response.mark_changed();
            sfx::want(Sound::Adjust);
        }
        response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), *value as f64, ""));
        let rail = Rect::from_min_max(
            pos2(r.left() + 5.0 * scale, r.center().y - 1.0 * scale),
            pos2(r.right() - 5.0 * scale, r.center().y + 1.0 * scale),
        );
        let x = rail.left() + rail.width() * ((*value - start) / (end - start));
        ui.painter().rect_filled(rail, 0, dim);
        ui.painter().rect_filled(Rect::from_min_max(rail.min, pos2(x, rail.bottom())), 0, accent);
        ui.painter().rect_filled(
            Rect::from_center_size(pos2(x, r.center().y), vec2(3.0 * scale, 13.0 * scale)),
            0,
            accent,
        );
        if response.has_focus() {
            ui.painter().rect_stroke(r, 0, Stroke::new(1.0, accent), StrokeKind::Inside);
        }
        let (r, _) = ui.allocate_exact_size(vec2(52.0 * scale, metrics::ROW * scale), Sense::hover());
        let shown = if suffix == "%" { *value * 100.0 } else { *value };
        ui.painter().text(
            r.right_center(),
            Align2::RIGHT_CENTER,
            format!("{:.*}{}", decimals, shown, suffix),
            FontId::new(13.0 * scale, fonts::strong()),
            ink,
        );
        focus(response)
    })
    .inner
}

pub(super) fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let scale = density(ui);
    let (ink, dim, accent) = colors(ui);
    let (r, mut response) = ui.allocate_exact_size(vec2(52.0 * scale, metrics::ROW * scale), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
        sfx::want(if *on { Sound::ToggleOn } else { Sound::ToggleOff });
    }
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    let box_rect = Rect::from_center_size(pos2(r.right() - 6.0 * scale, r.center().y), Vec2::splat(11.0 * scale));
    if *on {
        ui.painter().rect_filled(box_rect, 0, accent);
    } else {
        ui.painter().rect_stroke(box_rect, 0, Stroke::new(1.0, dim), StrokeKind::Inside);
    }
    ui.painter().text(
        pos2(box_rect.left() - 8.0 * scale, r.center().y),
        Align2::RIGHT_CENTER,
        if *on { "ON" } else { "OFF" },
        FontId::new(12.0 * scale, fonts::strong()),
        if response.hovered() || response.has_focus() { accent } else { ink },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(r, 0, Stroke::new(1.0, accent), StrokeKind::Inside);
    }
    focus(response)
}

/// A line's action: plain text, underlined while the pointer is on it.
pub(super) fn action_value(ui: &mut Ui, text: &str) -> Response {
    let scale = density(ui);
    let (ink, _, accent) = colors(ui);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(12.5 * scale, fonts::strong()), ink);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + 12.0 * scale, metrics::ROW * scale), Sense::click());
    let lit = response.hovered() || response.has_focus();
    let color = if lit { accent } else { ink };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(12.5 * scale, fonts::strong()), color);
    let size = galley.size();
    let at = pos2(rect.right() - 6.0 * scale - size.x, rect.center().y - size.y * 0.5);
    ui.painter().galley(at, galley, color);
    if lit {
        let y = at.y + size.y;
        ui.painter().line_segment([pos2(at.x, y), pos2(at.x + size.x, y)], Stroke::new(1.0, accent));
    }
    if response.has_focus() {
        ui.painter().rect_stroke(rect, 0, Stroke::new(1.0, accent), StrokeKind::Inside);
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), text));
    if response.clicked() {
        sfx::want(Sound::Confirm);
    }
    focus(response)
}

/// Plain text at a line's right (a path, a count, a state).
pub(super) fn value_text(ui: &mut Ui, text: &str) -> Response {
    let scale = density(ui);
    let (ink, dim, _) = colors(ui);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(12.5 * scale, fonts::regular()), dim);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + 4.0 * scale, metrics::ROW * scale), Sense::hover());
    let at = pos2(rect.right() - galley.size().x, rect.center().y - galley.size().y * 0.5);
    ui.painter().galley(at, galley, if response.hovered() { ink } else { dim });
    response
}

/// A line that only says something: its name, its help, and the value at the
/// right, not interactive.
pub(super) fn info_text(ui: &mut Ui, label: &str, help: &str, text: &str) {
    row(ui, label, help, |ui| value_text(ui, text));
}

/// A heading inside the content: a section's name in the shell's caption.
pub(super) fn section(ui: &mut Ui, text: &str) {
    let scale = density(ui);
    ui.add_space(6.0 * scale);
    ui.label(RichText::new(text).font(FontId::new(11.0 * scale, fonts::strong())).color(palette::TEXT_LOW));
    ui.add_space(2.0 * scale);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;

    #[test]
    fn slider_keyboard_keeps_focus_and_reports_changes() {
        let ctx = Context::default();
        theme::install(&ctx);
        let mut value = 0.75;
        let mut id = Id::NULL;
        let mut changed = false;
        for frame in 0..3 {
            let events = if frame == 2 {
                vec![Event::Key {
                    key: Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }]
            } else {
                vec![]
            };
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(390.0, 844.0))),
                    events,
                    ..Default::default()
                },
                |root| {
                    CentralPanel::default().show(root, |ui| {
                        let response = slider(ui, &mut value, 0.5..=1.0, 0.05, "%", 0);
                        id = response.id;
                        if frame < 2 {
                            response.request_focus();
                        }
                        changed = response.changed();
                    });
                },
            );
            output.textures_delta.clear();
        }
        assert!((value - 0.8).abs() < 0.001);
        assert!(changed);
        assert_eq!(ctx.memory(|m| m.focused()), Some(id));
    }

    #[test]
    fn controls_and_action_buttons_share_the_right_edge() {
        for width in [248.0, 318.0, 640.0] {
            let ctx = Context::default();
            theme::install(&ctx);
            let mut rect = Rect::NOTHING;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width + 40.0, 500.0))),
                    ..Default::default()
                },
                |root| {
                    CentralPanel::default().show(root, |ui| {
                        ui.set_width(width);
                        setting(
                            ui,
                            "Game folder",
                            Control::Action,
                            Some("A very long folder path to truncate"),
                            |ui| {
                                rect = action_button(ui, "Choose…").rect;
                            },
                        );
                        // The row keeps a 12 pt margin on the right: its
                        // control ends there (never at the panel's edge, and
                        // never short of it).
                        let right = ui.min_rect().right() - 12.0 * theme::density(ui);
                        assert!(
                            rect.right() <= right + 1.0 && rect.right() >= right - 2.0,
                            "{width}: {rect:?}, {right}, {:?}",
                            ui.min_rect()
                        );
                    });
                },
            );
            output.textures_delta.clear();
        }
    }
}
