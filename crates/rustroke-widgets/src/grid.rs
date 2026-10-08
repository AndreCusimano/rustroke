//! Tables of widgets with aligned columns.

use std::hash::Hash;

use rustroke_core::{Point, Rect, Vec2};

use crate::{Align, Id, InnerResponse, Layout, Ui};

/// Column widths and row heights measured in one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GridSizes {
    pub(crate) col_widths: Vec<f32>,
    pub(crate) row_heights: Vec<f32>,
}

impl GridSizes {
    fn grow(list: &mut Vec<f32>, index: usize, value: f32) {
        if list.len() <= index {
            list.resize(index + 1, 0.0);
        }
        list[index] = list[index].max(value);
    }
}

/// Placement state of a [`Ui`] that is laying out a [`Grid`].
#[derive(Clone, Debug)]
pub(crate) struct GridLayout {
    /// Sizes from the previous frame, used to place cells before the
    /// current frame's sizes are known.
    prev: GridSizes,
    curr: GridSizes,
    col: usize,
    row: usize,
    origin: Point,
    /// Top of the current row.
    row_top: f32,
    spacing: Vec2,
    min_row_height: f32,
    striped: bool,
}

impl GridLayout {
    /// The cell the next widget goes into. Before the column width is
    /// known (first frame) the cell extends to `max_x`.
    pub(crate) fn cell_rect(&self, max_x: f32) -> Rect {
        let width_of = |i: usize| {
            let prev = self.prev.col_widths.get(i).copied().unwrap_or(0.0);
            let curr = self.curr.col_widths.get(i).copied().unwrap_or(0.0);
            prev.max(curr)
        };
        let x = self.origin.x
            + (0..self.col)
                .map(|i| width_of(i) + self.spacing.x)
                .sum::<f32>();
        // A cell offers all the remaining width: last frame's column width
        // only positions the columns. Limiting a cell to it would freeze a
        // column at its old size (e.g. a field with a desired width added
        // to a column that only had empty cells).
        let width = (max_x - x).max(0.0);
        Rect::from_min_size(
            Point::new(x, self.row_top),
            Vec2::new(width, self.row_height_hint()),
        )
    }

    fn row_height_hint(&self) -> f32 {
        let prev = self.prev.row_heights.get(self.row).copied().unwrap_or(0.0);
        prev.max(self.min_row_height)
    }

    /// Places a widget of `size` in the current cell, vertically centered.
    pub(crate) fn place(&self, size: Vec2, max_x: f32) -> Rect {
        let cell = self.cell_rect(max_x);
        let y = cell.min.y + Align::Center.offset(cell.height(), size.y);
        Rect::from_min_size(Point::new(cell.min.x, y), size)
    }

    /// Records that `rect` filled the current cell and moves to the next one.
    pub(crate) fn advance(&mut self, rect: Rect) {
        let width = rect.max.x - self.cell_rect(f32::INFINITY).min.x;
        GridSizes::grow(&mut self.curr.col_widths, self.col, width);
        GridSizes::grow(
            &mut self.curr.row_heights,
            self.row,
            rect.height().max(self.min_row_height),
        );
        self.col += 1;
    }

    /// Moves to the start of the next row.
    pub(crate) fn end_row(&mut self) {
        let height = self.curr.row_heights.get(self.row).copied().unwrap_or(0.0);
        self.row_top += height + self.spacing.y;
        self.row += 1;
        self.col = 0;
    }

    /// Background stripe for the current row, if striping is enabled.
    pub(crate) fn stripe(&self) -> Option<Rect> {
        if !self.striped || self.row.is_multiple_of(2) || self.prev.col_widths.is_empty() {
            return None;
        }
        let width = self.prev.col_widths.iter().sum::<f32>()
            + self.spacing.x * (self.prev.col_widths.len() - 1) as f32;
        let pad = self.spacing.y / 2.0;
        Some(Rect::from_min_size(
            Point::new(self.origin.x - pad, self.row_top - pad),
            Vec2::new(width + 2.0 * pad, self.row_height_hint() + 2.0 * pad),
        ))
    }

    pub(crate) fn row(&self) -> usize {
        self.row
    }

    pub(crate) fn has_started_row(&self) -> bool {
        self.col > 0
    }
}

/// Lays out widgets in rows and columns; every column is as wide as its
/// widest cell. Call [`Ui::end_row`] after the last cell of each row.
///
/// Column widths are measured each frame and used in the next one, so a
/// grid may settle over two frames (another frame is requested
/// automatically when sizes change).
///
/// ```ignore
/// Grid::new("settings").show(ui, |ui| {
///     ui.label("Name");
///     ui.label("Value");
///     ui.end_row();
/// });
/// ```
#[derive(Clone, Debug)]
pub struct Grid {
    id: Id,
    spacing: Option<Vec2>,
    striped: bool,
}

impl Grid {
    /// `id_salt` must be unique among grids in the same [`Ui`].
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id: Id::new(id_salt),
            spacing: None,
            striped: false,
        }
    }

    /// Gaps between columns (`x`) and rows (`y`).
    pub fn spacing(mut self, spacing: Vec2) -> Self {
        self.spacing = Some(spacing);
        self
    }

    /// Shade every other row.
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Shows the grid; call `Ui::end_row` after each row in `add_contents`.
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let id = ui.id().with(self.id);
        let prev: GridSizes = ui.ctx().data(id).unwrap_or_default();
        let style = ui.style();
        let spacing = self.spacing.unwrap_or(Vec2::new(
            style.spacing.item_spacing.x * 2.0,
            style.spacing.item_spacing.y,
        ));
        let rect = ui.available_rect();
        let grid = GridLayout {
            prev: prev.clone(),
            curr: GridSizes::default(),
            col: 0,
            row: 0,
            origin: rect.min,
            row_top: rect.min.y,
            spacing,
            min_row_height: style.spacing.interact_height,
            striped: self.striped,
        };
        let stripe_color = style.visuals.stripe;

        ui.scope_with(rect, Layout::left_to_right(Align::Center), |child| {
            child.set_grid(grid, stripe_color);
            let inner = add_contents(child);
            let mut grid = child.take_grid().expect("grid set above");
            if grid.has_started_row() {
                grid.end_row();
            }
            if grid.curr != prev {
                child.ctx().request_repaint();
            }
            child.ctx().insert_data(id, grid.curr);
            inner
        })
    }
}
