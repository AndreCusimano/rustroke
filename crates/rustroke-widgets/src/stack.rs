//! [`Stack`]: widgets in a row, in a column or one over the other, in a
//! box with an optional background, border and size.

use std::hash::Hash;

use rustroke_core::{Rect, Vec2, vec2};

use crate::container::{BoxArea, box_setters};
use crate::slots::{LineParams, Measured, SlotKind, Slots, layout_line};
use crate::{Align, BoxStyle, Id, InnerResponse, Layout, Ui};

/// Where items sit inside their space, along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StackAlign {
    /// At the start (left or top).
    #[default]
    Start,
    /// In the middle.
    Center,
    /// At the end (right or bottom).
    End,
    /// Stretched over the whole space.
    Stretch,
}

impl StackAlign {
    pub(crate) fn taffy(self) -> taffy::AlignItems {
        match self {
            Self::Start => taffy::AlignItems::FLEX_START,
            Self::End => taffy::AlignItems::FLEX_END,
            Self::Center => taffy::AlignItems::CENTER,
            Self::Stretch => taffy::AlignItems::STRETCH,
        }
    }
}

/// How the items of a row or column share the free space along it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Justify {
    /// Packed at the start.
    #[default]
    Start,
    /// Packed in the middle.
    Center,
    /// Packed at the end.
    End,
    /// First and last at the edges, equal space between the others.
    SpaceBetween,
    /// Equal space around each item.
    SpaceAround,
    /// Equal space between items and at the edges.
    SpaceEvenly,
}

impl Justify {
    pub(crate) fn taffy(self) -> taffy::JustifyContent {
        match self {
            Self::Start => taffy::JustifyContent::FLEX_START,
            Self::End => taffy::JustifyContent::FLEX_END,
            Self::Center => taffy::JustifyContent::CENTER,
            Self::SpaceBetween => taffy::JustifyContent::SPACE_BETWEEN,
            Self::SpaceAround => taffy::JustifyContent::SPACE_AROUND,
            Self::SpaceEvenly => taffy::JustifyContent::SPACE_EVENLY,
        }
    }
}

/// How one item of a [`Stack`] grows, shrinks and aligns. Give it with
/// [`Ui::stack_item`]; other widgets use the defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackItem {
    pub(crate) grow: f32,
    pub(crate) shrink: f32,
    pub(crate) basis: Option<f32>,
    pub(crate) min_size: Vec2,
    pub(crate) max_size: Vec2,
    pub(crate) align_x: Option<StackAlign>,
    pub(crate) align_y: Option<StackAlign>,
}

impl Default for StackItem {
    fn default() -> Self {
        Self::new()
    }
}

impl StackItem {
    /// Its natural size: no growing, shrinking when there is not enough
    /// room.
    pub fn new() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: None,
            min_size: Vec2::ZERO,
            max_size: Vec2::splat(f32::INFINITY),
            align_x: None,
            align_y: None,
        }
    }

    /// Takes this share of the free space along the row or column (e.g.
    /// 1 for a search field that fills a toolbar), whatever the size of
    /// its content: give it a [`StackItem::min_size`] to keep it from
    /// getting too small. The stack needs room to share: a set or filled
    /// size.
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

    /// Starting size along the row or column, in points, instead of its
    /// content's size.
    pub fn basis(mut self, basis: f32) -> Self {
        self.basis = Some(basis);
        self
    }

    /// Never smaller than this (width, height), even when shrinking.
    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = size;
        self
    }

    /// Never bigger than this (width, height), even when growing or
    /// stretched; `f32::INFINITY` leaves a side free.
    pub fn max_size(mut self, size: Vec2) -> Self {
        self.max_size = size;
        self
    }

    /// Its own alignment across the row or column (in an overlay: along
    /// both axes), instead of the stack's.
    pub fn align(mut self, align: StackAlign) -> Self {
        self.align_x = Some(align);
        self.align_y = Some(align);
        self
    }

    /// Its own horizontal alignment (columns and overlays).
    pub fn align_x(mut self, align: StackAlign) -> Self {
        self.align_x = Some(align);
        self
    }

    /// Its own vertical alignment (rows and overlays).
    pub fn align_y(mut self, align: StackAlign) -> Self {
        self.align_y = Some(align);
        self
    }
}

/// Which way a [`Stack`] places its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StackDirection {
    /// Left to right.
    Row,
    /// Top to bottom.
    Column,
    /// One over the other in the same area: the first added at the back,
    /// the last on top (a "z-stack").
    Overlay,
}

/// Measured items of a stack, kept for the next frame.
#[derive(Clone, Debug, Default, PartialEq)]
struct StackMemory {
    items: Vec<Measured>,
}

/// A group of widgets in a row, a column or one over the other
/// ([`Stack::overlay`]), in a box that can have a background (color,
/// gradient or image), a border, rounded corners, a shadow, padding and
/// margins, and a size: its content's, all the available space, or fixed,
/// with limits.
///
/// Every widget added inside is one item. Items are aligned across the
/// stack with [`Stack::align`] and spread along it with
/// [`Stack::justify`]; [`Ui::stack_item`] gives one item its own options
/// (grow to fill the free space, size limits, alignment).
///
/// ```ignore
/// // A toolbar whose search field takes the free width.
/// Stack::row().fill_width().align(StackAlign::Center).show(ui, |ui| {
///     ui.button("Open");
///     ui.stack_item(StackItem::new().grow(1.0), |ui| {
///         ui.add(TextEdit::singleline(&mut search).desired_width(f32::INFINITY))
///     });
///     ui.button("Go");
/// });
///
/// // A card.
/// Stack::column().fill(card).corner_radius(8.0).padding(12.0).show(ui, |ui| {
///     ui.heading("Title");
///     ui.label("Text");
/// });
///
/// // A badge over an image.
/// Stack::overlay().show(ui, |ui| {
///     ui.image(&photo);
///     ui.stack_item(StackItem::new().align_x(StackAlign::End), |ui| ui.label("NEW"));
/// });
/// ```
///
/// For the common case, `ui.horizontal(..)` and `ui.vertical(..)` are
/// shorter: a row or column without a box. Like the other layouts that
/// depend on sizes, a stack uses the sizes its items had in the previous
/// frame and settles in two frames.
#[derive(Clone, Debug)]
pub struct Stack {
    direction: StackDirection,
    id_salt: Option<Id>,
    gap: Option<f32>,
    wrap: bool,
    justify: Option<Justify>,
    align: [Option<StackAlign>; 2],
    box_style: BoxStyle,
}

impl Stack {
    fn new(direction: StackDirection) -> Self {
        Self {
            direction,
            id_salt: None,
            gap: None,
            wrap: false,
            justify: None,
            align: [None, None],
            box_style: BoxStyle::new(),
        }
    }

    /// Items left to right.
    pub fn row() -> Self {
        Self::new(StackDirection::Row)
    }

    /// Items top to bottom.
    pub fn column() -> Self {
        Self::new(StackDirection::Column)
    }

    /// Items one over the other: the first at the back, the last on top.
    /// The stack is as big as its biggest item; each item is placed by
    /// [`Stack::align_x`] / [`Stack::align_y`] (or its own).
    pub fn overlay() -> Self {
        Self::new(StackDirection::Overlay)
    }

    /// Identifies the stack (to keep its measurements) when stacks may
    /// appear, disappear or move around it; by default it is identified
    /// by its position among the widgets of its Ui.
    pub fn id_salt(mut self, salt: impl Hash) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    /// Space between items (and between lines when wrapping), in points.
    /// Default: the style's item spacing.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Items that don't fit move to a new line (rows and columns).
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// How items share the free space along a row or column. Needs a
    /// stack bigger than its content: a filled or set size.
    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify = Some(justify);
        self
    }

    /// Alignment across the row or column: vertical in a row, horizontal
    /// in a column, both in an overlay. `Stretch` makes the items as tall
    /// as the row (or as wide as the column).
    pub fn align(mut self, align: StackAlign) -> Self {
        match self.direction {
            StackDirection::Row => self.align[1] = Some(align),
            StackDirection::Column => self.align[0] = Some(align),
            StackDirection::Overlay => self.align = [Some(align), Some(align)],
        }
        self
    }

    /// Horizontal position of the items: across a column, in an overlay,
    /// and along a row (like [`Stack::justify`] with start, center or
    /// end).
    pub fn align_x(mut self, align: StackAlign) -> Self {
        self.align[0] = Some(align);
        self
    }

    /// Vertical position of the items: across a row, in an overlay, and
    /// along a column (like [`Stack::justify`] with start, center or end).
    pub fn align_y(mut self, align: StackAlign) -> Self {
        self.align[1] = Some(align);
        self
    }

    /// The box's look and size from a [`BoxStyle`] (replacing what was
    /// set before).
    pub fn style(mut self, style: BoxStyle) -> Self {
        self.box_style = style;
        self
    }

    box_setters!(box_style);

    /// Shows the stack with the widgets added by `add_contents`.
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let id = match self.id_salt {
            Some(salt) => ui.id().with(salt),
            None => ui.next_auto_id(),
        };
        let style = ui.style();
        let spacing = style.spacing.item_spacing;
        let row = self.direction == StackDirection::Row;
        let gap = match self.gap {
            Some(g) => Vec2::splat(g),
            None if self.wrap => spacing,
            None => Vec2::splat(if row { spacing.x } else { spacing.y }),
        };
        let main = usize::from(!row);
        let cross = 1 - main;
        let align = [
            self.align[0].unwrap_or_default(),
            self.align[1].unwrap_or_default(),
        ];
        let justify = self.justify.unwrap_or(match align[main] {
            StackAlign::Center => Justify::Center,
            StackAlign::End => Justify::End,
            _ => Justify::Start,
        });
        let direction = self.direction;
        let wrap = self.wrap;
        self.box_style.show(ui, |outer, area: BoxArea| {
            let memory: StackMemory = outer.ctx().data(id).unwrap_or_default();
            let mut total = Vec2::ZERO;
            let (kind, rects, layout, content_area) = match direction {
                StackDirection::Overlay => {
                    // As big as the biggest item last frame, unless set.
                    let biggest = memory.items.iter().fold(Vec2::ZERO, |a, m| {
                        vec2(a.x.max(m.size.x), a.y.max(m.size.y))
                    });
                    let size = vec2(
                        area.fixed[0].unwrap_or(biggest.x.min(area.room.x)),
                        area.fixed[1].unwrap_or(biggest.y.min(area.room.y)),
                    );
                    let content = if memory.items.is_empty() {
                        area.rect()
                    } else {
                        Rect::from_min_size(area.min, size)
                    };
                    let rects = vec![content; memory.items.len()];
                    (
                        SlotKind::Overlay,
                        rects,
                        Layout::top_down(Align::Min),
                        content,
                    )
                }
                _ => {
                    let (rects, size) = layout_line(
                        &memory.items,
                        LineParams {
                            row,
                            wrap,
                            gap,
                            justify,
                            align: align[cross],
                            fixed: area.fixed,
                            room: area.room,
                        },
                    );
                    total = size;
                    let rects = rects
                        .into_iter()
                        .map(|r| Rect::from_min_size(area.min + r.min.to_vec2(), r.size()))
                        .collect();
                    let layout = if row {
                        Layout::left_to_right(Align::Min)
                    } else {
                        Layout::top_down(Align::Min)
                    };
                    (SlotKind::Line { row }, rects, layout, area.rect())
                }
            };
            let slots = Slots::new(kind, rects, content_area, align, gap);
            let (inner, slots, used) = outer.slot_child(
                id.with("content"),
                content_area,
                layout,
                slots,
                add_contents,
            );
            let new_memory = StackMemory { items: slots.items };
            // The content's size: the layout of last frame's sizes, or
            // what the items used if bigger.
            let laid_out = match direction {
                StackDirection::Overlay => new_memory.items.iter().fold(Vec2::ZERO, |a, m| {
                    vec2(a.x.max(m.size.x), a.y.max(m.size.y))
                }),
                _ => total,
            };
            let used_size = if used.is_empty() || new_memory.items.is_empty() {
                Vec2::ZERO
            } else {
                used.max - area.min
            };
            let content = vec2(
                area.fixed[0].unwrap_or(laid_out.x.max(used_size.x)),
                area.fixed[1].unwrap_or(laid_out.y.max(used_size.y)),
            );
            if new_memory != memory {
                outer.ctx().insert_data(id, new_memory);
                outer.ctx().request_repaint();
            }
            (inner, content)
        })
    }
}
