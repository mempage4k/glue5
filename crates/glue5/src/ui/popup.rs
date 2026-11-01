//! A warning over a page, as the game's own front end shows them: the page
//! goes dark, one square block in the middle carries a title, a few lines
//! and the choices as blocks of their own, the one under the selection
//! solid light with dark text. Nothing behind it answers until it is
//! settled: a destructive line asks here first, instead of acting on the
//! press (or asking "Sure?" on the line itself, a second press away).
//!
//! A page asks with [`ask`]; the frame that drew the page opens it, and the
//! answer comes back as the [`Action`](super::Action) it stood for. The
//! shape follows the alert movies of the game's phone (`CREATE_ALERT`:
//! a title, a dialog box, its options) and the pause menu's warnings,
//! painted with the shell's own blocks and type so the two sit together.
use std::cell::RefCell;

use egui::*;

use super::theme::{self, fonts, metrics, palette};
use crate::sfx::{self, Sound};

/// What is done when the warning is accepted. One entry per line that must
/// ask first; the frame turns it into the page's action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Confirm {
    /// Delete the copy of the game this device streamed.
    ClearCopy,
    /// The game's own graphics settings back to the engine's defaults.
    ResetGraphics,
}

/// A warning to show: its words and what accepting it does. The choices are
/// the confirm's own word and a way back; the way back starts selected, so
/// a stray press can do no harm.
pub(super) struct Request {
    title: &'static str,
    body: &'static str,
    confirm: &'static str,
    action: Confirm,
}

impl Request {
    pub(super) fn clear_copy() -> Self {
        Request {
            title: "CLEAR THE LOCAL COPY",
            body: "This deletes the copy of the game on this device. Your game folder and your saves stay. \
                   What is gone is streamed again when the game next reads it.",
            confirm: "CLEAR",
            action: Confirm::ClearCopy,
        }
    }

    pub(super) fn reset_graphics() -> Self {
        Request {
            title: "RESTORE GRAPHICS",
            body: "The game's own graphics settings go back to the engine's defaults. \
                   This application's settings (resolution scale, upscaling, frame rate) stay as they are.",
            confirm: "RESET",
            action: Confirm::ResetGraphics,
        }
    }

    /// The two choices' words, confirm first.
    fn options(&self) -> [&'static str; 2] {
        [self.confirm, "BACK"]
    }
}

thread_local! {
    /// A page asked during its frame; the frame opens it after the page.
    static ASK: RefCell<Option<Request>> = const { RefCell::new(None) };
}

/// Ask for a warning over the page being drawn.
pub(super) fn ask(request: Request) {
    ASK.with_borrow_mut(|asked| *asked = Some(request));
}

/// What a page asked for, once per frame.
pub(super) fn take() -> Option<Request> {
    ASK.with_borrow_mut(Option::take)
}

/// The keys a warning answers, taken out of a frame's input before the page
/// under it can see them (an arrow must not also move a slider's value, an
/// Enter not also press the line the warning is asking about).
#[derive(Clone, Copy, Default)]
pub(super) struct Keys {
    /// Steps of the selection, up or left against down or right.
    move_by: i32,
    accept: bool,
}

impl Keys {
    #[cfg(test)]
    fn accept() -> Self {
        Keys { move_by: 0, accept: true }
    }

    #[cfg(test)]
    fn down() -> Self {
        Keys { move_by: 1, accept: false }
    }
}

pub(super) fn take_keys(events: &mut Vec<Event>) -> Keys {
    let mut keys = Keys::default();
    events.retain(|event| {
        let Event::Key { key, pressed, .. } = event else {
            return true;
        };
        match key {
            Key::ArrowUp | Key::ArrowLeft => {
                keys.move_by -= i32::from(*pressed);
                false
            }
            Key::ArrowDown | Key::ArrowRight => {
                keys.move_by += i32::from(*pressed);
                false
            }
            Key::Enter | Key::Space => {
                keys.accept |= *pressed;
                false
            }
            _ => true,
        }
    });
    keys
}

/// The selection after a step of `delta` among `count` choices: it wraps,
/// as the game's lists do.
fn moved(selected: usize, delta: i32, count: usize) -> usize {
    (selected as i32 + delta).rem_euclid(count.max(1) as i32) as usize
}

/// A warning on show: the request and the choice under the selection.
pub(super) struct Open {
    request: Request,
    /// 0 confirms; the last way back starts selected.
    selected: usize,
}

impl Open {
    pub(super) fn new(request: Request) -> Self {
        let selected = request.options().len() - 1;
        Open { request, selected }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    /// Still waiting for an answer.
    Open,
    Confirmed(Confirm),
    Cancelled,
}

/// Draws the warning over the whole screen and answers for it: the dim
/// covers `screen` edge to edge (the shell's acrylic ignores the safe area
/// the same way), while the box itself is sized — and kept — inside `safe`,
/// the content rect the pages live in, so it never runs under a cut-out or
/// the on-screen keyboard. Dismissing it from outside (the Back button) is
/// the frame's own: dropping the [`Open`] is a cancel.
pub(super) fn show(ctx: &Context, open: &mut Open, screen: Rect, safe: Rect, keys: Keys) -> Outcome {
    if keys.move_by != 0 {
        let next = moved(open.selected, keys.move_by, open.request.options().len());
        if next != open.selected {
            open.selected = next;
            sfx::want(Sound::Focus);
        }
    }
    let mut outcome = Outcome::Open;
    Area::new(Id::new("popup"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_width(screen.width());
            ui.set_height(screen.height());
            ui.set_clip_rect(screen);
            let painter = ui.painter().clone();
            // The whole screen goes dark, corners and cut-out alike, and
            // the press that lands anywhere on it lands on the warning,
            // not on a line behind it.
            painter.rect_filled(screen, 0, Color32::from_black_alpha(165));
            let _ = ui.interact(screen, Id::new("popup-backdrop"), Sense::click());

            let scale = theme::layout_scale(safe);
            let pad = 16.0 * scale;
            let width = (440.0 * scale).min((safe.width() - 32.0 * scale).max(1.0));
            let body = painter.layout(
                open.request.body.to_owned(),
                FontId::new(12.5 * scale, fonts::regular()),
                palette::TEXT_MID,
                (width - 2.0 * pad).max(1.0),
            );
            let header_h = 40.0 * scale;
            let row_h = metrics::ROW * scale;
            let gap = 2.0 * scale;
            let options = open.request.options();
            let options_h = row_h * options.len() as f32 + gap * (options.len() - 1) as f32;
            let footer_h = 30.0 * scale;
            let height = header_h + pad + body.size().y + pad + options_h + pad * 0.75 + footer_h;
            let height = height.min(safe.height());
            // Centred on the screen, then slid back inside the safe area
            // if a cut-out or the keyboard would cover an edge of it.
            let half = vec2(width, height) * 0.5;
            let center = pos2(
                screen.center().x.clamp(safe.left() + half.x, (safe.right() - half.x).max(safe.left() + half.x)),
                screen.center().y.clamp(safe.top() + half.y, (safe.bottom() - half.y).max(safe.top() + half.y)),
            );
            let box_rect = Rect::from_center_size(center, vec2(width, height));
            painter.rect_filled(box_rect, 0, Color32::from_rgba_unmultiplied(10, 11, 14, 245));
            painter.rect_stroke(box_rect, 0, Stroke::new(1.0, palette::LINE), StrokeKind::Inside);

            // --- the title, and where the selection stands -----------------
            painter.text(
                pos2(box_rect.left() + pad, box_rect.top() + header_h * 0.5),
                Align2::LEFT_CENTER,
                open.request.title,
                FontId::new(14.0 * scale, fonts::heavy()),
                palette::TEXT_HI,
            );
            painter.text(
                pos2(box_rect.right() - pad, box_rect.top() + header_h * 0.5),
                Align2::RIGHT_CENTER,
                format!("{}/{}", open.selected + 1, options.len()),
                FontId::new(11.0 * scale, fonts::strong()),
                palette::TEXT_LOW,
            );
            painter.line_segment(
                [pos2(box_rect.left(), box_rect.top() + header_h), pos2(box_rect.right(), box_rect.top() + header_h)],
                Stroke::new(1.0, palette::LINE),
            );

            // --- the words ---------------------------------------------------
            let body_at = pos2(box_rect.left() + pad, box_rect.top() + header_h + pad);
            let body_h = body.size().y;
            painter.galley(body_at, body, palette::TEXT_MID);

            // --- the choices: blocks, the selected one inverted -------------
            let mut y = body_at.y + body_h + pad;
            for (index, word) in options.iter().enumerate() {
                let rect = Rect::from_min_size(pos2(box_rect.left() + pad, y), vec2(width - 2.0 * pad, row_h));
                let response = ui.interact(rect, Id::new(("popup-option", index)), Sense::click());
                response
                    .widget_info(|| WidgetInfo::selected(WidgetType::Button, true, index == open.selected, *word));
                if response.hovered() && open.selected != index {
                    open.selected = index;
                    sfx::want(Sound::Focus);
                }
                let selected = index == open.selected;
                if selected {
                    painter.rect_filled(rect, 0, palette::TEXT_HI);
                } else {
                    painter.rect_filled(rect, 0, palette::INK_800);
                    painter.rect_stroke(rect, 0, Stroke::new(1.0, palette::LINE), StrokeKind::Inside);
                }
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    *word,
                    FontId::new(13.0 * scale, fonts::strong()),
                    if selected { palette::INK_950 } else { palette::TEXT_MID },
                );
                if response.clicked() {
                    open.selected = index;
                    outcome = if index == 0 { Outcome::Confirmed(open.request.action) } else { Outcome::Cancelled };
                }
                y += row_h + gap;
            }

            // --- the buttons' names, as the pages' footer has them ---------
            let footer_top = box_rect.bottom() - footer_h;
            painter.line_segment(
                [pos2(box_rect.left(), footer_top), pos2(box_rect.right(), footer_top)],
                Stroke::new(1.0, palette::LINE),
            );
            painter.text(
                pos2(box_rect.left() + pad, footer_top + footer_h * 0.5),
                Align2::LEFT_CENTER,
                "A   SELECT      B   BACK",
                FontId::new(10.5 * scale, fonts::strong()),
                palette::TEXT_LOW,
            );
        });
    if outcome == Outcome::Open && keys.accept {
        outcome = if open.selected == 0 {
            Outcome::Confirmed(open.request.action)
        } else {
            Outcome::Cancelled
        };
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;

    #[test]
    fn the_selection_wraps_at_both_ends() {
        assert_eq!(moved(0, -1, 2), 1);
        assert_eq!(moved(1, 1, 2), 0);
        assert_eq!(moved(0, 1, 2), 1);
        assert_eq!(moved(1, -1, 2), 0);
        assert_eq!(moved(1, 0, 2), 1);
    }

    #[test]
    fn its_keys_leave_the_rest_of_the_input_alone() {
        let key = |key: Key, pressed: bool| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let mut events = vec![
            key(Key::ArrowDown, true),
            key(Key::ArrowDown, false),
            key(Key::Tab, true),
            key(Key::Enter, true),
            Event::PointerMoved(pos2(1.0, 2.0)),
        ];
        let keys = take_keys(&mut events);
        assert_eq!(keys.move_by, 1);
        assert!(keys.accept);
        assert_eq!(events.len(), 2, "Tab and the pointer stay for the page: {events:?}");
    }

    #[test]
    fn a_warning_starts_on_the_way_back_and_answers() {
        let ctx = Context::default();
        theme::install(&ctx);
        let area = Rect::from_min_size(Pos2::ZERO, vec2(844.0, 390.0));
        let mut open = Open::new(Request::clear_copy());
        let mut outcome = Outcome::Open;
        for (frame, keys) in [(0, Keys::default()), (1, Keys::accept())] {
            let mut output = ctx.run_ui(
                RawInput { screen_rect: Some(area), ..Default::default() },
                |root| {
                    outcome = show(root.ctx(), &mut open, area, area, keys);
                    let _ = frame;
                },
            );
            output.textures_delta.clear();
            if frame == 0 {
                assert_eq!(outcome, Outcome::Open, "unanswered, it stays");
            }
        }
        assert_eq!(outcome, Outcome::Cancelled, "accepting the opening selection is a cancel");

        // A step up from the way back lands on the confirm.
        let mut open = Open::new(Request::reset_graphics());
        let mut outcome = Outcome::Open;
        for keys in [Keys { move_by: -1, accept: false }, Keys::accept()] {
            let mut output = ctx.run_ui(
                RawInput { screen_rect: Some(area), ..Default::default() },
                |root| outcome = show(root.ctx(), &mut open, area, area, keys),
            );
            output.textures_delta.clear();
        }
        assert_eq!(outcome, Outcome::Confirmed(Confirm::ResetGraphics));
    }

    #[test]
    fn a_step_down_wraps_the_same_way() {
        let mut open = Open::new(Request::clear_copy());
        open.selected = moved(open.selected, Keys::down().move_by, open.request.options().len());
        assert_eq!(open.selected, 0, "down from the last choice wraps to the first");
    }
}
