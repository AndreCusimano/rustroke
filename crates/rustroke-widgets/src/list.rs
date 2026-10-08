//! Lists of rows that can be selected and reordered.

use std::hash::Hash;

use rustroke_core::{Key, Modifiers, Rect, Stroke, Vec2, point, vec2};

use crate::{Align, CursorIcon, Id, Layout, Response, Sense, Ui, WidgetInfo, WidgetRole};

/// Pointer movement (points) before pressing a row becomes a drag.
const DRAG_THRESHOLD: f32 = 4.0;

/// A row being dragged, kept between frames.
#[derive(Clone, Copy, Debug)]
struct RowDrag {
    from: usize,
    /// Vertical pointer movement since the press.
    total: f32,
    moved: bool,
}

/// What [`List::show`] returns.
#[derive(Clone, Debug)]
pub struct ListResponse {
    /// Covers the whole list.
    pub response: Response,
    /// One response per row, e.g. for [`Response::context_menu`] or
    /// [`Response::on_hover_text`].
    pub rows: Vec<Response>,
    /// The row clicked this frame (with any modifiers).
    pub clicked: Option<usize>,
    /// The selection changed this frame (click, arrow keys or a move).
    pub selection_changed: bool,
    /// A row was dragged from the first index to the second this frame;
    /// the items have already been moved.
    pub moved: Option<(usize, usize)>,
}

/// Rows of your items, one under the other, that the user can select
/// (click; Cmd/Ctrl+click and Shift+click with [`List::multi_select`];
/// up/down arrows while the list has focus) and reorder by dragging (with
/// [`List::reorderable`]).
///
/// The items and the selection (indices, in any order) belong to the app;
/// the list changes them. Rows can contain other widgets, which get
/// clicks before the row does.
///
/// ```ignore
/// let list = List::new("features").reorderable(true).show(
///     ui,
///     &mut self.features,
///     &mut self.selected,
///     |ui, _index, feature| {
///         ui.label(&feature.name);
///     },
/// );
/// for (i, row) in list.rows.iter().enumerate() {
///     row.context_menu(ui, |ui| { /* actions on self.features[i] */ });
/// }
/// ```
#[derive(Clone, Debug)]
pub struct List {
    id_salt: Id,
    multi_select: bool,
    reorderable: bool,
    accessible_label: String,
}

impl List {
    /// A list identified by `id_salt` (unique within its Ui).
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            multi_select: false,
            reorderable: false,
            accessible_label: String::new(),
        }
    }

    /// Allow selecting several rows with Cmd/Ctrl+click (toggle) and
    /// Shift+click or Shift+arrows (range).
    pub fn multi_select(mut self, multi: bool) -> Self {
        self.multi_select = multi;
        self
    }

    /// Allow reordering the items by dragging rows.
    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    /// The name screen readers announce for the list.
    pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
        self.accessible_label = label.into();
        self
    }

    /// Shows one row per item, drawn by `add_row(ui, index, item)`.
    pub fn show<T>(
        self,
        ui: &mut Ui<'_>,
        items: &mut Vec<T>,
        selection: &mut Vec<usize>,
        mut add_row: impl FnMut(&mut Ui<'_>, usize, &mut T),
    ) -> ListResponse {
        let style = ui.style();
        let id = ui.id().with(self.id_salt);
        let anchor_key = id.with("anchor");
        let drag_key = id.with("drag");
        let pad = vec2(6.0, 2.0);
        let radius = style.visuals.small_corner_radius;
        // Rows span the available width; where it is unlimited (an
        // auto-width panel, a horizontal scroll area), the widest row
        // content of the last frame.
        let content_width_key = id.with("content width");
        let natural: f32 = ui
            .ctx()
            .data(content_width_key)
            .unwrap_or(style.spacing.slider_width);
        let width = ui.fill_width(natural);
        let mut widest: f32 = 0.0;
        selection.retain(|&i| i < items.len());
        let before = selection.clone();

        // The list itself takes keyboard focus (rows don't, so Tab doesn't
        // stop at every row). Registered first: rows win clicks.
        let prev_rect: Option<Rect> = ui.ctx().data(id.with("rect"));
        let start = ui.available_rect().min;
        let list_rect = prev_rect
            .map(|r| Rect::from_min_size(start, r.size()))
            .unwrap_or_else(|| Rect::from_min_size(start, Vec2::ZERO));
        let mut list_response = ui.interact(id, list_rect, Sense::CLICK);
        ui.describe(
            &list_response,
            WidgetInfo::new(WidgetRole::List, self.accessible_label.clone()),
        );

        let mut rows = Vec::with_capacity(items.len());
        let mut clicked = None;
        let row_spacing = 2.0;
        let saved_spacing = ui.style().spacing.item_spacing;
        ui.style_mut().spacing.item_spacing.y = row_spacing;
        for (index, item) in items.iter_mut().enumerate() {
            let row_id = id.with(("row", index));
            let height: f32 = ui
                .ctx()
                .data(row_id.with("height"))
                .unwrap_or(style.spacing.interact_height);
            let top = ui.available_rect().min;
            let rect = Rect::from_min_size(top, vec2(width, height));
            let sense = if self.reorderable {
                Sense::POINTER_DRAG
            } else {
                Sense {
                    drag: false,
                    ..Sense::POINTER_DRAG
                }
            };
            let response = ui.interact(row_id, rect, sense);
            let selected = selection.contains(&index);
            ui.describe(
                &response,
                WidgetInfo::new(WidgetRole::SelectableItem, format!("Row {}", index + 1))
                    .selected(selected),
            );

            // Background under the row's content.
            if selected {
                ui.painter()
                    .rect_filled(rect, radius, style.visuals.selection);
            } else if response.hovered() {
                let fill = ui.widget_visuals(&response).bg_fill;
                ui.painter().rect_filled(rect, radius, fill);
            }
            let content_rect =
                Rect::from_min_max(rect.min + pad, point(rect.max.x - pad.x, f32::INFINITY));
            let used = ui
                .push_id(("row", index), |ui| {
                    ui.scope_with(content_rect, Layout::left_to_right(Align::Center), |ui| {
                        add_row(ui, index, item)
                    })
                })
                .response
                .rect;
            let content_height = if used.is_empty() { 0.0 } else { used.height() };
            if !used.is_empty() {
                widest = widest.max(used.max.x - rect.min.x + pad.x);
            }
            let new_height = (content_height + 2.0 * pad.y).max(style.spacing.interact_height);
            if new_height != height {
                ui.ctx().insert_data(row_id.with("height"), new_height);
                ui.ctx().request_repaint();
            }
            // Reserve the rest of the row (the content took its own part).
            let below = rect.min.y + height - ui.available_rect().min.y;
            if below > 0.0 {
                ui.allocate_rect(vec2(0.0, below));
            }
            if response.clicked() {
                clicked = Some(index);
            }
            rows.push(response);
        }
        ui.style_mut().spacing.item_spacing = saved_spacing;
        if !items.is_empty() && widest != natural {
            ui.ctx().insert_data(content_width_key, widest);
            ui.ctx().request_repaint();
        }

        // Selection by clicks.
        let modifiers = ui.input().modifiers;
        let mut anchor: Option<usize> = ui.ctx().data(anchor_key);
        if let Some(index) = clicked {
            if self.multi_select && modifiers.shift {
                let from = anchor.unwrap_or(index);
                *selection = range(from, index);
            } else if self.multi_select && modifiers.command() {
                if let Some(pos) = selection.iter().position(|&i| i == index) {
                    selection.remove(pos);
                } else {
                    selection.push(index);
                }
                anchor = Some(index);
            } else {
                *selection = vec![index];
                anchor = Some(index);
            }
            ui.ctx().request_focus(id);
            list_response.has_focus = true;
        }

        // Arrow keys while the list has focus.
        if list_response.has_focus() && !items.is_empty() {
            let last = items.len() - 1;
            let current = selection.last().copied();
            let input = ui.ctx().input_mut();
            let mut target = None;
            for shift in [false, true] {
                if shift && !self.multi_select {
                    continue;
                }
                let m = if shift {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                };
                let mut step = |key, to: Option<usize>| {
                    if input.consume_key(key, m) {
                        target = Some((to, shift));
                    }
                };
                step(
                    Key::ArrowDown,
                    Some(current.map_or(0, |c| (c + 1).min(last))),
                );
                step(
                    Key::ArrowUp,
                    Some(current.map_or(0, |c| c.saturating_sub(1))),
                );
                step(Key::Home, Some(0));
                step(Key::End, Some(last));
            }
            if let Some((Some(to), shift)) = target {
                if shift {
                    *selection = range(anchor.unwrap_or(to), to);
                    // Keep the moving end last, so the next arrow continues it.
                    selection.retain(|&i| i != to);
                    selection.push(to);
                } else {
                    *selection = vec![to];
                    anchor = Some(to);
                }
            }
        }

        // Reordering by dragging a row.
        let mut moved = None;
        if self.reorderable {
            let mut drag: Option<RowDrag> = ui.ctx().data(drag_key);
            for (index, row) in rows.iter().enumerate() {
                if row.drag_started() {
                    drag = Some(RowDrag {
                        from: index,
                        total: 0.0,
                        moved: false,
                    });
                }
                if row.dragged()
                    && let Some(d) = &mut drag
                    && d.from == index
                {
                    d.total += row.drag_delta().y;
                    d.moved |= d.total.abs() >= DRAG_THRESHOLD;
                }
            }
            if let Some(d) = drag.filter(|d| d.from < rows.len()) {
                let row = &rows[d.from];
                let gap = d.moved.then(|| {
                    let y = row
                        .interact_pointer_pos()
                        .map_or(row.rect.center().y, |p| p.y);
                    rows.iter().filter(|r| r.rect.center().y < y).count()
                });
                if let Some(gap) = gap {
                    ui.ctx().set_cursor(CursorIcon::Grabbing);
                    // Where the row will go: a line between rows.
                    let y = if gap < rows.len() {
                        rows[gap].rect.min.y - row_spacing / 2.0
                    } else {
                        rows[rows.len() - 1].rect.max.y + row_spacing / 2.0
                    };
                    let x = rows[0].rect.min.x;
                    ui.painter().line(
                        point(x, y),
                        point(x + width, y),
                        Stroke::new(2.0, style.visuals.accent),
                    );
                }
                if row.drag_stopped() {
                    if let Some(gap) = gap {
                        let to = if gap > d.from { gap - 1 } else { gap };
                        if to != d.from {
                            let item = items.remove(d.from);
                            items.insert(to, item);
                            for i in selection.iter_mut() {
                                *i = moved_index(*i, d.from, to);
                            }
                            anchor = anchor.map(|a| moved_index(a, d.from, to));
                            moved = Some((d.from, to));
                        }
                    }
                    drag = None;
                }
            }
            match drag {
                Some(d) => ui.ctx().insert_data(drag_key, d),
                None => ui.ctx().remove_data(drag_key),
            }
        }

        match anchor {
            Some(a) => ui.ctx().insert_data(anchor_key, a),
            None => ui.ctx().remove_data(anchor_key),
        }
        let rect = rows.iter().fold(Rect::NOTHING, |acc, r| acc.union(r.rect));
        let rect = if rect.is_empty() {
            Rect::from_min_size(start, Vec2::ZERO)
        } else {
            rect
        };
        if prev_rect != Some(rect) {
            ui.ctx().insert_data(id.with("rect"), rect);
        }
        if list_response.focus_visible() && !rect.is_empty() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                radius + 2.0,
                Stroke::new(2.0, style.visuals.focus),
            );
        }
        list_response.rect = rect;
        let selection_changed = *selection != before;
        if selection_changed {
            list_response.mark_changed();
        }
        ListResponse {
            response: list_response,
            rows,
            clicked,
            selection_changed,
            moved,
        }
    }
}

/// The indices from `a` to `b`, inclusive, in either order.
fn range(a: usize, b: usize) -> Vec<usize> {
    (a.min(b)..=a.max(b)).collect()
}

/// Where the item at `index` ends up after moving `from` to `to`.
pub(crate) fn moved_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < index && index <= to {
        index - 1
    } else if to <= index && index < from {
        index + 1
    } else {
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_follow_a_move() {
        // [a b c d], move 0 → 2: [b c a d]
        assert_eq!(
            (0..4).map(|i| moved_index(i, 0, 2)).collect::<Vec<_>>(),
            [2, 0, 1, 3]
        );
        // move 3 → 1: [a d b c]
        assert_eq!(
            (0..4).map(|i| moved_index(i, 3, 1)).collect::<Vec<_>>(),
            [0, 2, 3, 1]
        );
    }
}
