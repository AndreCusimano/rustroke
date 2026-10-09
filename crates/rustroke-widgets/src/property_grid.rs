//! Property panels: label / value rows in collapsible sections.

use std::hash::Hash;

use rustroke_core::{Rect, Stroke, Vec2, point, vec2};
use rustroke_text::IconId;

use crate::{
    Align, CollapsingHeader, CursorIcon, Id, Layout, Response, Sense, Ui, Widget, WidgetInfo,
    WidgetRole,
};

/// A two-column panel of properties: names on the left, aligned across
/// all sections, editable values on the right, grouped in collapsible
/// sections under an optional header with the object's icon and name.
///
/// ```ignore
/// PropertyGrid::new("inspector")
///     .header(Some(icons.part), "Base plate")
///     .show(ui, |grid| {
///         grid.section("General", true, |grid| {
///             grid.row("Name", |ui| ui.text_edit_singleline(&mut name));
///             grid.row("Visible", |ui| ui.checkbox(&mut visible, ""));
///         });
///         grid.section("Parameters", true, |grid| {
///             grid.row("Length", |ui| ui.add(DragValue::new(&mut length).suffix(" mm")));
///             grid.row("Sketch", |ui| ui.add(ReferenceField::new(sketch.as_deref())));
///         });
///     });
/// ```
#[derive(Clone, Debug)]
pub struct PropertyGrid {
    id_salt: Id,
    header: Option<(Option<IconId>, String)>,
    min_label_width: f32,
}

/// Adds sections and rows to a [`PropertyGrid`].
pub struct PropertyGridUi<'u, 'a> {
    ui: &'u mut Ui<'a>,
    id: Id,
    /// Width of the name column (widest name of the last frame).
    label_width: f32,
    /// Widest name seen this frame.
    widest: f32,
}

impl std::fmt::Debug for PropertyGridUi<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyGridUi")
            .field("label_width", &self.label_width)
            .finish_non_exhaustive()
    }
}

impl PropertyGrid {
    /// A property grid identified by `id_salt` (unique within its Ui).
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            header: None,
            min_label_width: 80.0,
        }
    }

    /// A header above the sections: the object's icon and name.
    pub fn header(mut self, icon: Option<IconId>, name: impl Into<String>) -> Self {
        self.header = Some((icon, name.into()));
        self
    }

    /// The name column is at least this wide (default 80 points).
    pub fn min_label_width(mut self, width: f32) -> Self {
        self.min_label_width = width;
        self
    }

    /// Shows the grid; `add_contents` adds sections and rows.
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_contents: impl FnOnce(&mut PropertyGridUi<'_, '_>) -> R,
    ) -> R {
        let id = ui.id().with(self.id_salt);
        if let Some((icon, name)) = &self.header {
            ui.horizontal(|ui| {
                if let Some(icon) = icon {
                    ui.add(crate::Icon::new(*icon).size(20.0));
                }
                let style = ui.style();
                let text = style.body.clone().weight(600);
                ui.add(
                    crate::Label::new(name.clone())
                        .style(text.clone())
                        .no_wrap(),
                );
            });
            ui.separator();
        }
        let width_key = id.with("label width");
        let label_width: f32 = ui.ctx().data(width_key).unwrap_or(self.min_label_width);
        let mut grid = PropertyGridUi {
            ui,
            id,
            label_width,
            widest: self.min_label_width,
        };
        let inner = add_contents(&mut grid);
        let widest = grid.widest;
        if widest != label_width {
            ui.ctx().insert_data(width_key, widest);
            ui.ctx().request_repaint();
        }
        inner
    }
}

impl<'a> PropertyGridUi<'_, 'a> {
    /// The Ui the grid is drawn in, e.g. for a note between rows.
    pub fn ui(&mut self) -> &mut Ui<'a> {
        self.ui
    }

    /// Width left for values in a row, e.g. for `TextEdit::desired_width`.
    pub fn value_width(&self) -> f32 {
        let spacing = self.ui.style().spacing.item_spacing.x;
        (self.ui.available_width() - self.label_width - spacing).max(40.0)
    }

    /// A collapsible section of rows, open at first if `default_open`.
    pub fn section<R>(
        &mut self,
        title: impl Into<String>,
        default_open: bool,
        add_rows: impl FnOnce(&mut PropertyGridUi<'_, '_>) -> R,
    ) -> Option<R> {
        let title = title.into();
        let id = self.id;
        let label_width = self.label_width;
        let mut widest = self.widest;
        let style = self.ui.style();
        let saved = self.ui.style().spacing.indent;
        // Rows line up with the header text, not further right.
        self.ui.style_mut().spacing.indent = 0.0;
        let heading = style.body.clone().weight(600);
        let response = CollapsingHeader::new(title.clone())
            .id_salt((id, &title))
            .default_open(default_open)
            .text_style(heading)
            .show(self.ui, |ui| {
                let mut grid = PropertyGridUi {
                    ui,
                    id,
                    label_width,
                    widest,
                };
                let inner = add_rows(&mut grid);
                widest = grid.widest;
                inner
            });
        self.ui.style_mut().spacing.indent = saved;
        self.widest = widest;
        response.body_returned
    }

    /// A row: `name` in the name column, then whatever `add_value` adds.
    /// Returns the value's result (e.g. its [`Response`]).
    pub fn row<R>(
        &mut self,
        name: impl Into<String>,
        add_value: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> R {
        let name = name.into();
        let label_width = self.label_width;
        let mut widest = self.widest;
        let inner = self
            .ui
            .with_layout(Layout::left_to_right(Align::Center), |ui| {
                let style = ui.style();
                let galley = ui.layout_text(&name, &style.body, None);
                widest = widest.max(galley.size.x.ceil());
                let height = style.spacing.interact_height;
                let rect = ui.allocate_rect(vec2(label_width, height));
                let response = ui.interact(ui.id().with(("name", &name)), rect, Sense::HOVER);
                ui.describe(&response, WidgetInfo::new(WidgetRole::Label, name.clone()));
                let pos = point(rect.min.x, rect.center().y - galley.size.y / 2.0);
                let color = style.visuals.weak_text.lerp(style.visuals.text, 0.4);
                let clip = ui.clip_rect();
                ui.set_clip_rect(rect);
                ui.painter().galley(pos, galley, color);
                ui.clip_rect_restore(clip);
                add_value(ui)
            })
            .inner;
        self.widest = widest;
        inner
    }
}

/// A field showing a reference to another object (a sketch, a face...)
/// with a × to clear it. [`Response::clicked`] when the field is clicked
/// (e.g. to start picking), [`Response::changed`] when × cleared it.
#[derive(Clone, Debug)]
pub struct ReferenceField {
    text: Option<String>,
    placeholder: String,
    width: Option<f32>,
}

impl ReferenceField {
    /// Shows `reference`, or a placeholder when it is `None`.
    pub fn new(reference: Option<&str>) -> Self {
        Self {
            text: reference.map(str::to_owned),
            placeholder: "None".to_owned(),
            width: None,
        }
    }

    /// Text shown when there is no reference (default "None").
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Width in points (default: the available width, at most 240).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for ReferenceField {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let width = self
            .width
            .unwrap_or_else(|| ui.fill_width(240.0).min(240.0));
        let size = vec2(width, style.spacing.interact_height);
        let id = ui.next_auto_id();
        let rect = ui.allocate_rect(size);
        let mut response = ui.interact(id, rect, Sense::CLICK);
        let clear_size = Vec2::splat(16.0);
        let clear_rect = Rect::from_center_size(
            point(rect.max.x - 6.0 - clear_size.x / 2.0, rect.center().y),
            clear_size,
        );
        let clear = self
            .text
            .is_some()
            .then(|| ui.interact(id.with("clear"), clear_rect, Sense::CLICK));
        let shown = self
            .text
            .clone()
            .unwrap_or_else(|| self.placeholder.clone());
        ui.describe(
            &response,
            WidgetInfo::new(WidgetRole::Button, "Reference").value(shown.clone()),
        );
        if let Some(clear) = &clear {
            ui.describe(
                clear,
                WidgetInfo::new(WidgetRole::Button, "Clear reference"),
            );
            if clear.clicked() {
                response.mark_changed();
                response.clicked = false;
            }
        }
        if response.hovered() {
            ui.ctx().set_cursor(CursorIcon::PointingHand);
        }

        let visuals = ui.widget_visuals(&response);
        let radius = style.visuals.corner_radius;
        ui.painter()
            .rect(rect, radius, style.visuals.text_field_fill, visuals.stroke);
        let color = if self.text.is_some() {
            style.visuals.text
        } else {
            style.visuals.weak_text
        };
        let galley = ui.layout_text(&shown, &style.body, None);
        let clip = ui.clip_rect();
        ui.set_clip_rect(Rect::from_min_max(
            rect.min,
            point(clear_rect.min.x - 2.0, rect.max.y),
        ));
        ui.painter().galley(
            point(rect.min.x + 8.0, rect.center().y - galley.size.y / 2.0),
            galley,
            color,
        );
        ui.clip_rect_restore(clip);
        if let Some(clear) = &clear {
            let fg = ui.widget_visuals(clear).fg;
            if clear.hovered() {
                ui.painter().rect_filled(clear_rect, 3.0, visuals.bg_fill);
            }
            let c = clear_rect.center();
            let d = 3.5;
            ui.painter()
                .line(c - vec2(d, d), c + vec2(d, d), Stroke::new(1.3, fg));
            ui.painter()
                .line(c + vec2(-d, d), c + vec2(d, -d), Stroke::new(1.3, fg));
        }
        response
    }
}
