//! Flexbox and CSS-grid style layouts, computed with taffy.
//!
//! Like [`crate::Grid`], they use the sizes their items had in the
//! previous frame, so a layout settles in two frames.

use std::hash::Hash;

use rustroke_core::{Rect, Vec2, point, vec2};
use taffy::prelude::{
    AvailableSpace, Dimension, LengthPercentage, LengthPercentageAuto, NodeId, Size, TaffyTree,
};

use crate::{Align, Id, InnerResponse, Layout, Ui};

/// Which way a [`Flex`] container lays out its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlexDirection {
    /// Left to right.
    Row,
    /// Top to bottom.
    Column,
}

/// How items share the free space along the main axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlexJustify {
    /// Packed at the start.
    Start,
    /// Packed at the end.
    End,
    /// Packed in the middle.
    Center,
    /// First and last at the edges, equal space between the others.
    SpaceBetween,
    /// Equal space around each item.
    SpaceAround,
    /// Equal space between items and at the edges.
    SpaceEvenly,
}

/// Where items sit across the main axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlexAlign {
    /// At the start (top of a row).
    Start,
    /// At the end.
    End,
    /// In the middle.
    Center,
    /// Stretched to the line's size.
    Stretch,
}

impl FlexAlign {
    fn taffy(self) -> taffy::AlignItems {
        match self {
            Self::Start => taffy::AlignItems::FLEX_START,
            Self::End => taffy::AlignItems::FLEX_END,
            Self::Center => taffy::AlignItems::CENTER,
            Self::Stretch => taffy::AlignItems::STRETCH,
        }
    }
}

/// How one item of a [`Flex`] container grows, shrinks and aligns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexItem {
    grow: f32,
    shrink: f32,
    basis: Option<f32>,
    align: Option<FlexAlign>,
}

impl Default for FlexItem {
    fn default() -> Self {
        Self::new()
    }
}

impl FlexItem {
    /// Its natural size: no growing, shrinking when there is not enough
    /// room.
    pub fn new() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: None,
            align: None,
        }
    }

    /// Takes this share of the free space (e.g. 1 for every item that
    /// should fill a row).
    pub fn grow(mut self, grow: f32) -> Self {
        self.grow = grow;
        self
    }

    /// Gives up this share of missing space (0: never smaller than its
    /// content).
    pub fn shrink(mut self, shrink: f32) -> Self {
        self.shrink = shrink;
        self
    }

    /// Starting size along the main axis, in points, instead of its
    /// content's size.
    pub fn basis(mut self, basis: f32) -> Self {
        self.basis = Some(basis);
        self
    }

    /// Its own cross-axis alignment.
    pub fn align(mut self, align: FlexAlign) -> Self {
        self.align = Some(align);
        self
    }
}

/// Item sizes and the container size from the previous frame.
#[derive(Clone, Debug, Default, PartialEq)]
struct FlexMemory {
    items: Vec<(FlexItem, Vec2)>,
}

/// A flexbox: items in a row or column that wrap, grow to fill the free
/// space and are aligned like CSS flexbox (computed with taffy).
///
/// ```ignore
/// Flex::row("toolbar").gap(8.0).align(FlexAlign::Center).show(ui, |flex| {
///     flex.add(FlexItem::new(), |ui| ui.button("Open"));
///     flex.add(FlexItem::new().grow(1.0), |ui| ui.text_edit_singleline(&mut search));
///     flex.add(FlexItem::new(), |ui| ui.button("Go"));
/// });
/// ```
#[derive(Clone, Debug)]
pub struct Flex {
    id_salt: Id,
    direction: FlexDirection,
    wrap: bool,
    gap: Vec2,
    justify: FlexJustify,
    align: FlexAlign,
}

impl Flex {
    fn new(id_salt: impl Hash, direction: FlexDirection) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            direction,
            wrap: false,
            gap: Vec2::splat(8.0),
            justify: FlexJustify::Start,
            align: FlexAlign::Start,
        }
    }

    /// Items left to right (identified by `id_salt`, unique in its Ui).
    pub fn row(id_salt: impl Hash) -> Self {
        Self::new(id_salt, FlexDirection::Row)
    }

    /// Items top to bottom.
    pub fn column(id_salt: impl Hash) -> Self {
        Self::new(id_salt, FlexDirection::Column)
    }

    /// Items that don't fit move to a new line.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Space between items and lines, in points (default 8).
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Vec2::splat(gap);
        self
    }

    /// How items share the free space along the main axis.
    pub fn justify(mut self, justify: FlexJustify) -> Self {
        self.justify = justify;
        self
    }

    /// Where items sit across the main axis (default `Start`).
    pub fn align(mut self, align: FlexAlign) -> Self {
        self.align = align;
        self
    }

    /// Lays out the items added by `add_items`.
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_items: impl FnOnce(&mut FlexUi<'_, '_>) -> R,
    ) -> InnerResponse<R> {
        let id = ui.id().with(self.id_salt);
        let memory: FlexMemory = ui.ctx().data(id).unwrap_or_default();
        let available = ui.available_rect();
        let row = self.direction == FlexDirection::Row;

        let mut tree: TaffyTree<()> = TaffyTree::new();
        let children: Vec<NodeId> = memory
            .items
            .iter()
            .map(|(item, size)| {
                let style = taffy::Style {
                    size: Size {
                        width: Dimension::length(size.x),
                        height: Dimension::length(size.y),
                    },
                    flex_grow: item.grow,
                    flex_shrink: item.shrink,
                    flex_basis: item.basis.map_or(Dimension::auto(), Dimension::length),
                    min_size: Size {
                        width: LengthPercentageAuto::length(0.0),
                        height: LengthPercentageAuto::length(0.0),
                    },
                    align_self: item.align.map(FlexAlign::taffy),
                    ..Default::default()
                };
                tree.new_leaf(style).expect("leaf")
            })
            .collect();
        let width = available.width();
        let height = available.height();
        let container = taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: if row {
                taffy::FlexDirection::Row
            } else {
                taffy::FlexDirection::Column
            },
            flex_wrap: if self.wrap {
                taffy::FlexWrap::Wrap
            } else {
                taffy::FlexWrap::NoWrap
            },
            gap: Size {
                width: LengthPercentage::length(self.gap.x),
                height: LengthPercentage::length(self.gap.y),
            },
            justify_content: Some(match self.justify {
                FlexJustify::Start => taffy::JustifyContent::FLEX_START,
                FlexJustify::End => taffy::JustifyContent::FLEX_END,
                FlexJustify::Center => taffy::JustifyContent::CENTER,
                FlexJustify::SpaceBetween => taffy::JustifyContent::SPACE_BETWEEN,
                FlexJustify::SpaceAround => taffy::JustifyContent::SPACE_AROUND,
                FlexJustify::SpaceEvenly => taffy::JustifyContent::SPACE_EVENLY,
            }),
            align_items: Some(self.align.taffy()),
            size: Size {
                width: if row && width.is_finite() {
                    Dimension::length(width)
                } else {
                    Dimension::auto()
                },
                height: if !row
                    && height.is_finite()
                    && memory.items.iter().any(|(i, _)| i.grow > 0.0)
                {
                    Dimension::length(height)
                } else {
                    Dimension::auto()
                },
            },
            ..Default::default()
        };
        let root = tree.new_with_children(container, &children).expect("root");
        let space = Size {
            width: if width.is_finite() {
                AvailableSpace::Definite(width)
            } else {
                AvailableSpace::MaxContent
            },
            height: AvailableSpace::MaxContent,
        };
        let _ = tree.compute_layout(root, space);
        let rects: Vec<Rect> = children
            .iter()
            .filter_map(|n| tree.layout(*n).ok())
            .map(|l| {
                Rect::from_min_size(
                    available.min + vec2(l.location.x, l.location.y),
                    vec2(l.size.width, l.size.height),
                )
            })
            .collect();
        let total = tree
            .layout(root)
            .map_or(Vec2::ZERO, |l| vec2(l.size.width, l.size.height));

        let mut flex = FlexUi {
            ui,
            id,
            rects,
            items: Vec::new(),
            used: Rect::NOTHING,
            available,
            row,
            gap: self.gap,
        };
        let inner = add_items(&mut flex);
        let FlexUi {
            ui, items, used, ..
        } = flex;
        let new_memory = FlexMemory { items };
        if new_memory != memory {
            ui.ctx().insert_data(id, new_memory);
            ui.ctx().request_repaint();
        }
        let size = vec2(
            total.x.max(used.max.x - available.min.x).max(0.0),
            total.y.max(used.max.y - available.min.y).max(0.0),
        );
        let rect = ui.allocate_rect(size);
        let response = ui.interact(id, rect, crate::Sense::HOVER);
        InnerResponse { inner, response }
    }
}

/// Adds items to a [`Flex`] (or [`FlexGrid`]) container.
pub struct FlexUi<'u, 'a> {
    ui: &'u mut Ui<'a>,
    id: Id,
    /// Where each item goes, from last frame's sizes.
    rects: Vec<Rect>,
    /// This frame's items with their measured content sizes.
    items: Vec<(FlexItem, Vec2)>,
    used: Rect,
    available: Rect,
    row: bool,
    gap: Vec2,
}

impl std::fmt::Debug for FlexUi<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlexUi")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl FlexUi<'_, '_> {
    /// Adds an item: `add_contents` lays it out in its place, top to
    /// bottom. Items stretched by the layout get a bigger area.
    pub fn add<R>(
        &mut self,
        item: FlexItem,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let index = self.items.len();
        // New items (no size yet) go after the others until measured.
        let rect = self.rects.get(index).copied().unwrap_or_else(|| {
            let start = if self.used == Rect::NOTHING {
                self.available.min
            } else if self.row {
                point(self.used.max.x + self.gap.x, self.available.min.y)
            } else {
                point(self.available.min.x, self.used.max.y + self.gap.y)
            };
            Rect::from_min_max(start, self.available.max)
        });
        let id = self.id.with(("flex item", index));
        let content = self.ui.push_id(id, |ui| {
            ui.scope_with_no_advance(rect, Layout::top_down(Align::Min), add_contents)
        });
        let size = content.response.rect.size();
        self.items
            .push((item, vec2(size.x.max(0.0), size.y.max(0.0))));
        self.used = self.used.union(Rect::from_min_size(rect.min, size));
        let mut response = content.response;
        response.rect = rect;
        InnerResponse {
            inner: content.inner,
            response,
        }
    }
}

impl<'a> FlexUi<'_, 'a> {
    /// The Ui the container is in (e.g. for its style).
    pub fn ui(&mut self) -> &mut Ui<'a> {
        self.ui
    }
}

/// A size in a [`FlexGrid`] track list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    /// Fixed, in points.
    Points(f32),
    /// A share of the free space (`1fr`).
    Fraction(f32),
    /// As big as its content.
    Auto,
}

impl Track {
    fn taffy(self) -> taffy::TrackSizingFunction {
        match self {
            Self::Points(p) => taffy::prelude::length(p),
            Self::Fraction(f) => taffy::prelude::fr(f),
            Self::Auto => taffy::prelude::auto(),
        }
    }
}

/// Where an item goes in a [`FlexGrid`]: column and row (from 0) and how
/// many it spans.
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

/// A CSS-style grid: column widths as fixed points, shares of the free
/// space (`fr`) or content size, items placed in cells that may span
/// several columns and rows (computed with taffy). For simple aligned
/// columns, [`crate::Grid`] is lighter.
///
/// ```ignore
/// FlexGrid::new("form", vec![Track::Points(120.0), Track::Fraction(1.0)]).show(ui, |grid| {
///     grid.add(GridCell::at(0, 0), |ui| ui.label("Name"));
///     grid.add(GridCell::at(1, 0), |ui| ui.text_edit_singleline(&mut name));
///     grid.add(GridCell::at(0, 1).span(2, 1), |ui| ui.button("Save"));
/// });
/// ```
#[derive(Clone, Debug)]
pub struct FlexGrid {
    id_salt: Id,
    columns: Vec<Track>,
    gap: Vec2,
}

/// Cells and their content sizes from the previous frame.
#[derive(Clone, Debug, Default, PartialEq)]
struct GridMemory {
    cells: Vec<(GridCell, Vec2)>,
}

impl FlexGrid {
    /// A grid with these column tracks (rows are as tall as their
    /// content).
    pub fn new(id_salt: impl Hash, columns: Vec<Track>) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            columns,
            gap: Vec2::splat(8.0),
        }
    }

    /// Space between columns and rows, in points (default 8).
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Vec2::splat(gap);
        self
    }

    /// Lays out the cells added by `add_cells`.
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_cells: impl FnOnce(&mut GridUi<'_, '_>) -> R,
    ) -> InnerResponse<R> {
        let id = ui.id().with(self.id_salt);
        let memory: GridMemory = ui.ctx().data(id).unwrap_or_default();
        let available = ui.available_rect();
        let columns = &self.columns;
        let auto_columns = |cell: &GridCell| {
            (cell.column..cell.column + cell.column_span)
                .any(|c| matches!(columns.get(c as usize), Some(Track::Auto) | None))
        };
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let children: Vec<NodeId> = memory
            .cells
            .iter()
            .map(|(cell, size)| {
                let style = taffy::Style {
                    // Content decides the width only of content-sized
                    // columns; fixed and fractional ones give their width
                    // to the content (which may fill it).
                    min_size: Size {
                        width: LengthPercentageAuto::length(if auto_columns(cell) {
                            size.x.min(available.width())
                        } else {
                            0.0
                        }),
                        height: LengthPercentageAuto::length(size.y),
                    },
                    grid_column: taffy::Line {
                        start: taffy::prelude::line(cell.column as i16 + 1),
                        end: taffy::prelude::span(cell.column_span),
                    },
                    grid_row: taffy::Line {
                        start: taffy::prelude::line(cell.row as i16 + 1),
                        end: taffy::prelude::span(cell.row_span),
                    },
                    ..Default::default()
                };
                tree.new_leaf(style).expect("leaf")
            })
            .collect();
        let width = available.width();
        let container = taffy::Style {
            display: taffy::Display::Grid,
            grid_template_columns: self
                .columns
                .iter()
                .map(|t| taffy::GridTemplateComponent::Single(t.taffy()))
                .collect(),
            gap: Size {
                width: LengthPercentage::length(self.gap.x),
                height: LengthPercentage::length(self.gap.y),
            },
            size: Size {
                width: if width.is_finite() {
                    Dimension::length(width)
                } else {
                    Dimension::auto()
                },
                height: Dimension::auto(),
            },
            ..Default::default()
        };
        let root = tree.new_with_children(container, &children).expect("root");
        let space = Size {
            width: if width.is_finite() {
                AvailableSpace::Definite(width)
            } else {
                AvailableSpace::MaxContent
            },
            height: AvailableSpace::MaxContent,
        };
        let _ = tree.compute_layout(root, space);
        let rects: Vec<Rect> = children
            .iter()
            .filter_map(|n| tree.layout(*n).ok())
            .map(|l| {
                Rect::from_min_size(
                    available.min + vec2(l.location.x, l.location.y),
                    vec2(l.size.width, l.size.height),
                )
            })
            .collect();
        let total = tree
            .layout(root)
            .map_or(Vec2::ZERO, |l| vec2(l.size.width, l.size.height));

        let mut grid = GridUi {
            ui,
            id,
            rects,
            cells: Vec::new(),
            available,
        };
        let inner = add_cells(&mut grid);
        let GridUi { ui, cells, .. } = grid;
        let new_memory = GridMemory { cells };
        if new_memory != memory {
            ui.ctx().insert_data(id, new_memory);
            ui.ctx().request_repaint();
        }
        let rect = ui.allocate_rect(vec2(total.x.max(0.0), total.y.max(0.0)));
        let response = ui.interact(id, rect, crate::Sense::HOVER);
        InnerResponse { inner, response }
    }
}

/// Adds cells to a [`FlexGrid`].
pub struct GridUi<'u, 'a> {
    ui: &'u mut Ui<'a>,
    id: Id,
    rects: Vec<Rect>,
    cells: Vec<(GridCell, Vec2)>,
    available: Rect,
}

impl std::fmt::Debug for GridUi<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridUi")
            .field("cells", &self.cells.len())
            .finish_non_exhaustive()
    }
}

impl GridUi<'_, '_> {
    /// Adds the content of `cell`, laid out top to bottom in its area.
    pub fn add<R>(
        &mut self,
        cell: GridCell,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let index = self.cells.len();
        let rect = self.rects.get(index).copied().unwrap_or(self.available);
        let id = self.id.with(("grid cell", index));
        let content = self.ui.push_id(id, |ui| {
            ui.scope_with_no_advance(rect, Layout::top_down(Align::Min), add_contents)
        });
        let size = content.response.rect.size();
        self.cells
            .push((cell, vec2(size.x.max(0.0), size.y.max(0.0))));
        let mut response = content.response;
        response.rect = rect;
        InnerResponse {
            inner: content.inner,
            response,
        }
    }
}
