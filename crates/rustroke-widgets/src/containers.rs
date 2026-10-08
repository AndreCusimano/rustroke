//! Containers that own an area of the window: panels, floating windows
//! and scroll areas.

use std::hash::Hash;

use rustroke_core::{DisplayList, Point, Rect, Stroke, Vec2, point, vec2};
use rustroke_text::Fonts;

use crate::context::Order;
use crate::{
    Align, Context, CursorIcon, Id, InnerResponse, LayerId, Layout, Response, Sense, Ui, Visuals,
};

/// Something that can host top-level containers: gives access to the
/// [`Context`] and the [`Fonts`] (e.g. the platform's frame).
pub trait UiRoot {
    /// The context and fonts, borrowed together.
    fn parts(&mut self) -> (&mut Context, &mut Fonts);
}

impl UiRoot for (&mut Context, &mut Fonts) {
    fn parts(&mut self) -> (&mut Context, &mut Fonts) {
        (&mut *self.0, &mut *self.1)
    }
}

/// Paints the background of floating content (window, popup, tooltip):
/// a soft shadow, the fill and a border.
pub(crate) fn paint_floating_frame(painter: &mut DisplayList, rect: Rect, visuals: &Visuals) {
    let radius = visuals.window_corner_radius;
    for (spread, alpha) in [(6.0, 0.25), (3.0, 0.5), (1.0, 1.0)] {
        let shadow = visuals.shadow.with_alpha(visuals.shadow.a * alpha * 0.4);
        let r = Rect::from_min_max(
            rect.min + vec2(-spread, -spread + 3.0),
            rect.max + vec2(spread, spread + 3.0),
        );
        painter.rect_filled(r, radius + spread, shadow);
    }
    painter.rect(rect, radius, visuals.window_fill, visuals.window_stroke);
}

// ---------------------------------------------------------------------------
// Panels

/// Which edge of the window a [`Panel`] sticks to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelSide {
    /// The top edge.
    Top,
    /// The bottom edge.
    Bottom,
    /// The left edge.
    Left,
    /// The right edge.
    Right,
}

/// A bar along an edge of the window (toolbar, status bar, sidebar).
/// Panels take space away from what is left for the central panel, so
/// show them before [`CentralPanel`].
///
/// Top and bottom panels are as tall as their content; side panels have a
/// width that can be changed by dragging their inner edge.
#[derive(Clone, Debug)]
pub struct Panel {
    side: PanelSide,
    id: Id,
    default_size: Option<f32>,
    resizable: bool,
    auto_width: bool,
}

impl Panel {
    fn new(side: PanelSide, id_salt: impl Hash) -> Self {
        Self {
            side,
            id: Id::new("panel").with(id_salt),
            default_size: None,
            resizable: matches!(side, PanelSide::Left | PanelSide::Right),
            auto_width: false,
        }
    }

    /// A panel along the top edge (toolbar, menu bar).
    pub fn top(id_salt: impl Hash) -> Self {
        Self::new(PanelSide::Top, id_salt)
    }

    /// A panel along the bottom edge (status bar).
    pub fn bottom(id_salt: impl Hash) -> Self {
        Self::new(PanelSide::Bottom, id_salt)
    }

    /// A panel along the left edge (sidebar).
    pub fn left(id_salt: impl Hash) -> Self {
        Self::new(PanelSide::Left, id_salt)
    }

    /// A panel along the right edge (inspector).
    pub fn right(id_salt: impl Hash) -> Self {
        Self::new(PanelSide::Right, id_salt)
    }

    /// Initial width (side panels) or height (top/bottom, before the
    /// content has been measured).
    pub fn default_size(mut self, size: f32) -> Self {
        self.default_size = Some(size);
        self
    }

    /// A side panel exactly as wide as its content (text doesn't wrap),
    /// instead of a width the user drags. Turns off resizing.
    pub fn auto_width(mut self) -> Self {
        self.auto_width = true;
        self.resizable = false;
        self
    }

    /// Whether a side panel can be resized by dragging its edge.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Shows the panel and its content. Returns what `add_contents` returned.
    pub fn show<R>(
        self,
        root: &mut impl UiRoot,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let (ctx, fonts) = root.parts();
        let style = std::sync::Arc::clone(ctx.style());
        let pad = style.spacing.window_padding;
        let available = ctx.available_rect();
        let vertical_side = matches!(self.side, PanelSide::Left | PanelSide::Right);
        let default = self
            .default_size
            .unwrap_or(if vertical_side { 200.0 } else { 32.0 });
        let size: f32 = ctx.data(self.id).unwrap_or(default);

        let rect = match self.side {
            PanelSide::Top => Rect::from_min_size(available.min, vec2(available.width(), size)),
            PanelSide::Bottom => Rect::from_min_max(
                point(available.min.x, available.max.y - size),
                available.max,
            ),
            PanelSide::Left => Rect::from_min_size(available.min, vec2(size, available.height())),
            PanelSide::Right => Rect::from_min_max(
                point(available.max.x - size, available.min.y),
                available.max,
            ),
        };
        let remaining = match self.side {
            PanelSide::Top => Rect::from_min_max(point(available.min.x, rect.max.y), available.max),
            PanelSide::Bottom => {
                Rect::from_min_max(available.min, point(available.max.x, rect.min.y))
            }
            PanelSide::Left => {
                Rect::from_min_max(point(rect.max.x, available.min.y), available.max)
            }
            PanelSide::Right => {
                Rect::from_min_max(available.min, point(rect.min.x, available.max.y))
            }
        };

        let layer = LayerId::background();
        let (inner, response, new_size) = ctx.ui_in_layer(layer, self.id, rect, fonts, |ui| {
            let unclipped = ui.clip_rect();
            ui.set_clip_rect(rect);
            let visuals = &style.visuals;
            ui.painter().rect_filled(rect, 0.0, visuals.panel_fill);
            let (a, b) = match self.side {
                PanelSide::Top => (rect.left_bottom(), rect.right_bottom()),
                PanelSide::Bottom => (rect.left_top(), rect.right_top()),
                PanelSide::Left => (rect.right_top(), rect.right_bottom()),
                PanelSide::Right => (rect.left_top(), rect.left_bottom()),
            };
            ui.painter().line(a, b, visuals.window_stroke);

            // The resize grip straddles the edge, so it isn't clipped to the
            // panel. It is registered before the content so that widgets
            // under it win (hit testing prefers what was added last).
            let grip_response = (self.resizable && vertical_side).then(|| {
                let x = if self.side == PanelSide::Left {
                    rect.max.x
                } else {
                    rect.min.x
                };
                let grip =
                    Rect::from_min_max(point(x - 3.0, rect.min.y), point(x + 3.0, rect.max.y));
                ui.clip_rect_restore(unclipped);
                let r = ui.interact(self.id.with("resize"), grip, Sense::POINTER_DRAG);
                ui.set_clip_rect(rect);
                r
            });

            let auto_width = self.auto_width && vertical_side;
            let mut content_rect = rect.expand(-pad);
            if auto_width {
                // Measure the content at its natural width.
                content_rect.max.x = f32::INFINITY;
            }
            let content = ui.scope_with(content_rect, Layout::top_down(Align::Min), add_contents);
            let used = content.response.rect;
            let mut new_size = if auto_width {
                let width = if used.is_empty() {
                    0.0
                } else {
                    used.max.x - content_rect.min.x
                };
                (width + 2.0 * pad).min(available.width()).round()
            } else if vertical_side {
                size
            } else {
                used.height() + 2.0 * pad
            };

            ui.clip_rect_restore(unclipped);
            if let Some(r) = grip_response {
                if r.hovered() || r.dragged() {
                    ui.ctx().set_cursor(CursorIcon::ResizeHorizontal);
                    let color = visuals.accent;
                    ui.painter().line(a, b, Stroke::new(2.0, color));
                }
                if r.dragged() {
                    let dx = r.drag_delta().x;
                    new_size += if self.side == PanelSide::Left {
                        dx
                    } else {
                        -dx
                    };
                }
                new_size = new_size.clamp(60.0, (available.width() - 60.0).max(60.0));
            }
            (content.inner, content.response, new_size)
        });

        if new_size != size {
            ctx.insert_data(self.id, new_size);
            ctx.request_repaint();
        }
        ctx.set_available_rect(remaining);
        let mut response = response;
        response.rect = rect;
        InnerResponse { inner, response }
    }
}

/// The area left after all [`Panel`]s, with the style's window margin.
#[derive(Clone, Copy, Debug, Default)]
pub struct CentralPanel;

impl CentralPanel {
    /// Shows the central panel and its content.
    pub fn show<R>(self, root: &mut impl UiRoot, add_contents: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let (ctx, fonts) = root.parts();
        let rect = ctx.available_rect();
        let margin = ctx.style().spacing.window_margin;
        ctx.set_available_rect(Rect::from_min_size(rect.min, Vec2::ZERO));
        ctx.ui(rect.expand(-margin), fonts, |ui| {
            ui.set_clip_rect(rect);
            add_contents(ui)
        })
    }
}

// ---------------------------------------------------------------------------
// Windows

/// Position and size of a window, kept between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowState {
    pos: Point,
    width: f32,
    /// Size of the content in the previous frame.
    content_size: Vec2,
}

/// A floating window with a title bar: drag the title to move it, click
/// it to bring it to the front, drag the bottom-right corner to change
/// its width. Its height follows the content.
#[derive(Debug)]
pub struct Window<'open> {
    title: String,
    id: Id,
    open: Option<&'open mut bool>,
    default_pos: Option<Point>,
    default_width: f32,
    resizable: bool,
}

impl<'open> Window<'open> {
    /// The title also identifies the window; use [`Window::id_salt`] if
    /// two windows share a title.
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            id: Id::new("window").with(&title),
            title,
            open: None,
            default_pos: None,
            default_width: 280.0,
            resizable: true,
        }
    }

    /// Identifies the window by `salt` instead of its title.
    pub fn id_salt(mut self, salt: impl Hash) -> Self {
        self.id = Id::new("window").with(salt);
        self
    }

    /// Shows a close button that sets `open` to `false`. The window is
    /// only shown while `*open` is `true`.
    pub fn open(mut self, open: &'open mut bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Where the window first appears (afterwards it stays where the user moves it).
    pub fn default_pos(mut self, pos: Point) -> Self {
        self.default_pos = Some(pos);
        self
    }

    /// Width when first shown, in points.
    pub fn default_width(mut self, width: f32) -> Self {
        self.default_width = width;
        self
    }

    /// Whether the width can be changed by dragging the bottom-right corner.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Returns `None` if the window is closed.
    pub fn show<R>(
        self,
        root: &mut impl UiRoot,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> Option<InnerResponse<R>> {
        if self.open.as_deref() == Some(&false) {
            return None;
        }
        let (ctx, fonts) = root.parts();
        let style = std::sync::Arc::clone(ctx.style());
        let visuals = &style.visuals;
        let pad = style.spacing.window_padding;
        let title_height = style.spacing.interact_height + 4.0;
        let screen = ctx.input().screen_rect;

        let mut state: WindowState = ctx.data(self.id).unwrap_or(WindowState {
            pos: self.default_pos.unwrap_or(point(60.0, 60.0)),
            width: self.default_width,
            content_size: Vec2::ZERO,
        });
        // Keep the title bar reachable.
        state.pos.x = state
            .pos
            .x
            .clamp(screen.min.x - state.width + 60.0, screen.max.x - 60.0);
        state.pos.y = state.pos.y.clamp(screen.min.y, screen.max.y - title_height);
        let height = title_height + state.content_size.y + 2.0 * pad;
        let rect = Rect::from_min_size(state.pos, vec2(state.width, height));

        let layer = LayerId::new(Order::Middle, self.id);
        let mut close = false;
        let (inner, content_rect) = ctx.ui_in_layer(layer, self.id, rect, fonts, |ui| {
            // The background blocks clicks to what is below the window.
            let _ = ui.interact(self.id.with("background"), rect, Sense::POINTER_DRAG);
            paint_floating_frame(ui.painter(), rect, visuals);

            // Title bar: drag to move.
            let title_rect = Rect::from_min_size(rect.min, vec2(rect.width(), title_height));
            let title = ui.interact(self.id.with("title"), title_rect, Sense::POINTER_DRAG);
            if title.dragged() {
                state.pos += title.drag_delta();
                ui.ctx().set_cursor(CursorIcon::Grabbing);
            }
            let galley = ui.layout_text(&self.title, &style.body.clone().bold(), None);
            let text_pos = point(
                rect.min.x + pad,
                title_rect.center().y - galley.size.y / 2.0,
            );
            ui.painter().galley(text_pos, galley, visuals.text);
            ui.painter().line(
                point(rect.min.x, title_rect.max.y),
                point(rect.max.x, title_rect.max.y),
                visuals.window_stroke,
            );

            if self.open.is_some() {
                let side = title_height - 12.0;
                let button = Rect::from_center_size(
                    point(rect.max.x - pad - side / 2.0, title_rect.center().y),
                    Vec2::splat(side),
                );
                let r = ui.interact(self.id.with("close"), button, Sense::CLICK);
                let w = ui.widget_visuals(&r);
                if r.hovered() {
                    ui.painter()
                        .rect_filled(button, visuals.small_corner_radius, w.bg_fill);
                }
                let c = button.center();
                let d = side * 0.22;
                ui.painter()
                    .line(c + vec2(-d, -d), c + vec2(d, d), Stroke::new(1.5, w.fg));
                ui.painter()
                    .line(c + vec2(-d, d), c + vec2(d, -d), Stroke::new(1.5, w.fg));
                close = r.clicked();
            }

            // Resize grip in the bottom-right corner, registered before the
            // content so that widgets reaching into the corner win.
            let grip_response = self.resizable.then(|| {
                let grip = Rect::from_min_max(rect.max - Vec2::splat(14.0), rect.max);
                ui.interact(self.id.with("resize"), grip, Sense::POINTER_DRAG)
            });

            // Content.
            ui.set_clip_rect(rect);
            let content_max = Rect::from_min_max(
                point(rect.min.x + pad, title_rect.max.y + pad),
                point(rect.max.x - pad, screen.max.y.max(rect.max.y)),
            );
            let content = ui.scope_with(content_max, Layout::top_down(Align::Min), add_contents);

            if let Some(r) = grip_response {
                if r.hovered() || r.dragged() {
                    ui.ctx().set_cursor(CursorIcon::ResizeNwSe);
                }
                if r.dragged() {
                    state.width = (state.width + r.drag_delta().x).max(120.0);
                }
                let color = if r.hovered() || r.dragged() {
                    visuals.accent
                } else {
                    visuals.weak_text
                };
                for i in 1..=2 {
                    let o = 4.0 * i as f32;
                    ui.painter().line(
                        rect.max - vec2(o + 2.0, 3.0),
                        rect.max - vec2(3.0, o + 2.0),
                        Stroke::new(1.0, color),
                    );
                }
            }
            (content.inner, content.response)
        });

        let new_state = WindowState {
            content_size: content_rect.rect.size(),
            ..state
        };
        let old: Option<WindowState> = ctx.data(self.id);
        if old != Some(new_state) {
            ctx.insert_data(self.id, new_state);
            ctx.request_repaint();
        }
        if close && let Some(open) = self.open {
            *open = false;
        }
        let mut response: Response = content_rect;
        response.rect = rect;
        Some(InnerResponse { inner, response })
    }
}

// ---------------------------------------------------------------------------
// Scroll areas

/// Scroll position and content size, kept between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ScrollState {
    offset: Vec2,
    /// Size of the content last frame (`None` before the first frame).
    content_size: Option<Vec2>,
}

/// A scrolling area: content larger than the area is clipped and can be
/// scrolled with the mouse wheel (Shift+wheel, or a trackpad, sideways) or
/// by dragging the scroll bars. Vertical by default; see
/// [`ScrollArea::horizontal`] and [`ScrollArea::both`].
///
/// In a horizontally scrolling area the content has unlimited width: text
/// doesn't wrap and widgets that would fill the width use their natural
/// size instead.
#[derive(Clone, Debug)]
pub struct ScrollArea {
    id_salt: Option<Id>,
    /// Scrolls along x, y.
    enabled: [bool; 2],
    max_size: Vec2,
    auto_shrink: bool,
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self::vertical()
    }
}

impl ScrollArea {
    fn new(enabled: [bool; 2]) -> Self {
        Self {
            id_salt: None,
            enabled,
            max_size: Vec2::splat(f32::INFINITY),
            auto_shrink: true,
        }
    }

    /// A vertically scrolling area.
    pub fn vertical() -> Self {
        Self::new([false, true])
    }

    /// A horizontally scrolling area (e.g. a wide table or a timeline).
    pub fn horizontal() -> Self {
        Self::new([true, false])
    }

    /// An area scrolling in both directions.
    pub fn both() -> Self {
        Self::new([true, true])
    }

    /// The area is at most this tall (it is also limited by the space
    /// available in the parent).
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_size.y = height;
        self
    }

    /// The area is at most this wide (it is also limited by the space
    /// available in the parent).
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_size.x = width;
        self
    }

    /// Shrink to the content when it is smaller than the maximum size
    /// along the scrolling directions (default `true`). With `false` the
    /// area always takes the maximum.
    pub fn auto_shrink(mut self, shrink: bool) -> Self {
        self.auto_shrink = shrink;
        self
    }

    /// Identifies the area (to keep its scroll position) when the default,
    /// position-based id is not stable.
    pub fn id_salt(mut self, salt: impl Hash) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    /// Shows the area with its content, scrolled as the user left it.
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
        let bar = style.spacing.scrollbar_width;
        let gap = 4.0;
        let [scroll_x, scroll_y] = self.enabled;
        let mut state: ScrollState = ui.ctx().data(id).unwrap_or_default();
        let prev_content = state.content_size;

        // Which bars are needed, from last frame's content size.
        let available = ui.available_rect();
        // Along a direction that doesn't scroll the area is as big as its
        // content (like any widget), even beyond the available space: a
        // horizontal area in a panel sized by its content must not start
        // at zero height.
        let max = vec2(
            self.max_size.x.min(available.width()),
            if scroll_y {
                self.max_size.y.min(available.height())
            } else {
                self.max_size.y
            },
        );
        let overflows = |content: Option<Vec2>, axis: usize, room: f32| {
            content.is_some_and(|c| [c.x, c.y][axis] > room + 0.5)
        };
        let show_y = scroll_y && overflows(prev_content, 1, max.y);
        let show_x = scroll_x
            && overflows(
                prev_content,
                0,
                max.x - if show_y { bar + gap } else { 0.0 },
            );
        let bar_room = vec2(
            if show_y { bar + gap } else { 0.0 },
            if show_x { bar + gap } else { 0.0 },
        );

        // The viewport: the maximum size, shrunk to the content along the
        // scrolling directions. Non-scrolling directions keep the old
        // behavior: full width, content height.
        let mut size = max;
        if let Some(content) = prev_content {
            if !scroll_y || self.auto_shrink {
                size.y = size.y.min(content.y + bar_room.y);
            }
            if scroll_x && self.auto_shrink {
                size.x = size.x.min(content.x + bar_room.x);
            }
        } else if !scroll_y {
            size.y = 0.0;
        }
        let viewport = ui.allocate_rect(vec2(size.x.max(0.0), size.y.max(0.0)));
        let inner = Rect::from_min_max(viewport.min, viewport.max - bar_room);
        let max_offset = |content: Vec2| {
            vec2(
                (content.x - inner.width()).max(0.0),
                (content.y - inner.height()).max(0.0),
            )
        };
        let clamp = |offset: Vec2, content: Vec2| {
            let m = max_offset(content);
            vec2(offset.x.clamp(0.0, m.x), offset.y.clamp(0.0, m.y))
        };
        state.offset = clamp(state.offset, prev_content.unwrap_or(Vec2::ZERO));

        // Content, shifted by the scroll offset and clipped to the viewport.
        let origin = inner.min - state.offset;
        let content_max = Rect::from_min_max(
            origin,
            point(
                if scroll_x { f32::INFINITY } else { inner.max.x },
                f32::INFINITY,
            ),
        );
        let saved_clip = ui.clip_rect();
        ui.set_clip_rect(inner);
        let content =
            ui.scope_with_no_advance(content_max, Layout::top_down(Align::Min), add_contents);
        let used = content.response.rect;
        let content_size = if used.is_empty() {
            Vec2::ZERO
        } else {
            used.max - origin
        };
        ui.clip_rect_restore(saved_clip);
        ui.set_clip_rect(viewport);

        // Mouse wheel, unless a nested area already used it. A vertical
        // wheel scrolls sideways in horizontal-only areas or with Shift.
        let area = ui.interact(id.with("area"), viewport, Sense::HOVER);
        let mut offset = state.offset;
        if area.hovered() {
            let mut delta = ui.input().scroll_delta;
            if scroll_x && (!scroll_y || ui.input().modifiers.shift) && delta.x == 0.0 {
                delta = vec2(delta.y, 0.0);
            }
            let used_x = if scroll_x { delta.x } else { 0.0 };
            let used_y = if scroll_y { delta.y } else { 0.0 };
            offset -= vec2(used_x, used_y);
            let input = ui.ctx().input_mut();
            if used_x != 0.0 {
                input.scroll_delta.x = 0.0;
                if !scroll_y {
                    input.scroll_delta.y = 0.0;
                }
            }
            if used_y != 0.0 {
                input.scroll_delta.y = 0.0;
            }
        }

        // Scroll bars.
        let max_off = max_offset(content_size);
        if show_y && content_size.y > inner.height() {
            let track = Rect::from_min_max(
                point(viewport.max.x - bar, viewport.min.y),
                point(viewport.max.x, inner.max.y),
            );
            offset.y = scroll_bar(
                ui,
                id.with("bar"),
                track,
                1,
                inner.height(),
                content_size.y,
                offset.y,
                max_off.y,
            );
        }
        if show_x && content_size.x > inner.width() {
            let track = Rect::from_min_max(
                point(viewport.min.x, viewport.max.y - bar),
                point(inner.max.x, viewport.max.y),
            );
            offset.x = scroll_bar(
                ui,
                id.with("hbar"),
                track,
                0,
                inner.width(),
                content_size.x,
                offset.x,
                max_off.x,
            );
        }
        ui.clip_rect_restore(saved_clip);

        let new_state = ScrollState {
            offset: clamp(offset, content_size),
            content_size: Some(content_size),
        };
        if new_state != state || prev_content != Some(content_size) {
            ui.ctx().insert_data(id, new_state);
            ui.ctx().request_repaint();
        }
        let mut response = area;
        response.rect = viewport;
        InnerResponse {
            inner: content.inner,
            response,
        }
    }
}

/// Draws a scroll bar along `axis` (0 = x, 1 = y) in `track` and returns
/// the offset after dragging its thumb.
#[allow(clippy::too_many_arguments)]
fn scroll_bar(
    ui: &mut Ui<'_>,
    id: Id,
    track: Rect,
    axis: usize,
    visible: f32,
    content: f32,
    offset: f32,
    max_offset: f32,
) -> f32 {
    let style = ui.style();
    let len = |v: Vec2| if axis == 0 { v.x } else { v.y };
    let track_len = len(track.size());
    let thumb_len = (track_len * visible / content).max(20.0).min(track_len);
    let travel = track_len - thumb_len;
    let t = if max_offset > 0.0 {
        (offset / max_offset).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb = if axis == 0 {
        Rect::from_min_size(
            point(track.min.x + travel * t, track.min.y),
            vec2(thumb_len, track.height()),
        )
    } else {
        Rect::from_min_size(
            point(track.min.x, track.min.y + travel * t),
            vec2(track.width(), thumb_len),
        )
    };
    let r = ui.interact(id, thumb, Sense::POINTER_DRAG);
    let mut offset = offset;
    if r.dragged() && travel > 0.0 {
        offset += len(r.drag_delta()) / travel * max_offset;
    }
    let color = if r.hovered() || r.dragged() {
        style.visuals.active.stroke.color
    } else {
        ui.widget_visuals(&r).bg_fill
    };
    let radius = style.spacing.scrollbar_width / 2.0;
    ui.painter().rect_filled(thumb, radius, color);
    offset
}
