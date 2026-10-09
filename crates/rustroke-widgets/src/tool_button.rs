//! Toolbar buttons: a large icon, optionally with a menu of variants.

use rustroke_core::{Rect, Stroke, point, vec2};
use rustroke_text::IconId;

use crate::{Button, Response, Sense, Ui, WidgetInfo, WidgetRole};

/// Width of the ▾ part of a [`ToolButton`] with a menu.
const ARROW_WIDTH: f32 = 14.0;

/// What [`ToolButton::show_with_menu`] returns.
#[derive(Clone, Debug)]
pub struct ToolButtonResponse<R> {
    /// The icon part: clicked to use the tool.
    pub response: Response,
    /// The ▾ part, which opens the menu.
    pub arrow: Response,
    /// What the menu returned while open.
    pub inner: Option<R>,
}

/// A toolbar button: a large frameless icon, highlighted while its tool
/// is active, with a tooltip showing its name and shortcut. With
/// [`ToolButton::show_with_menu`] a ▾ next to it opens the tool's
/// variants (e.g. Extrude ▾ → extrude, revolve, sweep, loft). Separate
/// groups of tools with `ui.separator()` inside a row.
///
/// ```ignore
/// ui.horizontal(|ui| {
///     let extrude = ToolButton::new(icons.extrude, "Extrude")
///         .shortcut_text("E")
///         .selected(tool == Tool::Extrude)
///         .show_with_menu(ui, |ui| {
///             ui.selectable_value(&mut tool, Tool::Extrude, "Extrude");
///             ui.selectable_value(&mut tool, Tool::Revolve, "Revolve");
///         });
///     if extrude.response.clicked() { tool = Tool::Extrude; }
///     ui.separator();
/// });
/// ```
#[derive(Clone, Debug)]
pub struct ToolButton {
    icon: IconId,
    label: String,
    shortcut: String,
    selected: bool,
    icon_size: f32,
}

impl ToolButton {
    /// A tool named `label` (tooltip and screen readers) shown as `icon`.
    pub fn new(icon: IconId, label: impl Into<String>) -> Self {
        Self {
            icon,
            label: label.into(),
            shortcut: String::new(),
            selected: false,
            icon_size: 22.0,
        }
    }

    /// The shortcut shown in the tooltip (e.g. `KeyboardShortcut::format`).
    pub fn shortcut_text(mut self, text: impl Into<String>) -> Self {
        self.shortcut = text.into();
        self
    }

    /// Highlighted as the current tool.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Icon size in points (default 22).
    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = size;
        self
    }

    fn tooltip(&self) -> String {
        if self.shortcut.is_empty() {
            self.label.clone()
        } else {
            format!("{}  ({})", self.label, self.shortcut)
        }
    }

    fn button(&self) -> Button {
        Button::icon_only(self.icon)
            .icon_size(self.icon_size)
            .frame(false)
            .selected(self.selected)
            .accessible_label(self.label.clone())
    }

    /// Shows the button alone.
    pub fn show(self, ui: &mut Ui<'_>) -> Response {
        let tooltip = self.tooltip();
        ui.add(self.button()).on_hover_text(ui, tooltip)
    }

    /// Shows the button with a ▾ that opens a menu filled by `add_menu`.
    pub fn show_with_menu<R>(
        self,
        ui: &mut Ui<'_>,
        add_menu: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> ToolButtonResponse<R> {
        let tooltip = self.tooltip();
        let saved = ui.style().spacing.item_spacing.x;
        ui.style_mut().spacing.item_spacing.x = 0.0;
        let response = ui.add(self.button()).on_hover_text(ui, tooltip);
        ui.style_mut().spacing.item_spacing.x = saved;

        let main = response.rect;
        let arrow_rect = ui.allocate_rect(vec2(ARROW_WIDTH, main.height()));
        let arrow_rect = Rect::from_min_size(
            point(arrow_rect.min.x, main.min.y),
            vec2(ARROW_WIDTH, main.height()),
        );
        let id = response.id.with("variants");
        let arrow = ui.interact(id, arrow_rect, Sense::CLICK);
        let open = ui.ctx().is_popup_open(id);
        ui.describe(
            &arrow,
            WidgetInfo::new(WidgetRole::Button, format!("{} variants", self.label)).expanded(open),
        );
        if arrow.clicked() {
            ui.ctx().toggle_popup(id);
        }
        let style = ui.style();
        let visuals = ui.widget_visuals(&arrow);
        if arrow.hovered() || arrow.is_pressed() {
            ui.painter()
                .rect_filled(arrow_rect, style.visuals.corner_radius, visuals.bg_fill);
        }
        let c = arrow_rect.center();
        ui.painter().polyline(
            vec![
                c + vec2(-3.5, -1.5),
                c + vec2(0.0, 2.0),
                c + vec2(3.5, -1.5),
            ],
            Stroke::new(1.3, visuals.fg),
        );
        let anchor = main.union(arrow_rect);
        let inner = ui.popup_below(id, anchor, add_menu);
        ToolButtonResponse {
            response,
            arrow,
            inner,
        }
    }
}
