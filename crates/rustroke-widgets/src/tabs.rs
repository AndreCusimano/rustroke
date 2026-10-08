//! Tab bars: open documents, panel groups.

use std::hash::Hash;

use rustroke_core::{Point, Rect, Stroke, Vec2, point, vec2};
use rustroke_text::IconId;

use crate::{CursorIcon, Id, Response, Sense, Ui, WidgetInfo, WidgetRole};

/// Pointer movement (points) before pressing a tab becomes a drag.
pub(crate) const TAB_DRAG_THRESHOLD: f32 = 6.0;

/// How one tab looks.
#[derive(Clone, Debug, Default)]
pub struct TabLabel {
    /// The tab's name.
    pub title: String,
    /// An icon before the name.
    pub icon: Option<IconId>,
    /// Shows a dot instead of the close button while not hovered (unsaved
    /// changes).
    pub modified: bool,
}

impl TabLabel {
    /// A tab named `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }

    /// With an icon before the name.
    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Marked as modified (unsaved).
    pub fn modified(mut self, modified: bool) -> Self {
        self.modified = modified;
        self
    }
}

impl From<&str> for TabLabel {
    fn from(title: &str) -> Self {
        Self::new(title)
    }
}

impl From<String> for TabLabel {
    fn from(title: String) -> Self {
        Self::new(title)
    }
}

/// What the low-level tab strip reported.
#[derive(Debug, Default)]
pub(crate) struct TabStripOutput {
    pub tabs: Vec<Response>,
    pub clicked: Option<usize>,
    pub close: Option<usize>,
    pub add_clicked: bool,
    /// Tab being dragged past the threshold, with the pointer position.
    pub dragging: Option<(usize, Point)>,
    /// The drag of this tab ended this frame, at this position.
    pub dropped: Option<(usize, Point)>,
}

/// A tab drag in progress, kept between frames.
#[derive(Clone, Copy, Debug)]
struct TabDrag {
    index: usize,
    total: Vec2,
    moved: bool,
}

/// Draws a row of tabs in `rect` and reports interaction. Used by
/// [`TabBar`] and the dock area.
pub(crate) fn tab_strip(
    ui: &mut Ui<'_>,
    id: Id,
    rect: Rect,
    labels: &[TabLabel],
    active: Option<usize>,
    closable: bool,
    add_button: bool,
) -> TabStripOutput {
    let style = ui.style();
    let mut out = TabStripOutput::default();
    let pad_x = 10.0;
    let close_size = 16.0;
    let gap = 2.0;
    let height = rect.height();
    let radius = style.visuals.small_corner_radius;
    let drag_key = id.with("drag");
    let mut drag: Option<TabDrag> = ui.ctx().data(drag_key);

    let mut x = rect.min.x;
    for (i, label) in labels.iter().enumerate() {
        let galley = ui.layout_text(&label.title, &style.body, None);
        let icon = label.icon.and_then(|icon| ui.rasterize_icon(icon, 16.0));
        let icon_w = icon.as_ref().map_or(0.0, |i| i.size.x + 6.0);
        let close_w = if closable { close_size + 6.0 } else { 0.0 };
        let width = pad_x * 2.0 + icon_w + galley.size.x + close_w;
        let tab_rect = Rect::from_min_size(point(x, rect.min.y), vec2(width, height));
        x += width + gap;

        let tab_id = id.with(("tab", i));
        let tab = ui.interact(tab_id, tab_rect, Sense::POINTER_DRAG);
        // The close button is registered after the tab, so it wins.
        let close_rect = Rect::from_center_size(
            point(
                tab_rect.max.x - pad_x - close_size / 2.0,
                tab_rect.center().y,
            ),
            Vec2::splat(close_size),
        );
        let close = closable.then(|| ui.interact(tab_id.with("close"), close_rect, Sense::CLICK));
        let is_active = active == Some(i);
        ui.describe(
            &tab,
            WidgetInfo::new(WidgetRole::Tab, label.title.clone()).selected(is_active),
        );
        if let Some(close) = &close {
            ui.describe(
                close,
                WidgetInfo::new(WidgetRole::Button, format!("Close {}", label.title)),
            );
        }

        // Look: the active tab has the content's background and the accent
        // line; others show a background while hovered.
        let visuals = ui.widget_visuals(&tab);
        if is_active {
            ui.painter()
                .rect_filled(tab_rect, radius, style.visuals.window_fill);
            let line = Rect::from_min_max(
                point(tab_rect.min.x + radius, tab_rect.max.y - 2.0),
                point(tab_rect.max.x - radius, tab_rect.max.y),
            );
            ui.painter().rect_filled(line, 1.0, style.visuals.accent);
        } else if tab.hovered() {
            ui.painter().rect_filled(tab_rect, radius, visuals.bg_fill);
        }
        let fg = if is_active {
            style.visuals.text
        } else {
            style.visuals.weak_text.lerp(style.visuals.text, 0.5)
        };
        let mut text_x = tab_rect.min.x + pad_x;
        if let Some(icon) = &icon {
            let pos = point(text_x, tab_rect.center().y - icon.size.y / 2.0);
            ui.paint_icon(pos, icon, fg);
            text_x += icon_w;
        }
        let text_pos = point(text_x, tab_rect.center().y - galley.size.y / 2.0);
        ui.painter().galley(text_pos, galley, fg);
        if let Some(close) = &close {
            let show_cross = close.hovered() || tab.hovered() || is_active || !label.modified;
            if close.hovered() {
                let fill = ui.widget_visuals(close).bg_fill;
                ui.painter().rect_filled(close_rect, radius, fill);
            }
            if label.modified && !close.hovered() && !tab.hovered() {
                ui.painter().circle_filled(close_rect.center(), 3.5, fg);
            } else if show_cross {
                let c = close_rect.center();
                let d = 3.5;
                let stroke = Stroke::new(1.3, fg);
                ui.painter().line(c - vec2(d, d), c + vec2(d, d), stroke);
                ui.painter().line(c + vec2(-d, d), c + vec2(d, -d), stroke);
            }
            if close.clicked() {
                out.close = Some(i);
            }
        }
        if tab.middle_clicked() && closable {
            out.close = Some(i);
        }

        // Dragging, to reorder or (in a dock) to move the tab elsewhere.
        if tab.drag_started() {
            drag = Some(TabDrag {
                index: i,
                total: Vec2::ZERO,
                moved: false,
            });
        }
        if let Some(d) = &mut drag
            && d.index == i
        {
            if tab.dragged() {
                d.total += tab.drag_delta();
                d.moved |= d.total.x.abs().max(d.total.y.abs()) >= TAB_DRAG_THRESHOLD;
                if d.moved {
                    let pos = tab.interact_pointer_pos().unwrap_or(tab_rect.center());
                    out.dragging = Some((i, pos));
                    ui.ctx().set_cursor(CursorIcon::Grabbing);
                }
            }
            if tab.drag_stopped() {
                if d.moved {
                    let pos = tab.interact_pointer_pos().unwrap_or(tab_rect.center());
                    out.dropped = Some((i, pos));
                } else if out.close.is_none() {
                    out.clicked = Some(i);
                }
                drag = None;
            }
        }
        if tab.clicked() && out.clicked.is_none() && out.close.is_none() && drag.is_none() {
            out.clicked = Some(i);
        }
        out.tabs.push(tab);
    }

    if add_button {
        let size = Vec2::splat(height.min(24.0));
        let add_rect = Rect::from_min_size(point(x + 2.0, rect.center().y - size.y / 2.0), size);
        let add = ui.interact(id.with("add"), add_rect, Sense::CLICK);
        ui.describe(&add, WidgetInfo::new(WidgetRole::Button, "New tab"));
        let visuals = ui.widget_visuals(&add);
        if add.hovered() {
            ui.painter().rect_filled(add_rect, radius, visuals.bg_fill);
        }
        let c = add_rect.center();
        let stroke = Stroke::new(1.5, visuals.fg);
        ui.painter()
            .line(c - vec2(5.0, 0.0), c + vec2(5.0, 0.0), stroke);
        ui.painter()
            .line(c - vec2(0.0, 5.0), c + vec2(0.0, 5.0), stroke);
        out.add_clicked = add.clicked();
    }

    match drag {
        Some(d) => ui.ctx().insert_data(drag_key, d),
        None => ui.ctx().remove_data(drag_key),
    }
    out
}

/// Where a tab dropped at `x` goes among `tabs` (their rects): the index of
/// the gap.
pub(crate) fn insertion_index(tabs: &[Response], x: f32) -> usize {
    tabs.iter().filter(|t| t.rect.center().x < x).count()
}

/// What [`TabBar::show`] returns.
#[derive(Clone, Debug)]
pub struct TabBarResponse {
    /// Covers the bar.
    pub response: Response,
    /// One response per tab (e.g. for [`Response::context_menu`]).
    pub tabs: Vec<Response>,
    /// The tab clicked this frame (it is now the active one).
    pub clicked: Option<usize>,
    /// The close button (or a middle click) of this tab was clicked: remove
    /// it, or ask about unsaved changes first.
    pub close_requested: Option<usize>,
    /// The "+" button was clicked.
    pub add_clicked: bool,
    /// A tab was dragged from the first index to the second; the items
    /// have already been moved.
    pub moved: Option<(usize, usize)>,
}

/// A row of tabs, e.g. the open documents under the menu bar: click to
/// switch, drag to reorder, × (or middle click) to close, optional "+".
///
/// ```ignore
/// let bar = TabBar::new("documents").add_button(true).show(
///     ui,
///     &mut self.documents,
///     &mut self.active,
///     |doc| TabLabel::new(&doc.name).modified(doc.dirty),
/// );
/// if let Some(i) = bar.close_requested { self.documents.remove(i); }
/// ```
#[derive(Clone, Debug)]
pub struct TabBar {
    id_salt: Id,
    closable: bool,
    add_button: bool,
    height: Option<f32>,
}

impl TabBar {
    /// A tab bar identified by `id_salt` (unique within its Ui).
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            closable: true,
            add_button: false,
            height: None,
        }
    }

    /// Show close buttons (default `true`).
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Show a "+" button after the tabs.
    pub fn add_button(mut self, add: bool) -> Self {
        self.add_button = add;
        self
    }

    /// Height of the bar (default: the interaction height plus a little).
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Shows a tab per item, labelled by `label(item)`; `active` is the
    /// index of the current tab (kept valid when tabs move).
    pub fn show<T>(
        self,
        ui: &mut Ui<'_>,
        items: &mut Vec<T>,
        active: &mut usize,
        label: impl Fn(&T) -> TabLabel,
    ) -> TabBarResponse {
        let style = ui.style();
        let id = ui.id().with(self.id_salt);
        let height = self.height.unwrap_or(style.spacing.interact_height + 4.0);
        let width = ui.fill_width(400.0);
        let rect = ui.allocate_rect(vec2(width, height));
        let labels: Vec<TabLabel> = items.iter().map(&label).collect();
        let current = (!items.is_empty()).then(|| (*active).min(items.len() - 1));
        let out = tab_strip(
            ui,
            id,
            rect,
            &labels,
            current,
            self.closable,
            self.add_button,
        );
        let mut moved = None;
        if let Some(i) = out.clicked {
            *active = i;
        }
        if let Some((from, pos)) = out.dropped {
            let gap = insertion_index(&out.tabs, pos.x);
            let to = if gap > from { gap - 1 } else { gap };
            if to != from && from < items.len() {
                let item = items.remove(from);
                items.insert(to, item);
                *active = crate::list::moved_index(*active, from, to);
                moved = Some((from, to));
            }
        }
        if let Some((from, pos)) = out.dragging {
            // Insertion line while dragging.
            let gap = insertion_index(&out.tabs, pos.x);
            let x = if gap < out.tabs.len() {
                out.tabs[gap].rect.min.x - 1.0
            } else {
                out.tabs.last().map_or(rect.min.x, |t| t.rect.max.x + 1.0)
            };
            if gap != from && gap != from + 1 {
                ui.painter().line(
                    point(x, rect.min.y + 3.0),
                    point(x, rect.max.y - 3.0),
                    Stroke::new(2.0, style.visuals.accent),
                );
            }
        }
        let response = ui.interact(id, rect, Sense::HOVER);
        TabBarResponse {
            response,
            tabs: out.tabs,
            clicked: out.clicked,
            close_requested: out.close,
            add_clicked: out.add_clicked,
            moved,
        }
    }
}
