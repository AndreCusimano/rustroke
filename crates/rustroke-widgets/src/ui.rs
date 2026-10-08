use std::hash::Hash;
use std::ops::RangeInclusive;
use std::sync::Arc;

use rustroke_core::{Color, DisplayList, Galley, InputState, Point, Rect, Vec2};
use rustroke_text::{Fonts, TextStyle};

use crate::grid::GridLayout;
use crate::widgets::{Button, Checkbox, Label, Numeric, RadioButton, Separator, Slider, Widget};
use crate::{Align, Context, Direction, Id, LayerId, Layout, Response, Sense, Style};

/// The result of a closure that added widgets to a child [`Ui`], plus a
/// response covering the area they used.
#[derive(Clone, Debug)]
pub struct InnerResponse<R> {
    /// What the closure returned.
    pub inner: R,
    /// Covers the area the content used.
    pub response: Response,
}

/// A grid being laid out by this Ui.
#[derive(Clone, Debug)]
struct UiGrid {
    layout: GridLayout,
    stripe_color: Color,
    /// Row whose stripe was already painted.
    painted_row: Option<usize>,
}

/// Places widgets in a region of the window and draws them.
///
/// Widgets are placed one after another following the Ui's [`Layout`]
/// (top-down by default). Use [`Ui::horizontal`], [`Ui::vertical`],
/// [`Ui::with_layout`] or [`crate::Grid`] to arrange them differently.
pub struct Ui<'a> {
    ctx: &'a mut Context,
    fonts: &'a mut Fonts,
    /// The layer this Ui draws into.
    layer: LayerId,
    /// Shapes and interactions are limited to this area.
    clip_rect: Rect,
    id: Id,
    /// Counter for automatic widget ids.
    next_id: u64,
    style: Arc<Style>,
    layout: Layout,
    max_rect: Rect,
    /// Where the next widget goes: its top for top-down layouts; its left
    /// (or right, for right-to-left) edge and the top of the row for rows.
    cursor: Point,
    /// Height of the current row (rows only), for wrapping.
    row_height: f32,
    /// Union of everything placed so far.
    min_rect: Rect,
    grid: Option<UiGrid>,
    /// Inside a popup menu (with the menu's width): buttons highlight
    /// across the menu and close it when clicked.
    in_menu: Option<f32>,
    /// Widgets react to input. Disabled Uis draw their widgets faded.
    enabled: bool,
}

impl std::fmt::Debug for Ui<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ui")
            .field("id", &self.id)
            .field("layout", &self.layout)
            .field("max_rect", &self.max_rect)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl<'a> Ui<'a> {
    pub(crate) fn new(
        ctx: &'a mut Context,
        fonts: &'a mut Fonts,
        layer: LayerId,
        id: Id,
        max_rect: Rect,
    ) -> Self {
        let style = Arc::clone(ctx.style());
        Self {
            ctx,
            fonts,
            layer,
            clip_rect: Rect::EVERYTHING,
            id,
            next_id: 0,
            style,
            layout: Layout::default(),
            max_rect,
            cursor: max_rect.min,
            row_height: 0.0,
            min_rect: Rect::NOTHING,
            grid: None,
            in_menu: None,
            enabled: true,
        }
    }

    /// A child Ui drawing into the same list, with its own layout and area.
    fn child(&mut self, max_rect: Rect, layout: Layout) -> Ui<'_> {
        let id = self.next_auto_id();
        self.child_with_id(id, max_rect, layout)
    }

    fn child_with_id(&mut self, id: Id, max_rect: Rect, layout: Layout) -> Ui<'_> {
        let cursor = match layout.direction {
            Direction::RightToLeft => Point::new(max_rect.max.x, max_rect.min.y),
            _ => max_rect.min,
        };
        Ui {
            ctx: &mut *self.ctx,
            fonts: &mut *self.fonts,
            layer: self.layer,
            clip_rect: self.clip_rect,
            id,
            next_id: 0,
            style: Arc::clone(&self.style),
            layout,
            max_rect,
            cursor,
            row_height: 0.0,
            min_rect: Rect::NOTHING,
            grid: None,
            in_menu: self.in_menu,
            enabled: self.enabled,
        }
    }

    /// A new top-level Ui in another layer (popups, tooltips), sharing
    /// this Ui's context and style but not its clip area.
    pub(crate) fn layer_ui(&mut self, layer: LayerId, id: Id, max_rect: Rect) -> Ui<'_> {
        let style = Arc::clone(&self.style);
        let mut ui = Ui::new(&mut *self.ctx, &mut *self.fonts, layer, id, max_rect);
        ui.style = style;
        ui
    }

    pub(crate) fn set_in_menu(&mut self, menu_width: Option<f32>) {
        self.in_menu = menu_width;
    }

    /// The menu width, if this Ui is inside a popup menu.
    pub(crate) fn in_menu(&self) -> Option<f32> {
        self.in_menu
    }

    // ---- Access ----

    /// The context shared by every Ui of the frame.
    pub fn ctx(&mut self) -> &mut Context {
        self.ctx
    }

    /// Input of the current frame.
    pub fn input(&self) -> &InputState {
        self.ctx.input()
    }

    /// Mutable input, e.g. [`InputState::consume_scroll`] or
    /// [`InputState::consume_key`].
    pub fn input_mut(&mut self) -> &mut InputState {
        self.ctx.input_mut()
    }

    /// This Ui's id; widget ids are derived from it.
    pub fn id(&self) -> Id {
        self.id
    }

    /// The style used by widgets in this Ui (and its children).
    pub fn style(&self) -> Arc<Style> {
        Arc::clone(&self.style)
    }

    /// Changes the style for this Ui and the children created after this
    /// call, e.g. `ui.style_mut().spacing.item_spacing.x = 2.0`.
    pub fn style_mut(&mut self) -> &mut Style {
        Arc::make_mut(&mut self.style)
    }

    /// How widgets are placed in this Ui.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Where shapes are drawn; use it for custom painting. Shapes added
    /// to it are clipped to [`Ui::clip_rect`].
    pub fn painter(&mut self) -> &mut DisplayList {
        let clip = self.clip_rect;
        let shapes = self.ctx.layer_shapes(self.layer);
        shapes.set_clip_rect(clip);
        shapes
    }

    /// The layer this Ui draws into.
    pub fn layer(&self) -> LayerId {
        self.layer
    }

    /// Area outside of which nothing is drawn or clickable.
    pub fn clip_rect(&self) -> Rect {
        self.clip_rect
    }

    /// Narrows the clip area to `rect` (intersected with the current one).
    pub fn set_clip_rect(&mut self, rect: Rect) {
        self.clip_rect = self.clip_rect.intersect(rect);
    }

    /// Sets the clip area back to a value from [`Ui::clip_rect`].
    pub fn clip_rect_restore(&mut self, rect: Rect) {
        self.clip_rect = rect;
    }

    /// The colors for a widget in the state of `response`, animated: hover
    /// and press fade in and out over the style's animation time.
    pub fn widget_visuals(&mut self, response: &Response) -> crate::WidgetVisuals {
        if !self.enabled {
            return self.style.visuals.inactive;
        }
        let time = self.style.animation_time;
        let hover = self.ctx.animate_bool_with_time(
            response.id.with("hover"),
            response.hovered() || response.is_pressed(),
            time,
        );
        let press =
            self.ctx
                .animate_bool_with_time(response.id.with("press"), response.is_pressed(), time);
        let v = &self.style.visuals;
        v.inactive.lerp(&v.hovered, hover).lerp(&v.active, press)
    }

    /// Registers a widget at `rect` and computes its interaction. In a
    /// disabled Ui the widget only reports hovering.
    pub fn interact(&mut self, id: Id, rect: Rect, sense: Sense) -> Response {
        let sense = if self.enabled { sense } else { Sense::HOVER };
        self.ctx
            .interact(self.layer, id, rect, self.clip_rect, sense)
    }

    /// Tells screen readers what the widget of `response` is (role, label,
    /// state). Built-in widgets do this themselves; custom widgets should.
    pub fn describe(&mut self, response: &Response, info: crate::WidgetInfo) {
        self.ctx
            .describe(response.id, info, response.rect, self.enabled);
    }

    /// Whether widgets in this Ui react to input.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Adds `widget`, disabled (faded, not interactive) unless `enabled`.
    pub fn add_enabled(&mut self, enabled: bool, widget: impl Widget) -> Response {
        self.add_enabled_ui(enabled, |ui| ui.add(widget)).inner
    }

    /// Runs `add_contents` with everything disabled unless `enabled`.
    pub fn add_enabled_ui<R>(
        &mut self,
        enabled: bool,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let fade = self.enabled && !enabled;
        let start = self.ctx.layer_shapes(self.layer).len();
        let was_enabled = self.enabled;
        self.enabled &= enabled;
        let used_before = self.min_rect;
        self.min_rect = Rect::NOTHING;
        let inner = add_contents(self);
        let used = self.min_rect();
        self.min_rect = used_before.union(self.min_rect);
        self.enabled = was_enabled;
        if fade {
            let alpha = self.style.visuals.disabled_alpha;
            self.ctx
                .layer_shapes(self.layer)
                .multiply_alpha_from(start, alpha);
        }
        let id = self.next_auto_id();
        let response = self
            .ctx
            .interact(self.layer, id, used, self.clip_rect, Sense::HOVER);
        InnerResponse { inner, response }
    }

    /// Fonts, e.g. for custom text layout.
    pub fn fonts(&mut self) -> &mut Fonts {
        self.fonts
    }

    /// Physical pixels per logical point.
    pub fn pixels_per_point(&self) -> f32 {
        self.ctx.input().pixels_per_point
    }

    /// The region this Ui may place widgets in.
    pub fn max_rect(&self) -> Rect {
        self.max_rect
    }

    /// The area used by the widgets placed so far.
    pub fn min_rect(&self) -> Rect {
        if self.min_rect == Rect::NOTHING {
            Rect::from_min_size(self.cursor, Vec2::ZERO)
        } else {
            self.min_rect
        }
    }

    /// The space left for the next widget.
    pub fn available_rect(&self) -> Rect {
        let max = self.max_rect;
        if let Some(grid) = &self.grid {
            let cell = grid.layout.cell_rect(max.max.x);
            return Rect::from_min_max(cell.min, Point::new(cell.max.x, max.max.y));
        }
        let rect = match self.layout.direction {
            Direction::TopDown => Rect::from_min_max(Point::new(max.min.x, self.cursor.y), max.max),
            Direction::LeftToRight => Rect::from_min_max(self.cursor, max.max),
            Direction::RightToLeft => Rect::from_min_max(
                Point::new(max.min.x, self.cursor.y),
                Point::new(self.cursor.x, max.max.y),
            ),
        };
        // Never inverted, even when the content overflowed.
        Rect::from_min_max(rect.min, rect.max.max(rect.min))
    }

    /// Width left for the next widget.
    pub fn available_width(&self) -> f32 {
        self.available_rect().width()
    }

    /// Lays out text with this frame's pixel density.
    pub fn layout_text(
        &mut self,
        text: &str,
        style: &TextStyle,
        wrap_width: Option<f32>,
    ) -> Arc<Galley> {
        let ppp = self.pixels_per_point();
        self.fonts.layout(text, style, wrap_width, ppp)
    }

    // ---- Ids ----

    /// The id the next automatically identified widget will get.
    pub fn next_auto_id(&mut self) -> Id {
        let id = self.id.with(self.next_id);
        self.next_id += 1;
        id
    }

    /// Runs `add_contents` with widget ids derived from `salt` instead of
    /// the running counter. Use it for content that may appear, disappear
    /// or be reordered (e.g. list items), so ids stay attached to the right
    /// item.
    pub fn push_id<R>(&mut self, salt: impl Hash, add_contents: impl FnOnce(&mut Self) -> R) -> R {
        let saved = (self.id, self.next_id);
        self.id = self.id.with(salt);
        self.next_id = 0;
        let result = add_contents(self);
        (self.id, self.next_id) = saved;
        result
    }

    // ---- Placement ----

    /// Reserves space for a widget of `size`, positioned according to the
    /// layout, and advances past it. The returned rect may be larger than
    /// `size` when the layout justifies (stretches) widgets.
    pub fn allocate_rect(&mut self, size: Vec2) -> Rect {
        self.paint_grid_stripe();
        let rect = self.place(size);
        self.advance_after_rect(rect);
        rect
    }

    /// Reserves space for a widget and computes its interaction.
    pub fn allocate_response(&mut self, size: Vec2, sense: Sense) -> Response {
        let id = self.next_auto_id();
        let rect = self.allocate_rect(size);
        self.interact(id, rect, sense)
    }

    /// Where a widget of `size` goes, without advancing.
    fn place(&mut self, size: Vec2) -> Rect {
        if let Some(grid) = &self.grid {
            return grid.layout.place(size, self.max_rect.max.x);
        }
        let layout = self.layout;
        let max = self.max_rect;
        match layout.direction {
            Direction::TopDown => {
                let avail = self.available_rect();
                let w = if layout.cross_justify {
                    avail.width().max(size.x)
                } else {
                    size.x
                };
                let h = if layout.main_justify {
                    avail.height().max(size.y)
                } else {
                    size.y
                };
                let x = avail.min.x + layout.cross_align.offset(avail.width(), w);
                Rect::from_min_size(Point::new(x, self.cursor.y), Vec2::new(w, h))
            }
            Direction::LeftToRight | Direction::RightToLeft => {
                let ltr = layout.direction == Direction::LeftToRight;
                let overflows = if ltr {
                    self.cursor.x + size.x > max.max.x && self.cursor.x > max.min.x
                } else {
                    self.cursor.x - size.x < max.min.x && self.cursor.x < max.max.x
                };
                if layout.main_wrap && overflows {
                    self.new_line();
                }
                let band = if layout.cross_justify {
                    max.max.y - self.cursor.y
                } else {
                    self.style.spacing.interact_height
                };
                let h = if layout.cross_justify {
                    band.max(size.y)
                } else {
                    size.y
                };
                let free = if ltr {
                    max.max.x - self.cursor.x
                } else {
                    self.cursor.x - max.min.x
                };
                let w = if layout.main_justify {
                    free.max(size.x)
                } else {
                    size.x
                };
                let y = self.cursor.y + layout.cross_align.offset(band, h);
                let x = if ltr {
                    self.cursor.x
                } else {
                    self.cursor.x - w
                };
                Rect::from_min_size(Point::new(x, y), Vec2::new(w, h))
            }
        }
    }

    /// Moves the cursor past `rect`, as if a widget had been placed there.
    fn advance_after_rect(&mut self, rect: Rect) {
        self.min_rect = self.min_rect.union(rect);
        if let Some(grid) = &mut self.grid {
            grid.layout.advance(rect);
            return;
        }
        let spacing = self.style.spacing.item_spacing;
        match self.layout.direction {
            Direction::TopDown => self.cursor.y = rect.max.y + spacing.y,
            Direction::LeftToRight => {
                self.cursor.x = rect.max.x + spacing.x;
                self.row_height = self.row_height.max(rect.max.y - self.cursor.y);
            }
            Direction::RightToLeft => {
                self.cursor.x = rect.min.x - spacing.x;
                self.row_height = self.row_height.max(rect.max.y - self.cursor.y);
            }
        }
    }

    fn new_line(&mut self) {
        let height = self.row_height.max(self.style.spacing.interact_height);
        self.cursor.y += height + self.style.spacing.item_spacing.y;
        self.cursor.x = match self.layout.direction {
            Direction::RightToLeft => self.max_rect.max.x,
            _ => self.max_rect.min.x,
        };
        self.row_height = 0.0;
    }

    /// Adds empty space along the main direction.
    pub fn add_space(&mut self, amount: f32) {
        match self.layout.direction {
            Direction::TopDown => self.cursor.y += amount,
            Direction::LeftToRight => self.cursor.x += amount,
            Direction::RightToLeft => self.cursor.x -= amount,
        }
    }

    /// Ends the current row of a [`crate::Grid`], or starts a new line in
    /// a wrapping row. Does nothing otherwise.
    pub fn end_row(&mut self) {
        if let Some(grid) = &mut self.grid {
            grid.layout.end_row();
        } else if self.layout.is_horizontal() {
            self.new_line();
        }
    }

    pub(crate) fn set_grid(&mut self, layout: GridLayout, stripe_color: Color) {
        self.grid = Some(UiGrid {
            layout,
            stripe_color,
            painted_row: None,
        });
    }

    pub(crate) fn take_grid(&mut self) -> Option<GridLayout> {
        self.grid.take().map(|g| g.layout)
    }

    /// Paints the background stripe of a grid row before its first cell.
    fn paint_grid_stripe(&mut self) {
        let Some(grid) = &mut self.grid else { return };
        let row = grid.layout.row();
        if grid.layout.has_started_row() || grid.painted_row == Some(row) {
            return;
        }
        grid.painted_row = Some(row);
        if let Some(stripe) = grid.layout.stripe() {
            let color = grid.stripe_color;
            let radius = self.style.visuals.small_corner_radius;
            self.painter().rect_filled(stripe, radius, color);
        }
    }

    // ---- Child Uis ----

    /// Runs `add_contents` in a child Ui with `layout`, placed in
    /// `max_rect`, then advances this Ui past the space the child used.
    pub fn scope_with<R>(
        &mut self,
        max_rect: Rect,
        layout: Layout,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        self.paint_grid_stripe();
        let id = self.next_auto_id();
        // The child's size is only known after its content is added. To
        // align it (e.g. a row inside a centered column), use the size it
        // had in the previous frame, and run another frame if it changed.
        let prev_size: Option<Vec2> = self.ctx.data(id);
        let max_rect = self.align_child(max_rect, prev_size);
        let mut child = self.child_with_id(id, max_rect, layout);
        let inner = add_contents(&mut child);
        let used = child.min_rect();
        if prev_size != Some(used.size()) {
            self.ctx.insert_data(id, used.size());
            if self.needs_child_alignment() {
                self.ctx.request_repaint();
            }
        }
        self.advance_after_rect(used);
        let response = self.interact(id, used, Sense::HOVER);
        InnerResponse { inner, response }
    }

    /// Runs `add_contents` in a child Ui placed in `max_rect` without
    /// moving this Ui's cursor (the caller already reserved the space).
    /// The response covers the area the content used.
    pub fn scope_with_no_advance<R>(
        &mut self,
        max_rect: Rect,
        layout: Layout,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let mut child = self.child(max_rect, layout);
        let id = child.id;
        let inner = add_contents(&mut child);
        let used = child.min_rect();
        let response = self.interact(id, used, Sense::HOVER);
        InnerResponse { inner, response }
    }

    /// Whether children are aligned using their previous size.
    fn needs_child_alignment(&self) -> bool {
        self.grid.is_none() && !self.layout.cross_justify && self.layout.cross_align != Align::Min
    }

    /// Shifts a child's area along this Ui's cross axis so content of
    /// `size` ends up aligned like a widget would be.
    fn align_child(&self, max_rect: Rect, size: Option<Vec2>) -> Rect {
        let Some(size) = size.filter(|_| self.needs_child_alignment()) else {
            return max_rect;
        };
        let align = self.layout.cross_align;
        let mut rect = max_rect;
        if self.layout.is_horizontal() {
            let band = self.style.spacing.interact_height;
            rect.min.y += align.offset(band, size.y);
        } else {
            rect.min.x += align.offset(max_rect.width(), size.x);
        }
        rect
    }

    /// A child Ui with a different layout, filling the available space.
    pub fn with_layout<R>(
        &mut self,
        layout: Layout,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let rect = self.available_rect();
        self.scope_with(rect, layout, add_contents)
    }

    /// Widgets side by side, left to right, vertically centered.
    pub fn horizontal<R>(
        &mut self,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        self.with_layout(Layout::left_to_right(Align::Center), add_contents)
    }

    /// Like [`Ui::horizontal`], but continues on a new line when a widget
    /// doesn't fit (like words in a paragraph).
    pub fn horizontal_wrapped<R>(
        &mut self,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        self.with_layout(
            Layout::left_to_right(Align::Center).with_main_wrap(true),
            add_contents,
        )
    }

    /// Widgets stacked top to bottom (useful inside a horizontal Ui).
    pub fn vertical<R>(&mut self, add_contents: impl FnOnce(&mut Ui<'_>) -> R) -> InnerResponse<R> {
        self.with_layout(Layout::top_down(Align::Min), add_contents)
    }

    /// Widgets stacked top to bottom, each centered horizontally.
    pub fn vertical_centered<R>(
        &mut self,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        self.with_layout(Layout::top_down(Align::Center), add_contents)
    }

    /// Adds `widget` stretched to exactly `size` (e.g. equally sized buttons).
    pub fn add_sized(&mut self, size: Vec2, widget: impl Widget) -> Response {
        let rect = self.allocate_rect(size);
        let mut child = self.child(rect, Layout::centered_and_justified());
        widget.ui(&mut child)
    }

    // ---- Widgets ----

    /// Adds any widget.
    pub fn add(&mut self, widget: impl Widget) -> Response {
        widget.ui(self)
    }

    /// Text that wraps to the available width.
    pub fn label(&mut self, text: impl Into<String>) -> Response {
        self.add(Label::new(text))
    }

    /// Large, bold text.
    pub fn heading(&mut self, text: impl Into<String>) -> Response {
        let style = self.style.heading.clone();
        self.add(Label::new(text).style(style))
    }

    /// A clickable button.
    pub fn button(&mut self, text: impl Into<String>) -> Response {
        self.add(Button::new(text))
    }

    /// A checkbox toggling `checked`.
    pub fn checkbox(&mut self, checked: &mut bool, text: impl Into<String>) -> Response {
        self.add(Checkbox::new(checked, text))
    }

    /// A radio button; returns a response that is clicked when chosen.
    pub fn radio(&mut self, selected: bool, text: impl Into<String>) -> Response {
        self.add(RadioButton::new(selected, text))
    }

    /// A radio button that sets `current` to `value` when clicked.
    pub fn radio_value<V: PartialEq>(
        &mut self,
        current: &mut V,
        value: V,
        text: impl Into<String>,
    ) -> Response {
        let mut response = self.radio(*current == value, text);
        if response.clicked() && *current != value {
            *current = value;
            response.mark_changed();
        }
        response
    }

    /// Shows an image at one point per pixel (see [`crate::Image`] for options).
    pub fn image(&mut self, texture: &crate::TextureHandle) -> Response {
        self.add(crate::Image::new(texture))
    }

    /// A one-line text field editing `text`.
    pub fn text_edit_singleline(&mut self, text: &mut String) -> Response {
        self.add(crate::TextEdit::singleline(text))
    }

    /// A multi-line text field editing `text`, as wide as the available space.
    pub fn text_edit_multiline(&mut self, text: &mut String) -> Response {
        self.add(crate::TextEdit::multiline(text))
    }

    /// A slider editing `value` within `range` (see [`crate::Slider`] for options).
    pub fn slider<T: Numeric>(&mut self, value: &mut T, range: RangeInclusive<T>) -> Response {
        self.add(Slider::new(value, range))
    }

    /// A button that opens a popup menu below it. Clicking a button inside
    /// the menu, clicking outside it or pressing Escape closes it.
    pub fn menu_button<R>(
        &mut self,
        text: impl Into<String>,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<Option<R>> {
        let response = self.add(Button::new(text).menu_style(self.layout.is_horizontal()));
        let id = response.id;
        if response.clicked() {
            if self.ctx.is_popup_open(id) {
                self.ctx.close_popup();
            } else {
                self.ctx.open_popup(id);
            }
        }
        let inner = self
            .ctx
            .is_popup_open(id)
            .then(|| crate::popup::show_popup(self, id, response.rect, add_contents));
        InnerResponse { inner, response }
    }

    /// Closes the open popup menu (e.g. after choosing an item that is not
    /// a button).
    pub fn close_menu(&mut self) {
        self.ctx.close_popup();
    }

    /// A line across the Ui: horizontal in top-down layouts, vertical in rows.
    pub fn separator(&mut self) -> Response {
        self.add(Separator)
    }
}
