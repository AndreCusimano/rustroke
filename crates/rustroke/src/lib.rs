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
//!   [`Ui::radio_value`], [`Ui::selectable_value`], [`Slider`],
//!   [`DragValue`] (drag or type a number), [`ComboBox`] (drop-down list),
//!   [`TextEdit`] (also for passwords), [`SearchField`], [`Hyperlink`],
//!   [`Image`], [`Icon`], [`IconToggle`] (two-state icon, e.g. an eye),
//!   [`ProgressBar`], [`Spinner`], [`List`] (rows to select and reorder
//!   by dragging), [`Table`] (sortable, resizable columns; millions of
//!   rows), [`Tree`] (expandable nodes), [`ToolButton`] (toolbar icon with a menu of variants),
//!   [`PropertyGrid`] (name / value rows in collapsible sections) and
//!   [`ReferenceField`], [`Ui::separator`]; any widget can be disabled with
//!   [`Ui::add_enabled`]. Buttons, fields and combo boxes take their own
//!   colors and sizes with `.fill(..)`, `.stroke(..)`,
//!   `.corner_radius(..)` and `.min_size(..)`. Implement [`Widget`] for
//!   your own.
//! - **Layout**: a [`Ui`] stacks widgets top to bottom. Use
//!   [`Ui::horizontal`], [`Ui::vertical_centered`], [`Ui::with_layout`]
//!   (e.g. [`Layout::right_to_left`]) and [`Grid`] for tables.
//! - **Containers**: [`Panel`] (top/bottom/left/right bars), the central
//!   area ([`Frame::ui`] or [`CentralPanel`]), floating [`Window`]s,
//!   modal dialogs ([`Modal`]), [`ScrollArea`] (vertical, horizontal or
//!   both; [`ScrollArea::show_rows`] lays out only the visible rows,
//!   [`Ui::scroll_to_rect`] brings something into view), popup menus with submenus ([`Ui::menu_button`]; also below any
//!   rectangle: [`Ui::popup_below`]), context menus
//!   ([`Response::context_menu`]), collapsible sections and trees
//!   ([`CollapsingHeader`]), tab bars ([`TabBar`]), dockable panels
//!   ([`DockArea`]: tab groups in resizable splits, rearranged by dragging
//!   tabs) and tooltips ([`Response::on_hover_text`]).
//!   Show panels before the central area.
//! - **Mouse and keyboard**: [`Response`] reports clicks and drags per
//!   button ([`Response::secondary_clicked`], [`Response::dragged_by`],
//!   [`Response::double_clicked`]);
//!   shortcuts are [`KeyboardShortcut`]s checked with
//!   [`InputState::consume_shortcut`] and shown in menus with
//!   [`Button::shortcut_text`]. Skip them while
//!   [`Context::wants_keyboard_input`] (the user is typing) or
//!   [`Context::is_modal_open`].
//! - **Text**: all text uses the bundled Inter font (every weight, e.g.
//!   `TextStyle::proportional(14.0).weight(600)`) or the platform's UI font
//!   ([`TextStyle::system`]), with system fonts as fallback for other
//!   scripts and emoji. Fields support selection,
//!   clipboard, undo/redo and input methods (IME); Escape cancels an edit and
//!   [`Response::lost_focus_reason`] tells why editing ended.
//! - **Style**: [`Style::dark`] / [`Style::light`], changed at runtime with
//!   [`Context::set_style`] or locally with [`Ui::style_mut`]. Hover and
//!   press colors animate.
//! - **Images**: decode with [`load_image`] (PNG, JPEG), upload once with
//!   [`Context::load_texture`], show with [`Image`].
//! - **Icons**: load SVGs once with [`Fonts::add_svg_icon`] (through
//!   [`Frame::fonts`]); they are rasterized at the size and screen density
//!   they are drawn at. Black parts take the text color and
//!   [`ICON_ACCENT_SOURCE_COLOR`] parts the theme's accent, so one file
//!   works in light and dark themes. Show them with [`Ui::icon`],
//!   [`Button::icon`], [`Button::icon_only`] or [`CollapsingHeader::icon`].
//! - **Custom drawing**: [`Ui::painter`] or [`Frame::shapes`] accept
//!   rectangles, circles, lines, polygons, text and images, anti-aliased.
//! - **Accessibility**: widgets are exposed to screen readers (VoiceOver,
//!   Narrator, Orca) through AccessKit. Custom widgets can describe
//!   themselves with [`Ui::describe`].
//! - **Your own GPU rendering** (e.g. a 3D viewport): get rustroke's device
//!   and queue with [`Frame::wgpu`], render into your texture and show it
//!   with [`Frame::register_native_texture`] — no copies. Smaller custom
//!   drawings can run inside the UI's render pass with a [`CallbackFn`]
//!   (`ui.painter().add(callback.into_shape(rect))`).
//! - **Your own event loop**: an app that already owns its winit window
//!   and wgpu device embeds the UI with [`Integration`] (events in, UI
//!   drawn over your content), instead of calling [`run`].
//! - **Background work**: [`Frame::repaint_handle`] gives a
//!   [`RepaintHandle`] that wakes the UI up from any thread.
//! - **Window**: [`Frame::set_title`]; [`App::on_close_requested`] can keep
//!   the window open (e.g. to ask about unsaved changes), and
//!   [`Frame::close`] closes it once answered. On macOS,
//!   [`WindowOptions::unified_titlebar`] lets the app draw its own top bar
//!   next to the window buttons.
//! - **More windows**: keep extra native windows open with
//!   [`Frame::show_window`] (e.g. a view on a second monitor) and draw
//!   them in [`App::update_window`].
//! - **Testing**: [`testing::Harness`] runs your app without a window;
//!   click widgets by label, type text, press keys, inspect
//!   [`Context::widgets`] and render the frame to an image.
//!
//! The `examples/` directory has a runnable demo for each topic:
//! `cargo run -p rustroke --example widgets` (and `properties`, `lists`,
//! `table`,
//! `docking`, `windows`, `files`, `custom_wgpu`, `integration`, `layout`,
//! `containers`, `text_input`, `themes`, `extras`, `text`, `shapes`,
//! `hello`).
//!
//! # Crates
//!
//! This crate re-exports everything an application needs. Underneath:
//! `rustroke-core` (geometry, shapes, tessellation, input), `rustroke-text` (text
//! layout), `rustroke-render` (wgpu renderer), `rustroke-widgets` (immediate-mode
//! API) and `rustroke-winit` (window and event loop).

pub use rustroke_core::{
    Color, ColorImage, DisplayList, Event, Galley, ImeEvent, InputState, Key, KeyboardShortcut,
    Modifiers, POINTS_PER_SCROLL_LINE, PaintCallback, PhysicalSize, Point, PointerButton, RawInput,
    Rect, Shape, Stroke, TextureId, TexturesDelta, Vec2, point, vec2,
};
pub use rustroke_text::{
    FontFamily, Fonts, ICON_ACCENT_SOURCE_COLOR, IconError, IconId, IconLayer, RasterizedIcon,
    TextStyle,
};
pub use rustroke_widgets::Image;
pub use rustroke_widgets::{
    Align, Button, CentralPanel, Checkbox, CollapsingHeader, CollapsingResponse, Column, ComboBox,
    Context, CursorIcon, Direction, DockArea, DockNode, DockState, DockViewer, DragValue,
    FocusLost, FrameOutput, Grid, Hyperlink, Icon, IconToggle, Id, InnerResponse, Label, LayerId,
    Layout, List, ListResponse, Modal, ModalResponse, Numeric, Order, Panel, PanelSide,
    ProgressBar, PropertyGrid, PropertyGridUi, RadioButton, ReferenceField, RepaintHandle,
    Response, ScrollArea, SearchField, SelectableLabel, Sense, Separator, Slider, SortOrder,
    Spinner, SplitAxis, Style, TabBar, TabBarResponse, TabLabel, Table, TableResponse, TextEdit,
    TextureHandle, ToolButton, ToolButtonResponse, Tree, TreeResponse, Ui, UiRoot, Visuals, Widget,
    WidgetDescription, WidgetInfo, WidgetRole, Window,
};
pub use rustroke_winit::{
    App, CallbackFn, CallbackInfo, EventResponse, Frame, Integration, RunError, RunOutput,
    WindowOptions, run, testing, wgpu, winit,
};

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
