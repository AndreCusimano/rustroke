//! [`Grid`]: widgets in rows and columns, in a box with an optional
//! background, border and size.

use std::hash::Hash;

use rustroke_core::{Vec2, vec2};

use crate::container::{BoxArea, box_setters};
use crate::slots::{GridParams, GridSlots, Measured, SlotKind, Slots, layout_grid};
use crate::{Align, BoxStyle, Id, InnerResponse, Layout, StackAlign, Ui};

/// The width of a [`Grid`] column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    /// Fixed, in points.
    Points(f32),
    /// A share of the free width (`1fr`); the grid then takes all the
    /// available width.
    Fraction(f32),
    /// As wide as its widest cell.
    Auto,
}

impl Track {
    pub(crate) fn taffy(self) -> taffy::TrackSizingFunction {
        match self {
            Self::Points(p) => taffy::prelude::length(p),
            Self::Fraction(f) => taffy::prelude::fr(f),
            Self::Auto => taffy::prelude::auto(),
        }
    }
}

/// A cell of a [`Grid`]: column and row (from 0) and how many it spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GridCell {
    /// First column.
    pub column: u16,
    /// First row.
    pub row: u16,
    /// Columns covered (at least 1).
    pub column_span: u16,
    /// Rows covered (at least 1).
    pub row_span: u16,
}

impl GridCell {
    /// The cell at `column`, `row`.
    pub fn at(column: u16, row: u16) -> Self {
        Self {
            column,
            row,
            column_span: 1,
            row_span: 1,
        }
    }

    /// Spanning `columns` columns and `rows` rows.
    pub fn span(mut self, columns: u16, rows: u16) -> Self {
        self.column_span = columns.max(1);
        self.row_span = rows.max(1);
        self
    }
}

/// Measured cells of a grid, kept for the next frame.
#[derive(Clone, Debug, Default, PartialEq)]
struct GridMemory {
    items: Vec<Measured>,
    /// Top and bottom of each row, and the grid's width (for stripes).
    rows: Vec<(f32, f32)>,
    width: f32,
}

/// Widgets in rows and columns, in a box that can have a background,
/// border, padding, margins and a size (see [`crate::Stack`] for the box
/// options, which are the same).
///
/// Each widget fills the next cell; call [`Ui::end_row`] after the last
/// cell of a row. Columns are as wide as their widest cell unless set
/// with [`Grid::columns`] (fixed points, shares of the free width, or
/// content). [`Ui::grid_span`] makes a cell cover several columns or
/// rows, [`Ui::grid_cell`] puts one in a given place.
///
/// ```ignore
/// Grid::new("form").columns(vec![Track::Points(100.0), Track::Fraction(1.0)]).show(ui, |ui| {
///     ui.label("Name");
///     ui.text_edit_singleline(&mut name);
///     ui.end_row();
///     ui.grid_span(2, 1, |ui| ui.button("Save"));
///     ui.end_row();
/// });
/// ```
///
/// Column widths are measured each frame and used in the next one, so a
/// grid may settle over two frames (another frame is requested
/// automatically when sizes change).
#[derive(Clone, Debug)]
pub struct Grid {
    id: Id,
    spacing: Option<Vec2>,
    striped: bool,
    columns: Vec<Track>,
    align: [StackAlign; 2],
    box_style: BoxStyle,
}

impl Grid {
    /// `id_salt` must be unique among grids in the same [`Ui`].
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id: Id::new(id_salt),
            spacing: None,
            striped: false,
            columns: Vec::new(),
            align: [StackAlign::Start, StackAlign::Center],
            box_style: BoxStyle::new(),
        }
    }

    /// Gaps between columns (`x`) and rows (`y`).
    pub fn spacing(mut self, spacing: Vec2) -> Self {
        self.spacing = Some(spacing);
        self
    }

    /// The same gap between columns and between rows.
    pub fn gap(self, gap: f32) -> Self {
        self.spacing(Vec2::splat(gap))
    }

    /// Shade every other row.
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Column widths; columns beyond the list are as wide as their
    /// content.
    pub fn columns(mut self, columns: impl IntoIterator<Item = Track>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    /// Horizontal position of each widget in its cell (default `Start`;
    /// `Stretch` fills the cell).
    pub fn align_x(mut self, align: StackAlign) -> Self {
        self.align[0] = align;
        self
    }

    /// Vertical position of each widget in its cell (default `Center`).
    pub fn align_y(mut self, align: StackAlign) -> Self {
        self.align[1] = align;
        self
    }

    /// The box's look and size from a [`BoxStyle`].
    pub fn style(mut self, style: BoxStyle) -> Self {
        self.box_style = style;
        self
    }

    box_setters!(box_style);

    /// Shows the grid; call `Ui::end_row` after each row in `add_contents`.
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let id = ui.id().with(self.id);
        let style = ui.style();
        let gap = self.spacing.unwrap_or(vec2(
            style.spacing.item_spacing.x * 2.0,
            style.spacing.item_spacing.y,
        ));
        let min_row_height = style.spacing.interact_height;
        let stripe = self.striped.then_some(style.visuals.stripe);
        let Self { columns, align, .. } = self;
        self.box_style.show(ui, |outer, area: BoxArea| {
            let memory: GridMemory = outer.ctx().data(id).unwrap_or_default();
            let params = GridParams {
                tracks: &columns,
                gap,
                min_row_height,
                fixed: area.fixed,
                room: area.room,
            };
            let (rects, total) = layout_grid(&memory.items, &params);
            let rects = rects
                .into_iter()
                .map(|r| rustroke_core::Rect::from_min_size(area.min + r.min.to_vec2(), r.size()))
                .collect();
            let mut grid = GridSlots::new(columns.clone(), area.min.y, min_row_height);
            grid.stripe = stripe;
            grid.rows = memory.rows.clone();
            grid.width = memory.width;
            let slots = Slots::new(SlotKind::Grid(grid), rects, area.rect(), align, gap);
            let layout = Layout::left_to_right(Align::Center);
            let (inner, mut slots, used) =
                outer.slot_child(id.with("content"), area.rect(), layout, slots, add_contents);
            slots.end_row_if_started();

            // Row extents for next frame's stripes, from this frame's
            // placement of single-row cells.
            let mut rows: Vec<(f32, f32)> = Vec::new();
            let laid: Vec<rustroke_core::Rect> = slots.rects.clone();
            for (i, m) in slots.items.iter().enumerate() {
                let Some(rect) = laid.get(i) else { continue };
                if m.cell.row_span != 1 {
                    continue;
                }
                let r = m.cell.row as usize;
                if rows.len() <= r {
                    rows.resize(r + 1, (f32::INFINITY, f32::NEG_INFINITY));
                }
                rows[r] = (rows[r].0.min(rect.min.y), rows[r].1.max(rect.max.y));
            }
            let used_size = if used.is_empty() || slots.items.is_empty() {
                Vec2::ZERO
            } else {
                used.max - area.min
            };
            let content = vec2(
                area.fixed[0].unwrap_or(total.x.max(used_size.x)),
                area.fixed[1].unwrap_or(total.y.max(used_size.y)),
            );
            let new_memory = GridMemory {
                items: slots.items,
                rows,
                width: total.x,
            };
            // Stripes follow the rows a frame late; only changed cell
            // sizes need another frame.
            if new_memory.items != memory.items {
                outer.ctx().request_repaint();
            }
            if new_memory != memory {
                outer.ctx().insert_data(id, new_memory);
            }
            (inner, content)
        })
    }
}
