//! Placement of the widgets of a [`crate::Stack`] or [`crate::Grid`].
//!
//! Every widget (or child Ui) added to such a Ui is one *item*. The
//! position of each item comes from a layout computed at the start of the
//! frame (with taffy) from the sizes the items had in the previous frame;
//! the sizes measured now are kept for the next frame. Items not seen
//! before go after the others until they are measured, so a layout
//! settles in two frames.

use std::collections::HashSet;

use rustroke_core::{Color, Rect, Vec2, point, vec2};
use taffy::prelude::{
    AvailableSpace, Dimension, LengthPercentage, LengthPercentageAuto, NodeId, Size, TaffyTree,
};

use crate::grid::{GridCell, Track};
use crate::stack::{Justify, StackAlign, StackItem};

/// One measured item, kept for the next frame's layout.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Measured {
    pub item: StackItem,
    /// Grid cell (grids only).
    pub cell: GridCell,
    /// The size its content asked for.
    pub size: Vec2,
}

/// What kind of container the items are in.
#[derive(Clone, Debug)]
pub(crate) enum SlotKind {
    /// A row (`row`) or column, possibly wrapping.
    Line { row: bool },
    /// Every item over the same area, the first at the back.
    Overlay,
    /// Rows and columns.
    Grid(GridSlots),
}

/// Where the next grid cell goes: a column and row (or the next free
/// place) and how many columns and rows it spans.
pub(crate) type CellRequest = (Option<(u16, u16)>, (u16, u16));

/// Auto placement and striping state of a grid.
#[derive(Clone, Debug)]
pub(crate) struct GridSlots {
    pub tracks: Vec<Track>,
    col: u16,
    row: u16,
    occupied: HashSet<(u16, u16)>,
    /// Fallback placement: top of the current row, bottom of its items.
    row_top: f32,
    row_bottom: f32,
    row_started: bool,
    /// Stripe color and last frame's row extents (top, bottom) and width.
    pub stripe: Option<Color>,
    pub rows: Vec<(f32, f32)>,
    pub width: f32,
    painted_row: Option<u16>,
    pub min_row_height: f32,
    /// Set by `Ui::grid_cell` / `Ui::grid_span` for the next item:
    /// explicit position (if any) and span.
    pub next_cell: Option<CellRequest>,
}

impl GridSlots {
    pub fn new(tracks: Vec<Track>, top: f32, min_row_height: f32) -> Self {
        Self {
            tracks,
            col: 0,
            row: 0,
            occupied: HashSet::new(),
            row_top: top,
            row_bottom: top,
            row_started: false,
            stripe: None,
            rows: Vec::new(),
            width: 0.0,
            painted_row: None,
            min_row_height,
            next_cell: None,
        }
    }

    /// Whether every column the cell covers sizes to its content.
    pub fn is_auto(&self, cell: &GridCell) -> bool {
        (cell.column..cell.column + cell.column_span)
            .all(|c| matches!(self.tracks.get(c as usize), Some(Track::Auto) | None))
    }

    fn free(&self, col: u16, row: u16, span: (u16, u16)) -> bool {
        (col..col + span.0).all(|c| (row..row + span.1).all(|r| !self.occupied.contains(&(c, r))))
    }

    /// The cell of the next item, advancing the auto placement.
    fn take_cell(&mut self) -> GridCell {
        let (pos, span) = self.next_cell.take().unwrap_or((None, (1, 1)));
        let span = (span.0.max(1), span.1.max(1));
        let (col, row) = match pos {
            Some(pos) => pos,
            None => {
                let mut col = self.col;
                while !self.free(col, self.row, span) {
                    col += 1;
                }
                (col, self.row)
            }
        };
        for c in col..col + span.0 {
            for r in row..row + span.1 {
                self.occupied.insert((c, r));
            }
        }
        if row == self.row {
            self.col = col + span.0;
        }
        GridCell {
            column: col,
            row,
            column_span: span.0,
            row_span: span.1,
        }
    }
}

/// Placement state of a Ui laying out a Stack or a Grid.
#[derive(Clone, Debug)]
pub(crate) struct Slots {
    pub kind: SlotKind,
    /// Where each item goes, from last frame's sizes.
    pub rects: Vec<Rect>,
    /// The area the content may use (may be unbounded).
    pub area: Rect,
    /// Default alignment of items along x and y.
    pub align: [StackAlign; 2],
    pub gap: Vec2,
    pub items: Vec<Measured>,
    /// Set by `Ui::stack_item` for the next item.
    pub next_item: Option<StackItem>,
    pending_natural: Option<Vec2>,
    used: Rect,
}

impl Slots {
    pub fn new(
        kind: SlotKind,
        rects: Vec<Rect>,
        area: Rect,
        align: [StackAlign; 2],
        gap: Vec2,
    ) -> Self {
        Self {
            kind,
            rects,
            area,
            align,
            gap,
            items: Vec::new(),
            next_item: None,
            pending_natural: None,
            used: Rect::NOTHING,
        }
    }

    fn next_props(&self) -> StackItem {
        self.next_item.unwrap_or_default()
    }

    /// Alignment of the next item along x and y.
    fn item_align(&self) -> [StackAlign; 2] {
        let item = self.next_props();
        [
            item.align_x.unwrap_or(self.align[0]),
            item.align_y.unwrap_or(self.align[1]),
        ]
    }

    /// Whether last frame's layout has a place for the next item.
    pub(crate) fn known(&self) -> bool {
        self.rects.get(self.items.len()).is_some()
    }

    /// The place last frame's layout assigned to the next item.
    pub(crate) fn slot_rect(&self) -> Rect {
        self.slot()
    }

    /// The area of the next item.
    fn slot(&self) -> Rect {
        if let Some(rect) = self.rects.get(self.items.len()) {
            return *rect;
        }
        let area = self.area;
        match &self.kind {
            SlotKind::Line { row } => {
                let start = if self.used == Rect::NOTHING {
                    area.min
                } else if *row {
                    point(self.used.max.x + self.gap.x, area.min.y)
                } else {
                    point(area.min.x, self.used.max.y + self.gap.y)
                };
                Rect::from_min_max(start, area.max.max(start))
            }
            SlotKind::Overlay => area,
            SlotKind::Grid(grid) => {
                let x = if grid.row_started {
                    self.used.max.x + self.gap.x
                } else {
                    area.min.x
                };
                let min = point(x, grid.row_top);
                Rect::from_min_max(
                    min,
                    point(area.max.x.max(x), grid.row_top + grid.min_row_height),
                )
            }
        }
    }

    /// The space offered to the next item's content.
    pub fn available(&self) -> Rect {
        let slot = self.slot();
        let area = self.area;
        let extend = |min: f32, max: f32, area_max: f32| (min, max.max(area_max));
        match &self.kind {
            SlotKind::Line { row } => {
                let item = self.next_props();
                let align = self.item_align();
                let (main_fixed, cross_fixed) = (item.grow > 0.0, {
                    let cross = if *row { align[1] } else { align[0] };
                    cross == StackAlign::Stretch
                });
                // Items keep the room to grow (so a longer text doesn't
                // wrap at last frame's width) unless the layout gave them
                // a size to fill.
                let (x_fixed, y_fixed) = if *row {
                    (main_fixed, cross_fixed)
                } else {
                    (cross_fixed, main_fixed)
                };
                let (x0, x1) = if x_fixed {
                    (slot.min.x, slot.max.x)
                } else {
                    extend(slot.min.x, slot.max.x, area.max.x)
                };
                let (y0, y1) = if y_fixed {
                    (slot.min.y, slot.max.y)
                } else {
                    extend(slot.min.y, slot.max.y, area.max.y)
                };
                Rect::from_min_max(point(x0, y0), point(x1, y1))
            }
            SlotKind::Overlay => area,
            SlotKind::Grid(grid) => {
                let auto = match grid.next_cell {
                    Some((Some((col, _)), span)) => grid.is_auto(&GridCell {
                        column: col,
                        row: 0,
                        column_span: span.0,
                        row_span: 1,
                    }),
                    Some((None, span)) => grid.is_auto(&GridCell {
                        column: grid.col,
                        row: 0,
                        column_span: span.0,
                        row_span: 1,
                    }),
                    None => grid.is_auto(&GridCell::at(grid.col, 0)),
                };
                if auto {
                    Rect::from_min_max(slot.min, point(slot.max.x.max(area.max.x), slot.max.y))
                } else {
                    slot
                }
            }
        }
    }

    /// Where a widget of `size` goes.
    pub fn place(&mut self, size: Vec2) -> Rect {
        self.place_inner(size, false)
    }

    /// Where a child Ui goes, from its size in the previous frame (which
    /// does not keep it from shrinking to its slot).
    pub fn place_child(&mut self, prev_size: Vec2) -> Rect {
        self.place_inner(prev_size, true)
    }

    fn place_inner(&mut self, size: Vec2, exact: bool) -> Rect {
        self.pending_natural = Some(size);
        let slot = self.slot();
        let known = self.known();
        let align = self.item_align();
        let along = |align: StackAlign, min: f32, max: f32, len: f32| -> (f32, f32) {
            let free = max - min;
            match align {
                StackAlign::Start => (min, len),
                StackAlign::Center => (min + (free - len) / 2.0, len),
                StackAlign::End => (max - len, len),
                StackAlign::Stretch => (min, free.max(len)),
            }
        };
        match &self.kind {
            SlotKind::Line { row } => {
                let item = self.next_props();
                let mut rect = Rect::from_min_size(slot.min, size);
                if known {
                    // Taffy already positioned the item; only stretched
                    // and grown sides take the slot's size.
                    let cross = if *row { align[1] } else { align[0] };
                    let grow = item.grow > 0.0;
                    let (fill_x, fill_y) = if *row {
                        (grow, cross == StackAlign::Stretch)
                    } else {
                        (cross == StackAlign::Stretch, grow)
                    };
                    // A growing side, or any filled side of a child Ui,
                    // takes exactly the slot; a stretched widget at least
                    // keeps its own size.
                    let fill = |grow_axis: bool, slot_max: f32, own_max: f32| {
                        if grow_axis || exact {
                            slot_max
                        } else {
                            slot_max.max(own_max)
                        }
                    };
                    if fill_x {
                        rect.max.x = fill(grow && *row, slot.max.x, rect.max.x);
                    }
                    if fill_y {
                        rect.max.y = fill(grow && !*row, slot.max.y, rect.max.y);
                    }
                }
                rect
            }
            SlotKind::Overlay | SlotKind::Grid(_) => {
                let (x, w) = along(align[0], slot.min.x, slot.max.x, size.x);
                let (y, h) = along(align[1], slot.min.y, slot.max.y, size.y);
                // Unmeasured overlays don't know their size yet: start at
                // the top-left instead of aligning in a guessed area.
                if !known && matches!(self.kind, SlotKind::Overlay) {
                    Rect::from_min_size(slot.min, size)
                } else {
                    Rect::from_min_size(point(x, y), vec2(w, h))
                }
            }
        }
    }

    /// The size the next item's content asked for (child Uis, measured
    /// after `place`).
    pub fn set_natural(&mut self, size: Vec2) {
        self.pending_natural = Some(size);
    }

    /// Records that an item took `rect`.
    pub fn advance(&mut self, rect: Rect) {
        let natural = self.pending_natural.take().unwrap_or(rect.size());
        let item = self.next_item.take().unwrap_or_default();
        let cell = match &mut self.kind {
            SlotKind::Grid(grid) => {
                grid.row_started = true;
                grid.row_bottom = grid.row_bottom.max(rect.max.y);
                grid.take_cell()
            }
            _ => GridCell::at(0, 0),
        };
        self.used = self.used.union(rect);
        self.items.push(Measured {
            item,
            cell,
            size: vec2(natural.x.max(0.0), natural.y.max(0.0)),
        });
    }

    /// Empty space of `amount` along a row or column.
    pub fn add_space(&mut self, amount: f32) {
        if let SlotKind::Line { row } = self.kind {
            let slot = self.slot();
            let size = if row {
                vec2(amount, 0.0)
            } else {
                vec2(0.0, amount)
            };
            self.pending_natural = Some(size);
            self.advance(Rect::from_min_size(slot.min, size));
        }
    }

    /// Ends a grid row.
    pub fn end_row(&mut self) {
        let gap = self.gap.y;
        if let SlotKind::Grid(grid) = &mut self.kind {
            let bottom = if grid.row_started {
                grid.row_bottom
            } else {
                grid.row_top + grid.min_row_height
            };
            grid.row += 1;
            grid.col = 0;
            grid.row_top = bottom + gap;
            grid.row_bottom = grid.row_top;
            grid.row_started = false;
        }
    }

    /// Ends the last grid row if it has cells.
    pub fn end_row_if_started(&mut self) {
        if matches!(&self.kind, SlotKind::Grid(g) if g.row_started) {
            self.end_row();
        }
    }

    /// The stripe to paint before the first item of an odd grid row.
    pub fn stripe(&mut self) -> Option<(Rect, Color)> {
        let gap = self.gap;
        let left = self.area.min.x;
        let SlotKind::Grid(grid) = &mut self.kind else {
            return None;
        };
        let color = grid.stripe?;
        if grid.row_started || grid.painted_row == Some(grid.row) {
            return None;
        }
        grid.painted_row = Some(grid.row);
        if grid.row % 2 == 0 {
            return None;
        }
        let (top, bottom) = *grid.rows.get(grid.row as usize)?;
        if top > bottom {
            return None;
        }
        let pad = gap.y / 2.0;
        Some((
            Rect::from_min_max(
                point(left - pad, top - pad),
                point(left + grid.width + pad, bottom + pad),
            ),
            color,
        ))
    }
}

/// Converts a finite value to a taffy length, infinity to `auto`.
fn length_or_auto(v: f32) -> LengthPercentageAuto {
    if v.is_finite() {
        LengthPercentageAuto::length(v)
    } else {
        LengthPercentageAuto::auto()
    }
}

/// Settings of a row or column layout.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LineParams {
    pub row: bool,
    pub wrap: bool,
    pub gap: Vec2,
    pub justify: Justify,
    pub align: StackAlign,
    /// Content width and height when set.
    pub fixed: [Option<f32>; 2],
    /// The most the content may take.
    pub room: Vec2,
}

/// Positions of the items of a row or column (relative to its top-left)
/// and its total size, from the items' sizes.
pub(crate) fn layout_line(items: &[Measured], p: LineParams) -> (Vec<Rect>, Vec2) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let children: Vec<NodeId> = items
        .iter()
        .map(|m| {
            let item = &m.item;
            let size = m.size;
            let cross_align = if p.row { item.align_y } else { item.align_x };
            let stretch = cross_align.unwrap_or(p.align) == StackAlign::Stretch;
            let (mut width, mut height) = match (stretch, p.row) {
                (true, true) => (Dimension::length(size.x), Dimension::auto()),
                (true, false) => (Dimension::auto(), Dimension::length(size.y)),
                _ => (Dimension::length(size.x), Dimension::length(size.y)),
            };
            // A growing item gets its share of the free space whatever its
            // content (which often fills what it gets), like CSS `flex: 1`.
            let grows = item.grow > 0.0;
            if grows {
                if p.row {
                    width = Dimension::auto();
                } else {
                    height = Dimension::auto();
                }
            }
            let (min_width, min_height) = match (stretch, p.row) {
                (true, true) => (item.min_size.x, item.min_size.y.max(size.y)),
                (true, false) => (item.min_size.x.max(size.x), item.min_size.y),
                _ => (item.min_size.x, item.min_size.y),
            };
            let style = taffy::Style {
                size: Size { width, height },
                flex_grow: item.grow,
                flex_shrink: item.shrink,
                flex_basis: item.basis.map_or(
                    if grows {
                        Dimension::length(0.0)
                    } else {
                        Dimension::auto()
                    },
                    Dimension::length,
                ),
                min_size: Size {
                    width: LengthPercentageAuto::length(min_width),
                    height: LengthPercentageAuto::length(min_height),
                },
                max_size: Size {
                    width: length_or_auto(item.max_size.x),
                    height: length_or_auto(item.max_size.y),
                },
                align_self: cross_align.map(StackAlign::taffy),
                ..Default::default()
            };
            tree.new_leaf(style).expect("leaf")
        })
        .collect();
    let definite = |v: Option<f32>| v.map_or(Dimension::auto(), Dimension::length);
    let container = taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: if p.row {
            taffy::FlexDirection::Row
        } else {
            taffy::FlexDirection::Column
        },
        flex_wrap: if p.wrap {
            taffy::FlexWrap::Wrap
        } else {
            taffy::FlexWrap::NoWrap
        },
        gap: Size {
            width: LengthPercentage::length(p.gap.x),
            height: LengthPercentage::length(p.gap.y),
        },
        justify_content: Some(p.justify.taffy()),
        align_items: Some(p.align.taffy()),
        align_content: Some(taffy::AlignContent::FLEX_START),
        size: Size {
            width: definite(p.fixed[0]),
            height: definite(p.fixed[1]),
        },
        max_size: Size {
            width: length_or_auto(p.room.x),
            height: length_or_auto(p.room.y),
        },
        ..Default::default()
    };
    let root = tree.new_with_children(container, &children).expect("root");
    let space = |v: f32| {
        if v.is_finite() {
            AvailableSpace::Definite(v)
        } else {
            AvailableSpace::MaxContent
        }
    };
    let _ = tree.compute_layout(
        root,
        Size {
            width: space(p.room.x),
            height: space(p.room.y),
        },
    );
    collect(&tree, root, &children)
}

/// Settings of a grid layout.
#[derive(Clone, Debug)]
pub(crate) struct GridParams<'a> {
    pub tracks: &'a [Track],
    pub gap: Vec2,
    pub min_row_height: f32,
    pub fixed: [Option<f32>; 2],
    pub room: Vec2,
}

/// Positions of the cells of a grid (relative to its top-left) and its
/// total size.
pub(crate) fn layout_grid(items: &[Measured], p: &GridParams<'_>) -> (Vec<Rect>, Vec2) {
    let auto = |cell: &GridCell| {
        (cell.column..cell.column + cell.column_span)
            .all(|c| matches!(p.tracks.get(c as usize), Some(Track::Auto) | None))
    };
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let children: Vec<NodeId> = items
        .iter()
        .map(|m| {
            let cell = &m.cell;
            // Content decides the width only of content-sized columns;
            // fixed and fractional ones give their width to the content.
            let style = taffy::Style {
                min_size: Size {
                    width: LengthPercentageAuto::length(if auto(cell) {
                        m.size.x.min(p.room.x)
                    } else {
                        0.0
                    }),
                    height: LengthPercentageAuto::length(m.size.y.max(p.min_row_height)),
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
    let has_fraction = p.tracks.iter().any(|t| matches!(t, Track::Fraction(_)));
    let width = p.fixed[0].or((has_fraction && p.room.x.is_finite()).then_some(p.room.x));
    let container = taffy::Style {
        display: taffy::Display::Grid,
        grid_template_columns: p
            .tracks
            .iter()
            .map(|t| taffy::GridTemplateComponent::Single(t.taffy()))
            .collect(),
        gap: Size {
            width: LengthPercentage::length(p.gap.x),
            height: LengthPercentage::length(p.gap.y),
        },
        size: Size {
            width: width.map_or(Dimension::auto(), Dimension::length),
            height: p.fixed[1].map_or(Dimension::auto(), Dimension::length),
        },
        align_content: Some(taffy::AlignContent::START),
        justify_content: Some(taffy::JustifyContent::START),
        ..Default::default()
    };
    let root = tree.new_with_children(container, &children).expect("root");
    let space = |v: f32| {
        if v.is_finite() {
            AvailableSpace::Definite(v)
        } else {
            AvailableSpace::MaxContent
        }
    };
    let _ = tree.compute_layout(
        root,
        Size {
            width: space(p.room.x),
            height: AvailableSpace::MaxContent,
        },
    );
    collect(&tree, root, &children)
}

fn collect(tree: &TaffyTree<()>, root: NodeId, children: &[NodeId]) -> (Vec<Rect>, Vec2) {
    let rects = children
        .iter()
        .filter_map(|n| tree.layout(*n).ok())
        .map(|l| {
            Rect::from_min_size(
                point(l.location.x, l.location.y),
                vec2(l.size.width, l.size.height),
            )
        })
        .collect();
    let total = tree
        .layout(root)
        .map_or(Vec2::ZERO, |l| vec2(l.size.width, l.size.height));
    (rects, total)
}
