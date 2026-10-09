//! Tables: a fixed header, resizable and sortable columns, and rows that
//! are only laid out while visible.

use std::hash::Hash;
use std::ops::Range;

use rustroke_core::{Color, Key, Modifiers, Rect, Stroke, Vec2, point, vec2};

use crate::containers::scroll_bar;
use crate::{Align, CursorIcon, Id, Layout, Response, Sense, Ui, WidgetInfo, WidgetRole};

/// Horizontal space between a cell's border and its content.
const CELL_PAD: f32 = 6.0;

/// Width of the strip at a column's right edge that resizes it.
const RESIZE_GRIP: f32 = 6.0;

/// A column of a [`Table`].
#[derive(Clone, Debug)]
pub struct Column {
    title: String,
    width: f32,
    min_width: f32,
    resizable: bool,
    sortable: bool,
    align: Align,
}

impl Column {
    /// A column with `title` in the header, 120 points wide.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: 120.0,
            min_width: 32.0,
            resizable: true,
            sortable: false,
            align: Align::Min,
        }
    }

    /// Width when first shown, in points.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The column can't be made narrower than this (default 32).
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Whether dragging the header's right edge resizes the column
    /// (default `true`). Double-clicking the edge fits the content.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Clicking the header sorts by this column (ascending, then
    /// descending); see [`TableResponse::sort`].
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Alignment of the cells' content: `Min` (left, default), `Center` or
    /// `Max` (right, for numbers).
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

/// The column a [`Table`] is sorted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortOrder {
    /// Index of the column.
    pub column: usize,
    /// Smallest first.
    pub ascending: bool,
}

/// What [`Table::show`] returns.
#[derive(Clone, Debug)]
pub struct TableResponse {
    /// Covers the whole table; has focus while arrow keys move the
    /// selection.
    pub response: Response,
    /// The column the user sorted by (clicking a sortable header), kept
    /// between frames. The app sorts its rows accordingly.
    pub sort: Option<SortOrder>,
    /// `sort` changed this frame.
    pub sort_changed: bool,
    /// The row clicked this frame.
    pub clicked_row: Option<usize>,
    /// The row double-clicked this frame (e.g. to open it).
    pub double_clicked_row: Option<usize>,
    /// The selection changed this frame (click or keys).
    pub selection_changed: bool,
    /// The rows laid out this frame (the visible ones).
    pub visible_rows: Range<usize>,
}

/// What is saved of a table between runs: column widths and the sort
/// order (column, ascending).
type SavedTable = (Vec<f32>, Option<(usize, bool)>);

/// Column widths, sort order and scroll position, kept between frames.
#[derive(Clone, Debug, Default, PartialEq)]
struct TableState {
    widths: Vec<f32>,
    /// Widest content of each column in the last frame (for fitting).
    content_widths: Vec<f32>,
    sort: Option<SortOrder>,
    offset: Vec2,
}

/// A table of rows and columns: a header that stays at the top, columns
/// that can be resized (drag the header's edge; double-click it to fit the
/// content) and sorted (click the header), the first columns optionally
/// fixed while the rest scroll sideways, striped rows, and a selection
/// moved with clicks and the arrow keys.
///
/// Only the visible rows are laid out, so tables of millions of rows stay
/// fast. Cells hold any widgets (labels, text fields to edit values,
/// checkboxes...). Rows can have different heights
/// ([`Table::show_with_heights`]), e.g. to expand a row with details.
///
/// ```ignore
/// let table = Table::new("parts")
///     .column(Column::new("Name").width(160.0).sortable(true))
///     .column(Column::new("Qty").width(60.0).align(Align::Max).sortable(true))
///     .show(ui, parts.len(), &mut selected, |ui, row, col| match col {
///         0 => { ui.label(&parts[row].name); }
///         _ => { ui.label(parts[row].qty.to_string()); }
///     });
/// if table.sort_changed { /* sort `parts` by table.sort */ }
/// ```
#[derive(Clone, Debug)]
pub struct Table {
    id_salt: Id,
    columns: Vec<Column>,
    row_height: Option<f32>,
    striped: bool,
    max_height: f32,
    sticky_columns: usize,
    accessible_label: String,
    scroll_to_row: Option<usize>,
}

impl Table {
    /// A table identified by `id_salt` (unique within its Ui).
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            columns: Vec::new(),
            row_height: None,
            striped: true,
            max_height: f32::INFINITY,
            sticky_columns: 0,
            accessible_label: String::new(),
            scroll_to_row: None,
        }
    }

    /// Adds a column.
    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    /// Height of every row in points (default: the style's
    /// `interact_height`).
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Every other row has a slightly different background (default
    /// `true`).
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// The table is at most this tall, header included (it is also
    /// limited by the available height).
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// The first `count` columns stay in place while the others scroll
    /// sideways.
    pub fn sticky_columns(mut self, count: usize) -> Self {
        self.sticky_columns = count;
        self
    }

    /// The name screen readers announce for the table.
    pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
        self.accessible_label = label.into();
        self
    }

    /// Scrolls so that `row` is visible (e.g. after selecting it from
    /// elsewhere).
    pub fn scroll_to_row(mut self, row: usize) -> Self {
        self.scroll_to_row = Some(row);
        self
    }

    /// Shows `rows` rows of equal height; `add_cell(ui, row, column)`
    /// fills each visible cell. `selection` is the selected row, changed
    /// by clicks and the arrow keys.
    pub fn show(
        self,
        ui: &mut Ui<'_>,
        rows: usize,
        selection: &mut Option<usize>,
        add_cell: impl FnMut(&mut Ui<'_>, usize, usize),
    ) -> TableResponse {
        let height = self
            .row_height
            .unwrap_or(ui.style().spacing.interact_height);
        self.show_impl(ui, rows, &|_| height, Some(height), selection, add_cell)
    }

    /// Like [`Table::show`], with the height of each row given by
    /// `row_height(row)` (e.g. taller for expanded rows). It is called for
    /// every row each frame, so keep it cheap.
    pub fn show_with_heights(
        self,
        ui: &mut Ui<'_>,
        rows: usize,
        row_height: impl Fn(usize) -> f32,
        selection: &mut Option<usize>,
        add_cell: impl FnMut(&mut Ui<'_>, usize, usize),
    ) -> TableResponse {
        self.show_impl(ui, rows, &row_height, None, selection, add_cell)
    }

    fn show_impl(
        self,
        ui: &mut Ui<'_>,
        rows: usize,
        row_height: &dyn Fn(usize) -> f32,
        uniform: Option<f32>,
        selection: &mut Option<usize>,
        mut add_cell: impl FnMut(&mut Ui<'_>, usize, usize),
    ) -> TableResponse {
        let style = ui.style();
        let visuals = &style.visuals;
        let id = ui.id().with(self.id_salt);
        let columns = &self.columns;
        let ncols = columns.len();
        let sticky = self.sticky_columns.min(ncols);
        let bar = style.spacing.scrollbar_width;

        // Column widths and sort order survive restarts (with the
        // `persistence` feature).
        let saved_key = id.with("saved");
        let mut state: TableState = match ui.ctx().data(id) {
            Some(state) => state,
            None => {
                let saved: Option<SavedTable> = ui.ctx().data_persisted(saved_key);
                let (widths, sort) = saved.unwrap_or_default();
                TableState {
                    widths,
                    sort: sort.map(|(column, ascending)| SortOrder { column, ascending }),
                    ..TableState::default()
                }
            }
        };
        if state.widths.len() != ncols {
            state.widths = columns.iter().map(|c| c.width).collect();
        }
        state.content_widths.resize(ncols, 0.0);
        let old_state = state.clone();
        if selection.is_some_and(|s| s >= rows) {
            *selection = None;
        }
        let before = *selection;

        // Row positions: where each row starts, from the top of the body
        // (computed only when heights differ).
        let starts: Vec<f32> = if uniform.is_some() {
            Vec::new()
        } else {
            let mut y = 0.0;
            (0..rows)
                .map(|i| {
                    let top = y;
                    y += row_height(i);
                    top
                })
                .collect()
        };
        let row_top = |i: usize| match uniform {
            Some(h) => i as f32 * h,
            None => starts.get(i).copied().unwrap_or(0.0),
        };
        let total_height = match uniform {
            Some(h) => rows as f32 * h,
            None => starts.last().map_or(0.0, |&t| t + row_height(rows - 1)),
        };
        let row_at = |y: f32| -> usize {
            match uniform {
                Some(h) if h > 0.0 => ((y / h).floor().max(0.0) as usize).min(rows),
                Some(_) => 0,
                None => starts.partition_point(|&t| t <= y).saturating_sub(1),
            }
        };

        // Overall size.
        let header_h = style.spacing.interact_height;
        let col_x: Vec<f32> = state
            .widths
            .iter()
            .scan(0.0, |x, w| {
                let left = *x;
                *x += w;
                Some(left)
            })
            .collect();
        let total_width: f32 = state.widths.iter().sum();
        let sticky_width: f32 = state.widths[..sticky].iter().sum();
        let available = ui.available_rect();
        let max_h = self.max_height.min(available.height()).max(header_h);
        let width = ui.fill_width(total_width + bar).min(available.width());
        let needs_y = header_h + total_height > max_h;
        let body_w = width - if needs_y { bar } else { 0.0 };
        let needs_x = total_width > body_w + 0.5;
        let hbar_h = if needs_x { bar } else { 0.0 };
        let height = (header_h + total_height + hbar_h).min(max_h);
        let outer = ui.allocate_rect(vec2(width, height));
        let header = Rect::from_min_size(outer.min, vec2(body_w, header_h));
        let body = Rect::from_min_max(
            point(outer.min.x, header.max.y),
            point(outer.min.x + body_w, outer.max.y - hbar_h),
        );
        // Region of the scrolling (non-sticky) columns.
        let scroll_region = Rect::from_min_max(
            point((body.min.x + sticky_width).min(body.max.x), outer.min.y),
            point(body.max.x, body.max.y),
        );
        let max_offset = vec2(
            (total_width - body.width()).max(0.0),
            (total_height - body.height()).max(0.0),
        );
        let clamp = |o: Vec2| vec2(o.x.clamp(0.0, max_offset.x), o.y.clamp(0.0, max_offset.y));
        state.offset = clamp(state.offset);

        // The table takes keyboard focus; registered first so rows and
        // cells win clicks.
        let mut table_response = ui.interact(id, outer, Sense::CLICK);
        ui.describe(
            &table_response,
            WidgetInfo::new(WidgetRole::List, self.accessible_label.clone()),
        );

        // Wheel.
        let wheel = ui.interact(id.with("wheel"), outer, Sense::HOVER);
        if wheel.hovered() {
            let mut delta = ui.input().scroll_delta;
            if ui.input().modifiers.shift && delta.x == 0.0 {
                delta = vec2(delta.y, 0.0);
            }
            let next = clamp(state.offset - delta);
            let used = state.offset - next;
            state.offset = next;
            let input = ui.ctx().input_mut();
            if used.x != 0.0 {
                input.scroll_delta.x = 0.0;
            }
            if used.y != 0.0 {
                input.scroll_delta.y = 0.0;
            }
        }

        // Keyboard: move the selection and keep it visible.
        let mut reveal = self.scroll_to_row;
        if table_response.has_focus() && rows > 0 {
            let page = row_at(body.height()).max(1);
            let current = *selection;
            let input = ui.ctx().input_mut();
            let mut to = None;
            let mut step = |key, target: usize| {
                if input.consume_key(key, Modifiers::NONE) {
                    to = Some(target);
                }
            };
            step(Key::ArrowDown, current.map_or(0, |c| (c + 1).min(rows - 1)));
            step(Key::ArrowUp, current.map_or(0, |c| c.saturating_sub(1)));
            step(
                Key::PageDown,
                current.map_or(0, |c| (c + page).min(rows - 1)),
            );
            step(Key::PageUp, current.map_or(0, |c| c.saturating_sub(page)));
            step(Key::Home, 0);
            step(Key::End, rows - 1);
            if let Some(to) = to {
                *selection = Some(to);
                reveal = Some(to);
            }
        }
        if let Some(row) = reveal.filter(|&r| r < rows) {
            let (top, bottom) = (row_top(row), row_top(row) + row_height(row));
            if top < state.offset.y {
                state.offset.y = top;
            } else if bottom > state.offset.y + body.height() {
                state.offset.y = bottom - body.height();
            }
            state.offset = clamp(state.offset);
        }

        // Column positions on screen.
        let cell_x = |c: usize, offset_x: f32| {
            let x = body.min.x + col_x[c];
            if c < sticky { x } else { x - offset_x }
        };
        let clip_for = |c: usize| {
            if c < sticky {
                Rect::from_min_max(body.min, point(scroll_region.min.x, body.max.y))
            } else {
                Rect::from_min_max(point(scroll_region.min.x, body.min.y), body.max)
            }
        };

        // Body.
        let first = row_at(state.offset.y);
        let last = (row_at(state.offset.y + body.height()) + 1).min(rows);
        let saved_clip = ui.clip_rect();
        let mut clicked_row = None;
        let mut double_clicked_row = None;
        let mut widest = vec![0.0_f32; ncols];
        for row in first..last {
            let y = body.min.y + row_top(row) - state.offset.y;
            let rect =
                Rect::from_min_size(point(body.min.x, y), vec2(body.width(), row_height(row)));
            ui.clip_rect_restore(saved_clip);
            ui.set_clip_rect(body);
            let row_id = id.with(("row", row));
            let sense = Sense {
                focusable: false,
                activate_with_keys: false,
                ..Sense::CLICK
            };
            let response = ui.interact(row_id, rect, sense);
            let selected = *selection == Some(row);
            ui.describe(
                &response,
                WidgetInfo::new(WidgetRole::SelectableItem, format!("Row {}", row + 1))
                    .selected(selected),
            );
            let fill = if selected {
                visuals.selection
            } else if response.hovered() {
                ui.widget_visuals(&response).bg_fill.with_alpha(0.5)
            } else if self.striped && row % 2 == 1 {
                visuals.stripe
            } else {
                Color::TRANSPARENT
            };
            if fill.a > 0.0 {
                ui.painter().rect_filled(rect, 0.0, fill);
            }
            if response.clicked() {
                clicked_row = Some(row);
            }
            if response.double_clicked() {
                double_clicked_row = Some(row);
            }
            for (c, column) in columns.iter().enumerate() {
                let x = cell_x(c, state.offset.x);
                let cell = Rect::from_min_size(point(x, y), vec2(state.widths[c], rect.height()));
                let clip = saved_clip.intersect(clip_for(c)).intersect(cell);
                if clip.is_empty() {
                    continue;
                }
                ui.clip_rect_restore(saved_clip);
                ui.set_clip_rect(clip);
                let content = Rect::from_min_max(
                    point(cell.min.x + CELL_PAD, cell.min.y),
                    point(cell.max.x - CELL_PAD, cell.max.y),
                );
                let layout = match column.align {
                    Align::Max => Layout::right_to_left(Align::Center),
                    _ => Layout::left_to_right(Align::Center),
                };
                let used = ui
                    .push_id(("cell", row, c), |ui| {
                        ui.scope_with_no_advance(content, layout, |ui| add_cell(ui, row, c))
                    })
                    .response
                    .rect;
                if !used.is_empty() {
                    widest[c] = widest[c].max(used.width() + 2.0 * CELL_PAD);
                }
            }
        }
        ui.clip_rect_restore(saved_clip);
        for (c, w) in widest.into_iter().enumerate() {
            if w > 0.0 {
                state.content_widths[c] = w;
            }
        }

        // Header, over the body.
        ui.set_clip_rect(header);
        ui.painter().rect_filled(header, 0.0, visuals.panel_fill);
        let mut sort_changed = false;
        for (c, column) in columns.iter().enumerate() {
            let x = cell_x(c, state.offset.x);
            let cell = Rect::from_min_size(point(x, header.min.y), vec2(state.widths[c], header_h));
            let region = if c < sticky {
                Rect::from_min_max(header.min, point(scroll_region.min.x, header.max.y))
            } else {
                Rect::from_min_max(point(scroll_region.min.x, header.min.y), header.max)
            };
            let clip = saved_clip
                .intersect(region)
                .intersect(cell.expand(RESIZE_GRIP / 2.0));
            if clip.is_empty() {
                continue;
            }
            ui.clip_rect_restore(saved_clip);
            ui.set_clip_rect(clip);
            let head_id = id.with(("header", c));
            let head = if column.sortable {
                let sense = Sense {
                    focusable: false,
                    activate_with_keys: false,
                    ..Sense::CLICK
                };
                ui.interact(head_id, cell, sense)
            } else {
                ui.interact(head_id, cell, Sense::HOVER)
            };
            if column.sortable && head.hovered() {
                let fill = ui.widget_visuals(&head).bg_fill.with_alpha(0.5);
                ui.painter().rect_filled(cell, 0.0, fill);
            }
            if column.sortable && head.clicked() {
                state.sort = Some(match state.sort {
                    Some(s) if s.column == c => SortOrder {
                        column: c,
                        ascending: !s.ascending,
                    },
                    _ => SortOrder {
                        column: c,
                        ascending: true,
                    },
                });
                sort_changed = true;
            }
            let title_style = style.body.clone().weight(600);
            let galley = ui.layout_text(&column.title, &title_style, None);
            let arrow_room = if column.sortable { 14.0 } else { 0.0 };
            let text_x = match column.align {
                Align::Max => cell.max.x - CELL_PAD - arrow_room - galley.size.x,
                Align::Center => cell.center().x - galley.size.x / 2.0,
                Align::Min => cell.min.x + CELL_PAD,
            };
            let title_width = galley.size.x + 2.0 * CELL_PAD + arrow_room;
            state.content_widths[c] = state.content_widths[c].max(title_width);
            ui.painter().galley(
                point(text_x, cell.center().y - galley.size.y / 2.0),
                galley.clone(),
                visuals.text,
            );
            if let Some(sort) = state.sort.filter(|s| s.column == c) {
                let cx = match column.align {
                    Align::Max => cell.max.x - CELL_PAD - 5.0,
                    _ => text_x + galley.size.x + 9.0,
                };
                let cy = cell.center().y;
                let d = if sort.ascending { -1.0 } else { 1.0 };
                ui.painter().polyline(
                    vec![
                        point(cx - 4.0, cy - 2.0 * d),
                        point(cx, cy + 2.0 * d),
                        point(cx + 4.0, cy - 2.0 * d),
                    ],
                    Stroke::new(1.5, visuals.weak_text),
                );
            }
            // Separator and resize grip at the right edge.
            ui.painter().line(
                point(cell.max.x, cell.min.y + 5.0),
                point(cell.max.x, cell.max.y - 5.0),
                visuals.window_stroke,
            );
        }
        // Resize grips, after all headers so they win over the next
        // column's header.
        for (c, column) in columns.iter().enumerate() {
            if !column.resizable {
                continue;
            }
            let edge = cell_x(c, state.offset.x) + state.widths[c];
            ui.clip_rect_restore(saved_clip);
            if c >= sticky {
                ui.set_clip_rect(Rect::from_min_max(
                    point(scroll_region.min.x, header.min.y),
                    header.max,
                ));
            } else {
                ui.set_clip_rect(header);
            }
            let grip =
                Rect::from_center_size(point(edge, header.center().y), vec2(RESIZE_GRIP, header_h));
            let r = ui.interact(id.with(("resize", c)), grip, Sense::POINTER_DRAG);
            if r.hovered() || r.dragged() {
                ui.ctx().set_cursor(CursorIcon::ResizeHorizontal);
            }
            if r.dragged() {
                state.widths[c] = (state.widths[c] + r.drag_delta().x).max(column.min_width);
            }
            if r.double_clicked() {
                state.widths[c] = state.content_widths[c].max(column.min_width);
            }
        }
        ui.clip_rect_restore(saved_clip);
        ui.painter().line(
            point(outer.min.x, header.max.y),
            point(outer.max.x, header.max.y),
            visuals.window_stroke,
        );
        if sticky > 0 && needs_x {
            let x = scroll_region.min.x;
            ui.painter().line(
                point(x, outer.min.y),
                point(x, body.max.y),
                visuals.window_stroke,
            );
        }

        // Scroll bars.
        if needs_y {
            let track = Rect::from_min_max(
                point(body.max.x, body.min.y),
                point(outer.max.x, body.max.y),
            );
            state.offset.y = scroll_bar(
                ui,
                id.with("vbar"),
                track,
                1,
                body.height(),
                total_height,
                state.offset.y,
                max_offset.y,
                None,
            );
        }
        if needs_x {
            let track = Rect::from_min_max(
                point(scroll_region.min.x, body.max.y),
                point(body.max.x, outer.max.y),
            );
            state.offset.x = scroll_bar(
                ui,
                id.with("hbar"),
                track,
                0,
                body.width() - sticky_width,
                total_width - sticky_width,
                state.offset.x,
                max_offset.x,
                None,
            );
        }
        state.offset = clamp(state.offset);

        // Selection by clicks.
        if let Some(row) = clicked_row {
            *selection = Some(row);
            ui.ctx().request_focus(id);
            table_response.has_focus = true;
        }
        if table_response.focus_visible() {
            ui.painter().rect_stroke(
                outer.expand(2.0),
                visuals.small_corner_radius + 2.0,
                Stroke::new(2.0, visuals.focus),
            );
        }

        if state != old_state {
            if state.widths != old_state.widths || state.sort != old_state.sort {
                let sort = state.sort.map(|s| (s.column, s.ascending));
                ui.ctx()
                    .insert_persisted(saved_key, (state.widths.clone(), sort));
            }
            ui.ctx().insert_data(id, state.clone());
            ui.ctx().request_repaint();
        }
        let selection_changed = *selection != before;
        if selection_changed {
            table_response.mark_changed();
        }
        TableResponse {
            response: table_response,
            sort: state.sort,
            sort_changed,
            clicked_row,
            double_clicked_row,
            selection_changed,
            visible_rows: first..last,
        }
    }
}
