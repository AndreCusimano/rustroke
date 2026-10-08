//! **Rustroke** is an immediate-mode GUI library for Rust, with its own GPU renderer.
//!
//! # Immediate mode in one minute
//!
//! Your app keeps its own state. Every frame, [`App::update`] describes
//! the whole UI again; widgets return a [`Response`] telling what the user
//! did with them. There are no callbacks and no widget objects to keep in
//! sync with your data.
//!
//! ```no_run
//! use rustroke::{App, Frame, Slider, WindowOptions};
//!
//! struct Counter {
//!     count: i32,
//!     step: i32,
//!     dark: bool,
//! }
//!
//! impl App for Counter {
//!     fn update(&mut self, frame: &mut Frame) {
//!         frame.ui(|ui| {
//!             ui.heading("Counter");
//!             ui.horizontal(|ui| {
//!                 if ui.button("−").clicked() {
//!                     self.count -= self.step;
//!                 }
//!                 ui.label(self.count.to_string());
//!                 if ui.button("+").clicked() {
//!                     self.count += self.step;
//!                 }
//!             });
//!             ui.add(Slider::new(&mut self.step, 1..=10).text("Step"));
//!             ui.checkbox(&mut self.dark, "Dark theme");
//!         });
//!         let style = if self.dark { rustroke::Style::dark() } else { rustroke::Style::light() };
//!         frame.ctx().set_style(style);
//!     }
//! }
//!
//! fn main() -> Result<(), rustroke::RunError> {
//!     let app = Counter { count: 0, step: 1, dark: true };
//!     rustroke::run(WindowOptions::default(), app)
//! }
//! ```
//!
//! The window only redraws when something happens (input, an animation, a
//! timer), so an idle app uses no CPU.
//!
//! Small programs don't even need a struct: any closure taking the
//! [`Frame`] is an app.
//!
//! ```no_run
//! rustroke::run(Default::default(), |frame: &mut rustroke::Frame| {
//!     frame.ui(|ui| ui.label("Hello!"));
//! })
//! .unwrap();
//! ```
//!
//! # Guide
//!
//! - **Widgets**: [`Ui::label`], [`Ui::button`], [`Ui::checkbox`],
//!   [`Ui::radio_value`], [`Slider`], [`TextEdit`], [`Image`],
//!   [`Ui::separator`]; any widget can be disabled with
//!   [`Ui::add_enabled`]. Implement [`Widget`] for your own.
//! - **Layout**: a [`Ui`] stacks widgets top to bottom. Use
//!   [`Ui::horizontal`], [`Ui::vertical_centered`], [`Ui::with_layout`]
//!   (e.g. [`Layout::right_to_left`]) and [`Grid`] for tables.
//! - **Containers**: [`Panel`] (top/bottom/left/right bars), the central
//!   area ([`Frame::ui`] or [`CentralPanel`]), floating [`Window`]s,
//!   [`ScrollArea`], popup menus ([`Ui::menu_button`]) and tooltips
//!   ([`Response::on_hover_text`]). Show panels before the central area.
//! - **Text**: all text uses the bundled Inter font, with system fonts as
//!   fallback for other scripts and emoji. Fields support selection,
//!   clipboard and input methods (IME).
//! - **Style**: [`Style::dark`] / [`Style::light`], changed at runtime with
//!   [`Context::set_style`] or locally with [`Ui::style_mut`]. Hover and
//!   press colors animate.
//! - **Images**: decode with [`load_image`] (PNG, JPEG), upload once with
//!   [`Context::load_texture`], show with [`Image`].
//! - **Custom drawing**: [`Ui::painter`] or [`Frame::shapes`] accept
//!   rectangles, circles, lines, polygons, text and images, anti-aliased.
//! - **Accessibility**: widgets are exposed to screen readers (VoiceOver,
//!   Narrator, Orca) through AccessKit. Custom widgets can describe
//!   themselves with [`Ui::describe`].
//!
//! The `examples/` directory has a runnable demo for each topic:
//! `cargo run -p rustroke --example widgets` (and `layout`, `containers`,
//! `text_input`, `themes`, `extras`, `text`, `shapes`, `hello`).
//!
//! # Crates
//!
//! This crate re-exports everything an application needs. Underneath:
//! `rustroke-core` (geometry, shapes, tessellation, input), `rustroke-text` (text
//! layout), `rustroke-render` (wgpu renderer), `rustroke-widgets` (immediate-mode
//! API) and `rustroke-winit` (window and event loop).

pub use rustroke_core::{
    Color, ColorImage, DisplayList, Galley, InputState, Key, Modifiers, Point, PointerButton, Rect,
    Shape, Stroke, TextureId, Vec2, point, vec2,
};
pub use rustroke_text::{FontFamily, Fonts, TextStyle};
pub use rustroke_widgets::Image;
pub use rustroke_widgets::{
    Align, Button, CentralPanel, Checkbox, Context, CursorIcon, Direction, Grid, Id, InnerResponse,
    Label, LayerId, Layout, Numeric, Order, Panel, PanelSide, RadioButton, Response, ScrollArea,
    Sense, Separator, Slider, Style, TextEdit, TextureHandle, Ui, UiRoot, Visuals, Widget,
    WidgetInfo, WidgetRole, Window,
};
pub use rustroke_winit::{App, Frame, RunError, WindowOptions, run};

/// Decodes a PNG or JPEG file (already read into memory) into an image
/// that can be uploaded with `Context::load_texture`.
#[cfg(feature = "image")]
pub fn load_image(bytes: &[u8]) -> Result<ColorImage, image::ImageError> {
    let rgba = image::load_from_memory(bytes)?.into_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied(
        [rgba.width(), rgba.height()],
        rgba.as_raw(),
    ))
}

#[cfg(all(test, feature = "image"))]
mod tests {
    #[test]
    fn decodes_png() {
        // 1x1 red pixel.
        let png = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D,
            0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let image = super::load_image(&png).unwrap();
        assert_eq!(image.size, [1, 1]);
        assert_eq!(image.pixels[0], [255, 0, 0, 255]);
        assert!(super::load_image(b"not an image").is_err());
    }
}

/// Compiles the README example as a doc test, so it can't go stale.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
