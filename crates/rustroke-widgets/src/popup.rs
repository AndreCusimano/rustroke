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

/// Draws floating content in `layer` at `pos`, sized like it was last
/// frame (measured, then remembered under `id`).
fn show_floating<R>(
    ui: &mut Ui<'_>,
    layer: LayerId,
    id: Id,
    pos: Point,
    max_width: f32,
    interactive: bool,
    add_contents: impl FnOnce(&mut Ui<'_>) -> R,
) -> R {
    let style = ui.style();
    let pad = style.spacing.window_padding;
    let screen = ui.input().screen_rect;
    let size: Vec2 = ui.ctx().data(id).unwrap_or(Vec2::ZERO);
    let rect = keep_on_screen(
        Rect::from_min_size(pos, size + Vec2::splat(2.0 * pad)),
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
                content.set_in_menu(interactive.then_some(size.x));
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
    add_contents: impl FnOnce(&mut Ui<'_>) -> R,
) -> R {
    let layer = popup_layer(owner);
    let pos = anchor.left_bottom() + vec2(0.0, 4.0);
    show_floating(ui, layer, layer.id, pos, 220.0, true, add_contents)
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
        show_floating(ui, layer, id, pos, 280.0, false, |ui| {
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
