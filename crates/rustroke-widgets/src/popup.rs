//! Content floating above everything else: popup menus and tooltips.

use rustroke_core::{Point, Rect, Vec2, point, vec2};

use crate::containers::paint_floating_frame;
use crate::context::{Order, popup_layer};
use crate::{Align, Id, LayerId, Layout, Response, Sense, Ui};

/// Seconds the pointer must rest on a widget before its tooltip appears.
const TOOLTIP_DELAY: f64 = 0.5;

/// Keeps `rect` inside `screen` when possible, preferring to move it up or
/// left rather than letting it overflow.
fn keep_on_screen(rect: Rect, screen: Rect) -> Rect {
    let mut min = rect.min;
    min.x = min.x.min(screen.max.x - rect.width()).max(screen.min.x);
    min.y = min.y.min(screen.max.y - rect.height()).max(screen.min.y);
    Rect::from_min_size(min, rect.size())
}

/// Where and how [`show_floating`] draws.
struct Floating {
    layer: LayerId,
    /// Remembers the content size between frames.
    id: Id,
    /// Top-left corner (moved to stay on screen).
    pos: Point,
    /// Content wraps at this width.
    max_width: f32,
    /// The popup is at least this wide (e.g. as wide as a combo box).
    min_width: f32,
    /// Popup menus block clicks to what is below; tooltips don't.
    interactive: bool,
}

/// Draws floating content in `f.layer` at `f.pos`, sized like it was last
/// frame (measured, then remembered under `f.id`).
fn show_floating<R>(
    ui: &mut Ui<'_>,
    f: Floating,
    add_contents: impl FnOnce(&mut Ui<'_>) -> R,
) -> R {
    let Floating {
        layer,
        id,
        pos,
        max_width,
        min_width,
        interactive,
    } = f;
    let style = ui.style();
    let pad = style.spacing.window_padding;
    let screen = ui.input().screen_rect;
    let size: Vec2 = ui.ctx().data(id).unwrap_or(Vec2::ZERO);
    // Content at least `min_width` wide (e.g. a combo box's popup is as
    // wide as the combo box).
    let content_width = size.x.max(min_width - 2.0 * pad);
    let rect = keep_on_screen(
        Rect::from_min_size(pos, vec2(content_width, size.y) + Vec2::splat(2.0 * pad)),
        screen,
    );

    let inner_and_used = {
        let mut floating = ui.layer_ui(layer, id, rect);
        if interactive {
            // Blocks clicks to what is below, and keeps the popup open.
            let _ = floating.interact(id.with("background"), rect, Sense::POINTER_DRAG);
        }
        paint_floating_frame(floating.painter(), rect, &style.visuals);
        let content_rect = Rect::from_min_size(
            rect.min + Vec2::splat(pad),
            vec2(max_width, screen.height()),
        );
        let mut inner = None;
        let used = floating
            .scope_with_no_advance(content_rect, Layout::top_down(Align::Min), |content| {
                // Menu items highlight across the whole menu, which is as
                // wide as the widest item measured last frame.
                content.set_in_menu(interactive.then_some(content_width));
                inner = Some(add_contents(content));
            })
            .response
            .rect;
        (inner.expect("closure ran"), used)
    };
    let (inner, used) = inner_and_used;
    let measured = used.size();
    if measured != size {
        ui.ctx().insert_data(id, measured);
        ui.ctx().request_repaint();
    }
    inner
}

/// Shows the popup belonging to widget `owner` below `anchor`.
pub(crate) fn show_popup<R>(
    ui: &mut Ui<'_>,
    owner: Id,
    anchor: Rect,
    min_width: f32,
    add_contents: impl FnOnce(&mut Ui<'_>) -> R,
) -> R {
    let layer = popup_layer(owner);
    let pos = anchor.left_bottom() + vec2(0.0, 4.0);
    let floating = Floating {
        layer,
        id: layer.id,
        pos,
        max_width: 220.0_f32.max(min_width),
        min_width,
        interactive: true,
    };
    show_floating(ui, floating, add_contents)
}

impl Response {
    /// Opens a popup menu at the pointer when the widget is clicked with
    /// the secondary (right) mouse button. Returns what `add_contents`
    /// returned while the menu is open. It closes like other menus: by
    /// choosing a button inside it, clicking elsewhere or pressing Escape.
    ///
    /// The widget must react to clicks (buttons, selectable labels, images
    /// with `Sense::CLICK`...).
    pub fn context_menu<R>(
        &self,
        ui: &mut Ui<'_>,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> Option<R> {
        let popup = self.id.with("context menu");
        if self.secondary_clicked() {
            let pos = ui
                .input()
                .pointer
                .pos()
                .unwrap_or_else(|| self.rect.center());
            ui.ctx().insert_data(popup, pos);
            ui.ctx().open_popup(popup);
        }
        if !ui.ctx().is_popup_open(popup) {
            return None;
        }
        ui.ctx().keep_alive(popup);
        let pos: Point = ui.ctx().data(popup).unwrap_or_else(|| self.rect.center());
        let anchor = Rect::from_min_size(pos - vec2(0.0, 4.0), Vec2::ZERO);
        Some(show_popup(ui, popup, anchor, 0.0, add_contents))
    }
}

impl Response {
    /// Shows `text` in a tooltip when the widget has been hovered for a
    /// moment.
    pub fn on_hover_text(self, ui: &mut Ui<'_>, text: impl Into<String>) -> Self {
        let hovered_for = ui.ctx().hover_duration(self.id, self.hovered());
        if !self.hovered() || self.is_pressed() {
            return self;
        }
        if hovered_for < TOOLTIP_DELAY {
            ui.ctx().request_repaint_after(TOOLTIP_DELAY - hovered_for);
            return self;
        }
        let Some(pointer) = ui.input().pointer.pos() else {
            return self;
        };
        let text = text.into();
        let id = self.id.with("tooltip");
        let layer = LayerId::new(Order::Tooltip, id);
        let pos = point(pointer.x + 12.0, pointer.y + 18.0);
        let floating = Floating {
            layer,
            id,
            pos,
            max_width: 280.0,
            min_width: 0.0,
            interactive: false,
        };
        show_floating(ui, floating, |ui| {
            let style = ui.style();
            let galley = ui.layout_text(&text, &style.body, Some(280.0));
            let rect = ui.allocate_rect(galley.size);
            ui.painter().galley(rect.min, galley, style.visuals.text);
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_on_screen_moves_rects_inside() {
        let screen = Rect::from_min_size(Point::ZERO, vec2(100.0, 100.0));
        let r = keep_on_screen(
            Rect::from_min_size(point(90.0, 95.0), vec2(20.0, 20.0)),
            screen,
        );
        assert_eq!(r.min, point(80.0, 80.0));
        let big = keep_on_screen(
            Rect::from_min_size(point(50.0, 0.0), vec2(200.0, 20.0)),
            screen,
        );
        assert_eq!(big.min.x, 0.0);
    }
}
