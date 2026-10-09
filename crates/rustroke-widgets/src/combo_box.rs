//! Drop-down list.

use std::hash::Hash;

use rustroke_core::{Color, Rect, Stroke, Vec2, point, vec2};

use crate::popup::show_popup;
use crate::widgets::{FrameOverride, frame_setters};
use crate::{Id, InnerResponse, Sense, Ui, WidgetInfo, WidgetRole};

/// A box showing the current choice; clicking it opens a list of options
/// below it.
///
/// ```ignore
/// ComboBox::from_label("Axis")
///     .selected_text(format!("{axis:?}"))
///     .show_ui(ui, |ui| {
///         ui.selectable_value(&mut axis, Axis::X, "X");
///         ui.selectable_value(&mut axis, Axis::Y, "Y");
///         ui.selectable_value(&mut axis, Axis::Z, "Z");
///     });
/// ```
///
/// Choosing an option ([`crate::SelectableLabel`], radio button or
/// button) closes the list, as do Escape and clicks elsewhere.
#[derive(Clone, Debug)]
pub struct ComboBox {
    id_salt: Id,
    label: Option<String>,
    selected_text: String,
    width: Option<f32>,
    frame_style: FrameOverride,
}

impl ComboBox {
    /// A combo box with `label` after it. The label also identifies the
    /// combo box, so it must be unique within its Ui.
    pub fn from_label(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            id_salt: Id::new(&label),
            label: Some(label),
            selected_text: String::new(),
            width: None,
            frame_style: FrameOverride::default(),
        }
    }

    /// A combo box without a label, identified by `salt` (unique within
    /// its Ui).
    pub fn from_id_salt(salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(salt),
            label: None,
            selected_text: String::new(),
            width: None,
            frame_style: FrameOverride::default(),
        }
    }

    /// The text shown in the box: usually the name of the current choice.
    pub fn selected_text(mut self, text: impl Into<String>) -> Self {
        self.selected_text = text.into();
        self
    }

    frame_setters!();

    /// Width of the box in points (default: the slider width). The list
    /// is at least as wide.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Shows the box and, while it is open, the list filled by
    /// `add_contents`. `inner` is what `add_contents` returned, or `None`
    /// when the list is closed. The response is clicked when the box is.
    pub fn show_ui<R>(
        self,
        ui: &mut Ui<'_>,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<Option<R>> {
        let style = ui.style();
        let padding = style.spacing.button_padding;
        let height = self
            .frame_style
            .min_size
            .map_or(style.spacing.interact_height, |m| m.y);
        let id = ui.id().with(self.id_salt);
        let label = self
            .label
            .as_ref()
            .map(|l| ui.layout_text(l, &style.body, None));
        let label_width = label
            .as_ref()
            .map_or(0.0, |g| style.spacing.icon_spacing + g.size.x);
        let width = self
            .width
            .unwrap_or(style.spacing.slider_width)
            .max(self.frame_style.min_size.map_or(0.0, |m| m.x))
            .min(ui.available_width() - label_width)
            .max(2.0 * height);

        let rect = ui.allocate_rect(vec2(width + label_width, height));
        let box_rect = Rect::from_min_size(rect.min, vec2(width, height));
        let mut response = ui.interact(id, box_rect, Sense::CLICK);
        if response.clicked() {
            if ui.ctx().is_popup_open(id) {
                ui.ctx().close_popup();
            } else {
                ui.ctx().open_popup(id);
            }
        }
        let open = ui.ctx().is_popup_open(id);
        ui.describe(
            &response,
            WidgetInfo::new(
                WidgetRole::ComboBox,
                self.label
                    .clone()
                    .unwrap_or_else(|| self.selected_text.clone()),
            )
            .value(self.selected_text.clone())
            .expanded(open),
        );

        // The box: current choice on the left, a chevron on the right.
        let visuals = ui.widget_visuals(&response);
        let custom = self.frame_style;
        let radius = custom.corner_radius(style.visuals.corner_radius);
        let fill = custom.fill(&response, visuals.bg_fill, style.visuals.text);
        ui.painter()
            .rect(box_rect, radius, fill, custom.stroke(visuals.stroke));
        let arrow = 4.0;
        let arrow_center = point(box_rect.max.x - padding.x - arrow, box_rect.center().y);
        let text_rect = Rect::from_min_max(
            box_rect.min + vec2(padding.x, 0.0),
            point(arrow_center.x - arrow - 4.0, box_rect.max.y),
        );
        let galley = ui.layout_text(&self.selected_text, &style.body, None);
        let saved_clip = ui.clip_rect();
        ui.set_clip_rect(saved_clip.intersect(text_rect));
        let text_pos = point(text_rect.min.x, box_rect.center().y - galley.size.y / 2.0);
        ui.painter().galley(text_pos, galley, visuals.fg);
        ui.clip_rect_restore(saved_clip);
        let flip = if open { -1.0 } else { 1.0 };
        let half = vec2(arrow, arrow / 2.0 * flip);
        ui.painter().polyline(
            vec![
                arrow_center - half,
                arrow_center + vec2(0.0, half.y),
                arrow_center + vec2(half.x, -half.y),
            ],
            Stroke::new(1.5, visuals.fg),
        );
        if let Some(label) = label {
            let pos = point(
                box_rect.max.x + style.spacing.icon_spacing,
                rect.center().y - label.size.y / 2.0,
            );
            ui.painter().galley(pos, label, style.visuals.text);
        }
        if response.focus_visible() {
            ui.painter().rect_stroke(
                box_rect.expand(3.0),
                radius + 3.0,
                Stroke::new(2.0, style.visuals.focus),
            );
        }

        let inner = open.then(|| show_popup(ui, id, box_rect, box_rect.width(), add_contents));
        response.rect = rect;
        InnerResponse { inner, response }
    }
}
