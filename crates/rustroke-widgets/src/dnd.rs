//! Dragging things between widgets: a payload of any type travels from a
//! drag source to a drop zone.

use std::any::Any;
use std::sync::Arc;

use rustroke_core::{Rect, Stroke, Vec2};

use crate::context::Order;
use crate::{CursorIcon, Id, InnerResponse, LayerId, Response, Sense, Ui};

/// Pointer movement (points) before pressing a widget inside a drag
/// source starts dragging it.
const DRAG_THRESHOLD: f32 = 4.0;

impl Ui<'_> {
    /// Content that can be dragged: pressing on it (outside its own
    /// buttons and fields) and moving starts a drag carrying `payload`.
    /// While dragged, the content follows the pointer and its place stays
    /// empty. `id` must be stable (e.g. from the item's key).
    ///
    /// ```ignore
    /// for (i, item) in items.iter().enumerate() {
    ///     ui.dnd_drag_source(Id::new(("item", item.id)), i, |ui| ui.label(&item.name));
    /// }
    /// ```
    pub fn dnd_drag_source<P: Any + Send + Sync, R>(
        &mut self,
        id: Id,
        payload: P,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let size_key = id.with("dnd size");
        let size: Vec2 = self.ctx().data(size_key).unwrap_or(Vec2::ZERO);
        let start = self.available_rect().min;
        let sense = Sense {
            click: false,
            drag: true,
            focusable: false,
            activate_with_keys: false,
        };
        let dragged = self.ctx().dnd_source().filter(|(source, _)| *source == id);
        if let Some((_, grab)) = dragged {
            // Keep the place, draw the content at the pointer.
            let place = self.allocate_rect(size);
            let mut response = self.interact(id, place, sense);
            let visuals = self.style().visuals.clone();
            self.painter().rect_stroke(
                place,
                visuals.small_corner_radius,
                Stroke::new(1.0, visuals.weak_text.with_alpha(0.5)),
            );
            let pointer = self.input().pointer.pos().unwrap_or(place.min);
            let at = Rect::from_min_size(pointer - grab, size);
            let layer = LayerId::new(Order::Tooltip, id.with("dnd"));
            let inner = {
                let mut floating = self.layer_ui(
                    layer,
                    id.with("dnd"),
                    Rect::from_min_size(at.min, Vec2::splat(f32::INFINITY)),
                );
                floating.painter().rect_filled(
                    at.expand(2.0),
                    visuals.small_corner_radius,
                    visuals.window_fill.with_alpha(0.85),
                );
                add_contents(&mut floating)
            };
            self.ctx().set_cursor(CursorIcon::Grabbing);
            self.ctx().request_repaint();
            response.rect = place;
            return InnerResponse { inner, response };
        }

        let rect = Rect::from_min_size(start, size);
        let response = self.interact(id, rect, sense);
        let content = self.scope_with(self.available_rect(), *self.layout(), add_contents);
        let used = content.response.rect;
        if used.size() != size {
            self.ctx().insert_data(size_key, used.size());
            self.ctx().request_repaint();
        }
        if response.hovered() {
            self.ctx().set_cursor(CursorIcon::Grab);
        }
        // Dragging also starts from widgets inside that don't drag
        // themselves (e.g. a card that is a button), once the pointer has
        // moved a little; sliders and text fields keep their own drags.
        let pointer = self.input().pointer.clone();
        let origin = pointer.press_origin();
        let from_inside = !response.drag_started()
            && pointer.primary_down()
            && !self.ctx().is_dnd_active()
            && !self.ctx().active_drags()
            && origin.is_some_and(|o| used.contains(o))
            && pointer
                .pos()
                .zip(origin)
                .is_some_and(|(p, o)| (p - o).length() > DRAG_THRESHOLD);
        if response.drag_started() || from_inside {
            let grab = origin.unwrap_or(used.min) - used.min;
            self.ctx().start_dnd(id, Arc::new(payload), grab);
            self.ctx().request_repaint();
        }
        let mut response = response;
        response.rect = used;
        InnerResponse {
            inner: content.inner,
            response,
        }
    }

    /// Makes an existing widget a drag source, without changing the
    /// layout (unlike [`Ui::dnd_drag_source`]): once the widget of
    /// `response` is pressed and the pointer moves a little (or its drag
    /// starts, for widgets that sense drags), a drag carrying `payload`
    /// starts, which [`Ui::dnd_drop_zone`] or [`crate::Context::take_dnd_payload`]
    /// receive. Nothing is drawn at the pointer: draw a preview if needed,
    /// e.g. while [`crate::Context::dnd_source_id`] is this widget's id.
    /// Returns whether this widget's payload is being dragged.
    ///
    /// ```ignore
    /// let response = ui.interact(id, node_rect, Sense::CLICK);
    /// ui.dnd_set_payload(&response, node.key);
    /// ```
    pub fn dnd_set_payload<P: Any + Send + Sync>(
        &mut self,
        response: &Response,
        payload: P,
    ) -> bool {
        let id = response.id;
        if self.ctx().dnd_source_id() == Some(id) {
            self.ctx().set_cursor(CursorIcon::Grabbing);
            return true;
        }
        if self.ctx().is_dnd_active() {
            return false;
        }
        let pointer = self.input().pointer.clone();
        let origin = pointer.press_origin();
        let pressed_here = self.ctx().active_id() == Some(id)
            && pointer.primary_down()
            && pointer
                .pos()
                .zip(origin)
                .is_some_and(|(p, o)| (p - o).length() > DRAG_THRESHOLD);
        if !(response.drag_started() || pressed_here) {
            return false;
        }
        let grab = origin.unwrap_or(response.rect.min) - response.rect.min;
        self.ctx().start_dnd(id, Arc::new(payload), grab);
        self.ctx().set_cursor(CursorIcon::Grabbing);
        self.ctx().request_repaint();
        true
    }

    /// An area things can be dropped on: while a `P` is dragged it is
    /// outlined (highlighted under the pointer); when it is released over
    /// it, the payload is returned and the drag ends.
    pub fn dnd_drop_zone<P: Any + Send + Sync, R>(
        &mut self,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> (InnerResponse<R>, Option<Arc<P>>) {
        let content = self.scope_with(self.available_rect(), *self.layout(), add_contents);
        let rect = content.response.rect.expand(2.0);
        let mut dropped = None;
        if self.ctx().dnd_payload::<P>().is_some() {
            let over = self.input().pointer.pos().is_some_and(|p| rect.contains(p));
            let visuals = self.style().visuals.clone();
            let stroke = if over {
                Stroke::new(2.0, visuals.accent)
            } else {
                Stroke::new(1.0, visuals.accent.with_alpha(0.4))
            };
            self.painter()
                .rect_stroke(rect, visuals.small_corner_radius, stroke);
            if over && self.input().pointer.primary_released() {
                dropped = self.ctx().take_dnd_payload::<P>();
            }
        }
        (content, dropped)
    }
}
