use rustroke_core::{Color, Stroke, Vec2};
use rustroke_text::TextStyle;

use crate::Response;

/// Sizes, colors, fonts and timing used by the built-in widgets.
///
/// Start from [`Style::dark`] or [`Style::light`] and change what you
/// need, then apply it with `Context::set_style` (whole app, takes effect
/// immediately) or `Ui::style_mut` (one part of the UI).
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// Distances and sizes.
    pub spacing: Spacing,
    /// Colors and shapes.
    pub visuals: Visuals,
    /// Text of labels and widgets.
    pub body: TextStyle,
    /// Text of headings.
    pub heading: TextStyle,
    /// Seconds for hover/press color transitions. 0 disables animations.
    pub animation_time: f32,
}

/// Distances, in logical points.
#[derive(Clone, Debug, PartialEq)]
pub struct Spacing {
    /// Gap between consecutive widgets: `x` in rows, `y` in columns.
    pub item_spacing: Vec2,
    /// Space between a button's border and its text.
    pub button_padding: Vec2,
    /// Minimum height of interactive widgets (buttons, sliders, ...).
    pub interact_height: f32,
    /// Size of checkbox and radio button indicators.
    pub icon_size: f32,
    /// Gap between an indicator (or slider) and its text.
    pub icon_spacing: f32,
    /// Length of slider tracks.
    pub slider_width: f32,
    /// Space between the window edge and the content of the central panel.
    pub window_margin: f32,
    /// Space between the border of panels, windows and popups and their content.
    pub window_padding: f32,
    /// Width of scroll bars.
    pub scrollbar_width: f32,
    /// Horizontal shift of nested content (collapsible sections, trees).
    pub indent: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            item_spacing: Vec2::new(8.0, 8.0),
            button_padding: Vec2::new(12.0, 4.0),
            interact_height: 28.0,
            icon_size: 18.0,
            icon_spacing: 8.0,
            slider_width: 200.0,
            window_margin: 16.0,
            window_padding: 10.0,
            scrollbar_width: 8.0,
            indent: 18.0,
        }
    }
}

/// Colors for one interaction state of a widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetVisuals {
    /// Background.
    pub bg_fill: Color,
    /// Border.
    pub stroke: Stroke,
    /// Text and icon color.
    pub fg: Color,
}

impl WidgetVisuals {
    /// Blends towards `other` (`t` from 0 to 1), for animated transitions.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            bg_fill: self.bg_fill.lerp(other.bg_fill, t),
            stroke: Stroke::new(
                self.stroke.width + (other.stroke.width - self.stroke.width) * t,
                self.stroke.color.lerp(other.stroke.color, t),
            ),
            fg: self.fg.lerp(other.fg, t),
        }
    }
}

/// The color scheme.
#[derive(Clone, Debug, PartialEq)]
pub struct Visuals {
    /// Used to pick contrasting details (e.g. shadows); not a theme switch.
    pub dark_mode: bool,
    /// Behind the central panel.
    pub background: Color,
    /// Side/top/bottom panels.
    pub panel_fill: Color,
    /// Floating windows, popups, menus and tooltips.
    pub window_fill: Color,
    /// Border of floating content.
    pub window_stroke: Stroke,
    /// Corner radius of floating content.
    pub window_corner_radius: f32,
    /// Shadow under floating content.
    pub shadow: Color,
    /// Inside text fields.
    pub text_field_fill: Color,
    /// Normal text.
    pub text: Color,
    /// Secondary text (hints, captions).
    pub weak_text: Color,
    /// Selected/checked state and filled slider track.
    pub accent: Color,
    /// Content drawn on top of `accent` (check marks, radio dots).
    pub on_accent: Color,
    /// Background of selected text.
    pub selection: Color,
    /// Keyboard focus indicator.
    pub focus: Color,
    /// Something went well (done, valid, computed).
    pub success: Color,
    /// Needs attention (outdated, unsaved, suspicious).
    pub warning: Color,
    /// Something failed.
    pub error: Color,
    /// Neutral information (hints, in-progress states).
    pub info: Color,
    /// Background of every other row in striped grids.
    pub stripe: Color,
    /// Opacity of disabled widgets (0 = invisible, 1 = like enabled ones).
    pub disabled_alpha: f32,
    /// Slider handles.
    pub handle_fill: Color,
    /// Border of slider handles.
    pub handle_stroke: Stroke,
    /// Buttons, text fields, windows' inner elements.
    pub corner_radius: f32,
    /// Small elements: checkboxes, close buttons, grid stripes.
    pub small_corner_radius: f32,
    /// Widgets at rest.
    pub inactive: WidgetVisuals,
    /// Widgets under the pointer.
    pub hovered: WidgetVisuals,
    /// While pressed or dragged.
    pub active: WidgetVisuals,
}

impl Visuals {
    /// The colors for a widget in the state described by `response`,
    /// without animation (see `Ui::widget_visuals` for the animated one).
    pub fn widget(&self, response: &Response) -> &WidgetVisuals {
        if response.is_pressed() {
            &self.active
        } else if response.hovered() {
            &self.hovered
        } else {
            &self.inactive
        }
    }

    /// Changes the accent color (and the selection that derives from it).
    pub fn with_accent(mut self, accent: Color) -> Self {
        self.accent = accent;
        self.selection = accent.with_alpha(self.selection.a);
        self
    }

    /// Catppuccin Mocha-inspired dark colors.
    pub fn dark() -> Self {
        let rgb = Color::from_srgb8;
        let text = rgb(205, 214, 244);
        let accent = rgb(137, 180, 250);
        Self {
            dark_mode: true,
            background: rgb(30, 30, 46),
            panel_fill: rgb(24, 24, 37),
            window_fill: rgb(36, 36, 54),
            window_stroke: Stroke::new(1.0, rgb(69, 71, 90)),
            window_corner_radius: 10.0,
            shadow: Color::new(0.0, 0.0, 0.0, 0.35),
            text_field_fill: rgb(24, 24, 37),
            text,
            weak_text: rgb(166, 173, 200),
            accent,
            on_accent: rgb(30, 30, 46),
            selection: accent.with_alpha(0.35),
            focus: rgb(249, 226, 175),
            success: rgb(166, 227, 161),
            warning: rgb(249, 226, 175),
            error: rgb(243, 139, 168),
            info: rgb(137, 220, 235),
            stripe: rgb(49, 50, 68).with_alpha(0.5),
            disabled_alpha: 0.4,
            handle_fill: text,
            handle_stroke: Stroke::new(1.0, rgb(30, 30, 46)),
            corner_radius: 6.0,
            small_corner_radius: 4.0,
            inactive: WidgetVisuals {
                bg_fill: rgb(49, 50, 68),
                stroke: Stroke::new(1.0, rgb(69, 71, 90)),
                fg: text,
            },
            hovered: WidgetVisuals {
                bg_fill: rgb(69, 71, 90),
                stroke: Stroke::new(1.0, rgb(108, 112, 134)),
                fg: text,
            },
            active: WidgetVisuals {
                bg_fill: rgb(88, 91, 112),
                stroke: Stroke::new(1.0, rgb(127, 132, 156)),
                fg: text,
            },
        }
    }

    /// Catppuccin Latte-inspired light colors.
    pub fn light() -> Self {
        let rgb = Color::from_srgb8;
        let text = rgb(76, 79, 105);
        let accent = rgb(30, 102, 245);
        Self {
            dark_mode: false,
            background: rgb(239, 241, 245),
            panel_fill: rgb(230, 233, 239),
            window_fill: rgb(250, 251, 252),
            window_stroke: Stroke::new(1.0, rgb(204, 208, 218)),
            window_corner_radius: 10.0,
            shadow: Color::new(0.0, 0.0, 0.0, 0.18),
            text_field_fill: rgb(255, 255, 255),
            text,
            weak_text: rgb(108, 111, 133),
            accent,
            on_accent: rgb(255, 255, 255),
            selection: accent.with_alpha(0.25),
            focus: rgb(223, 142, 29),
            success: rgb(64, 160, 43),
            warning: rgb(223, 142, 29),
            error: rgb(210, 15, 57),
            info: rgb(4, 165, 229),
            stripe: rgb(220, 224, 232).with_alpha(0.6),
            disabled_alpha: 0.45,
            handle_fill: rgb(255, 255, 255),
            handle_stroke: Stroke::new(1.0, rgb(156, 160, 176)),
            corner_radius: 6.0,
            small_corner_radius: 4.0,
            inactive: WidgetVisuals {
                bg_fill: rgb(220, 224, 232),
                stroke: Stroke::new(1.0, rgb(188, 192, 204)),
                fg: text,
            },
            hovered: WidgetVisuals {
                bg_fill: rgb(204, 208, 218),
                stroke: Stroke::new(1.0, rgb(156, 160, 176)),
                fg: text,
            },
            active: WidgetVisuals {
                bg_fill: rgb(188, 192, 204),
                stroke: Stroke::new(1.0, rgb(140, 143, 161)),
                fg: text,
            },
        }
    }
}

impl Default for Style {
    fn default() -> Self {
        Self::dark()
    }
}

impl Style {
    /// The dark theme.
    pub fn dark() -> Self {
        Self {
            spacing: Spacing::default(),
            visuals: Visuals::dark(),
            body: TextStyle::proportional(14.0),
            heading: TextStyle::proportional(20.0).bold(),
            animation_time: 0.12,
        }
    }

    /// The light theme (same sizes as the dark one).
    pub fn light() -> Self {
        Self {
            visuals: Visuals::light(),
            ..Self::dark()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_visuals_lerp_endpoints() {
        let v = Visuals::dark();
        assert_eq!(v.inactive.lerp(&v.hovered, 0.0), v.inactive);
        assert_eq!(v.inactive.lerp(&v.hovered, 1.0), v.hovered);
    }

    #[test]
    fn with_accent_updates_selection() {
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        let v = Visuals::light().with_accent(red);
        assert_eq!(v.accent, red);
        assert_eq!(v.selection, red.with_alpha(0.25));
    }
}
