//! Sections that open and close, and trees made of them.

use std::hash::Hash;

use rustroke_core::{Key, Modifiers, Point, Rect, Stroke, Vec2, point, vec2};

use crate::{Align, Id, Layout, Response, Sense, Ui, WidgetInfo, WidgetRole};

/// What [`CollapsingHeader::show`] returns.
#[derive(Clone, Debug)]
pub struct CollapsingResponse<R> {
    /// The header. With [`CollapsingHeader::selected`], it is clicked
    /// when the label is clicked (to select the node), not when the
    /// section is opened or closed.
    pub header_response: Response,
    /// What the body returned, if it was shown.
    pub body_returned: Option<R>,
    /// Whether the section is open (it may still be animating).
    pub open: bool,
}

/// A header with a triangle that shows or hides the content below it.
/// Nest them for a tree.
///
/// ```ignore
/// CollapsingHeader::new("Parameters").default_open(true).show(ui, |ui| {
///     ui.add(DragValue::new(&mut width).suffix(" mm"));
/// });
/// ```
///
/// The open state is remembered between frames, keyed by the text (or
/// [`CollapsingHeader::id_salt`]). Keyboard: Enter/Space toggle, right and
/// left arrows open and close.
#[derive(Clone, Debug)]
pub struct CollapsingHeader {
    text: String,
    id_salt: Id,
    default_open: bool,
    open: Option<bool>,
    selected: Option<bool>,
}

impl CollapsingHeader {
    /// A closed section titled `text`.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            id_salt: Id::new(&text),
            text,
            default_open: false,
            open: None,
            selected: None,
        }
    }

    /// Open the first time it is shown.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Identifies the section instead of its text (needed when two
    /// sections in the same Ui have the same text, or the text changes).
    pub fn id_salt(mut self, salt: impl Hash) -> Self {
        self.id_salt = Id::new(salt);
        self
    }

    /// Opens or closes the section from the app (`Some`), e.g. "expand
    /// all". `None` leaves it to the user.
    pub fn open(mut self, open: Option<bool>) -> Self {
        self.open = open;
        self
    }

    /// Tree-node style: only the triangle opens and closes the section;
    /// clicking the label clicks the header response (to select the
    /// node), which is highlighted when `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Shows the header and, while open, the body (indented).
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_body: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> CollapsingResponse<R> {
        let style = ui.style();
        let id = ui.id().with(self.id_salt);
        let open_key = id.with("open");
        let mut open = ui.ctx().data(open_key).unwrap_or(self.default_open);
        if let Some(forced) = self.open {
            open = forced;
        }

        // Header: triangle, then the text.
        let icon = style.spacing.icon_size;
        let galley = ui.layout_text(&self.text, &style.body, None);
        let height = style.spacing.interact_height;
        let width = if self.selected.is_some() {
            icon + style.spacing.icon_spacing + galley.size.x + 8.0
        } else {
            ui.available_width()
        };
        let rect = ui.allocate_rect(vec2(width, height));
        let toggle_rect = Rect::from_min_size(rect.min, vec2(icon, height));
        let (toggle, mut header) = if self.selected.is_some() {
            let toggle = ui.interact(id.with("toggle"), toggle_rect, Sense::POINTER_DRAG);
            let label_rect = Rect::from_min_max(point(toggle_rect.max.x, rect.min.y), rect.max);
            let header = ui.interact(id, label_rect, Sense::CLICK);
            (toggle.clicked(), header)
        } else {
            let header = ui.interact(id, rect, Sense::CLICK);
            (header.clicked(), header)
        };
        let mut toggled = toggle;
        if header.has_focus() {
            let input = ui.ctx().input_mut();
            let key = if open {
                Key::ArrowLeft
            } else {
                Key::ArrowRight
            };
            if input.consume_key(key, Modifiers::NONE) {
                toggled = true;
            }
        }
        if toggled {
            open = !open;
            header.mark_changed();
        }
        ui.ctx().insert_data(open_key, open);
        ui.describe(
            &header,
            WidgetInfo::new(WidgetRole::CollapsingHeader, self.text.clone()).expanded(open),
        );

        let openness = ui.ctx().animate_bool(id.with("openness"), open);
        let visuals = ui.widget_visuals(&header);
        let radius = style.visuals.small_corner_radius;
        let highlight = header.rect;
        if self.selected == Some(true) {
            ui.painter()
                .rect_filled(highlight, radius, style.visuals.selection);
        } else if header.hovered() || header.is_pressed() {
            ui.painter().rect_filled(highlight, radius, visuals.bg_fill);
        }
        paint_triangle(ui, toggle_rect.center(), icon * 0.3, openness, visuals.fg);
        let text_x = toggle_rect.max.x + 4.0;
        ui.painter().galley(
            point(text_x, rect.center().y - galley.size.y / 2.0),
            galley,
            visuals.fg,
        );
        if header.focus_visible() {
            ui.painter().rect_stroke(
                highlight.expand(2.0),
                radius + 2.0,
                Stroke::new(2.0, style.visuals.focus),
            );
        }

        // Body: slides open and closed.
        let body_returned = (openness > 0.0).then(|| {
            let height_key = id.with("body height");
            let full: f32 = ui.ctx().data(height_key).unwrap_or(0.0);
            let indent = style.spacing.indent;
            let mut area = ui.available_rect();
            area.min.x += indent;
            let visible = if openness < 1.0 {
                full * openness
            } else {
                f32::INFINITY
            };
            let saved_clip = ui.clip_rect();
            let clip = Rect::from_min_max(
                point(saved_clip.min.x, area.min.y),
                point(saved_clip.max.x, area.min.y + visible),
            );
            ui.set_clip_rect(saved_clip.intersect(clip));
            // A stable id, so widgets after the section keep theirs when
            // it opens or closes.
            let inner = ui.push_id(self.id_salt, |ui| {
                ui.scope_with_no_advance(area, Layout::top_down(Align::Min), add_body)
            });
            ui.clip_rect_restore(saved_clip);
            let used = inner.response.rect;
            let used_height = if used.is_empty() { 0.0 } else { used.height() };
            if used_height != full {
                ui.ctx().insert_data(height_key, used_height);
            }
            let shown = used_height.min(visible);
            if shown > 0.0 {
                let width = if used.is_empty() { 0.0 } else { used.width() };
                ui.allocate_rect(vec2(width + indent, shown));
            }
            inner.inner
        });

        CollapsingResponse {
            header_response: header,
            body_returned: body_returned.filter(|_| open),
            open,
        }
    }
}

/// A filled triangle pointing right (`openness` 0) or down (1).
fn paint_triangle(
    ui: &mut Ui<'_>,
    center: Point,
    size: f32,
    openness: f32,
    color: rustroke_core::Color,
) {
    let angle = openness * std::f32::consts::FRAC_PI_2;
    let (sin, cos) = angle.sin_cos();
    let rotate = |v: Vec2| center + vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos) * size;
    let points = vec![
        rotate(vec2(-0.6, -0.9)),
        rotate(vec2(0.9, 0.0)),
        rotate(vec2(-0.6, 0.9)),
    ];
    ui.painter().polygon(points, color, Stroke::NONE);
}

impl Ui<'_> {
    /// A collapsible section titled `heading`, closed at first. See
    /// [`CollapsingHeader`] for options.
    pub fn collapsing<R>(
        &mut self,
        heading: impl Into<String>,
        add_body: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> CollapsingResponse<R> {
        CollapsingHeader::new(heading).show(self, add_body)
    }

    /// Runs `add_contents` shifted right by the style's indent (nested
    /// content, tree levels).
    pub fn indent<R>(&mut self, add_contents: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let indent = self.style().spacing.indent;
        let mut area = self.available_rect();
        area.min.x += indent;
        self.scope_with(area, Layout::top_down(Align::Min), add_contents)
            .inner
    }
}
