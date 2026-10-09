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
//!   [`ColorPicker`], [`DatePicker`], [`TimePicker`], toasts
//!   ([`Context::toast`]),
//!   [`PropertyGrid`] (name / value rows in collapsible sections) and
//!   [`ReferenceField`], [`Ui::separator`]; any widget can be disabled with
//!   [`Ui::add_enabled`]. Buttons, fields and combo boxes take their own
//!   colors and sizes with `.fill(..)`, `.stroke(..)`,
//!   `.corner_radius(..)` and `.min_size(..)`. Implement [`Widget`] for
//!   your own.
//! - **Layout**: a [`Ui`] stacks widgets top to bottom. Use
//!   [`Ui::horizontal`], [`Ui::vertical_centered`], [`Ui::with_layout`]
//!   (e.g. [`Layout::right_to_left`]) and [`Grid`] for tables; [`Flex`]
//!   (flexbox: wrapping, growing, justified items) and [`FlexGrid`]
//!   (CSS-style grid with fractional columns and spans).
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
//! - **Rich text**: [`Label::rich`] shows a [`LayoutJob`] whose sections
//!   have their own [`TextFormat`] (bold, italic, colors, highlights,
//!   underline, links). With the `markdown` feature, `ui.markdown(text)`
//!   renders Markdown documents.
//! - **Text**: all text uses the bundled Inter font (every weight, e.g.
//!   `TextStyle::proportional(14.0).weight(600)`) or the platform's UI font
//!   ([`TextStyle::system`]), with system fonts as fallback for other
//!   scripts and emoji; right-to-left scripts are aligned and selected
//!   correctly. Fields support selection,
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
//!   rectangles, circles, lines, polygons, text and images, anti-aliased,
//!   plus [`Gradient`] fills, soft [`Shadow`]s, dashed and dotted lines,
//!   Bézier curves, app-built [`Mesh`]es and [`Transform`]s
//!   (`DisplayList::with_transform`, `DisplayList::galley_transformed`).
//! - **Animated images**: [`load_animated_image`] (GIF, APNG, WebP) →
//!   [`AnimatedTexture`] → [`AnimatedImage`].
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
//! - **Drag and drop**: [`Ui::dnd_drag_source`] and [`Ui::dnd_drop_zone`]
//!   move payloads of any type between widgets; files dropped from the
//!   system arrive in [`InputState::dropped_files`]; pinch and rotation
//!   gestures in [`InputState::zoom_delta`] and `rotation_delta`.
//! - **Platform**: a macOS menu bar ([`Frame::set_native_menu`], feature
//!   `native-menu`), a tray icon ([`Frame::set_tray`], feature `tray`) and
//!   system notifications ([`notify`]).
//! - **More windows**: keep extra native windows open with
//!   [`Frame::show_window`] (e.g. a view on a second monitor) and draw
//!   them in [`App::update_window`].
//! - **Testing**: [`testing::Harness`] runs your app without a window;
//!   click widgets by label, type text, press keys, inspect
//!   [`Context::widgets`] and the accessibility tree, and render the
//!   frame to an image. [`Context::automation`] drives a running app from
//!   another thread the same way.
//! - **Saving state** (feature `persistence`): with
//!   [`WindowOptions::persistence_id`] window positions, panel sizes,
//!   open sections, table columns and your own values
//!   ([`Frame::set_value`]) are restored at the next start.
//! - **Debugging**: Cmd/Ctrl+Alt+I opens the inspector (what is under the
//!   pointer, focus, frame time, live style editing); Cmd/Ctrl + = / - / 0
//!   zoom the UI.
//!
//! The `examples/` directory has a runnable demo for each topic:
//! `cargo run -p rustroke --example widgets` (and `properties`, `lists`,
//! `table`, `rich_text` with `--features markdown`, `graphics`,
//! `platform` with `--features native-menu,tray`, `pickers`, `flex`,
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
    Color, ColorImage, DisplayList, Event, Galley, Gradient, GradientKind, ImeEvent, InputState,
    Key, KeyboardShortcut, Mesh, Modifiers, POINTS_PER_SCROLL_LINE, PaintCallback, PhysicalSize,
    Point, PointerButton, RawInput, Rect, Shadow, Shape, Stroke, TextureId, TexturesDelta,
    Transform, Vec2, Vertex, point, vec2,
};
pub use rustroke_text::{
    FontFamily, Fonts, ICON_ACCENT_SOURCE_COLOR, IconError, IconId, IconLayer, LayoutJob,
    RasterizedIcon, TextFormat, TextStyle,
};
pub use rustroke_widgets::Image;
pub use rustroke_widgets::{
    Align, AnimatedImage, AnimatedTexture, Automation, Button, CentralPanel, Checkbox,
    CollapsingHeader, CollapsingResponse, ColorPicker, Column, ComboBox, Context, CursorIcon, Date,
    DatePicker, Direction, DockArea, DockNode, DockState, DockViewer, DragValue, FocusLost,
    FrameOutput, Grid, Hyperlink, Icon, IconToggle, Id, InnerResponse, Label, LayerId, Layout,
    List, ListResponse, Modal, ModalResponse, Numeric, Order, Panel, PanelSide, ProgressBar,
    PropertyGrid, PropertyGridUi, RadioButton, ReferenceField, RepaintHandle, Response, ScrollArea,
    SearchField, SelectableLabel, Sense, Separator, Slider, SortOrder, Spinner, SplitAxis, Style,
    TabBar, TabBarResponse, TabLabel, Table, TableResponse, TextEdit, TextureHandle, TimePicker,
    Toast, ToastLevel, ToolButton, ToolButtonResponse, Tree, TreeResponse, Ui, UiRoot, Visuals,
    Widget, WidgetDescription, WidgetInfo, WidgetRole, Window, show_inspector, show_toasts,
};
pub use rustroke_widgets::{
    Flex, FlexAlign, FlexDirection, FlexGrid, FlexItem, FlexJustify, FlexUi, GridCell, GridUi,
    Track,
};
#[cfg(feature = "markdown")]
pub use rustroke_widgets::{ImageLoader, Markdown};
pub use rustroke_winit::{
    App, CallbackFn, CallbackInfo, EventResponse, Frame, Integration, NativeMenu, NativeMenuItem,
    RunError, RunOutput, TrayOptions, WindowOptions, accesskit, notify, run, testing, wgpu, winit,
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

/// Decodes an animated GIF, PNG (APNG) or WebP file into its frames and
/// how long each is shown, for [`AnimatedTexture::new`]. Still images
/// (also JPEG) give one frame.
#[cfg(feature = "image")]
pub fn load_animated_image(
    bytes: &[u8],
) -> Result<Vec<(ColorImage, std::time::Duration)>, image::ImageError> {
    use image::AnimationDecoder;
    use std::io::Cursor;
    let frames = match image::guess_format(bytes)? {
        image::ImageFormat::Gif => {
            image::codecs::gif::GifDecoder::new(Cursor::new(bytes))?.into_frames()
        }
        image::ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))?;
            if !decoder.is_apng()? {
                return Ok(vec![(load_image(bytes)?, std::time::Duration::ZERO)]);
            }
            decoder.apng()?.into_frames()
        }
        image::ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))?;
            if !decoder.has_animation() {
                return Ok(vec![(load_image(bytes)?, std::time::Duration::ZERO)]);
            }
            decoder.into_frames()
        }
        _ => return Ok(vec![(load_image(bytes)?, std::time::Duration::ZERO)]),
    };
    frames
        .map(|frame| {
            let frame = frame?;
            let delay = std::time::Duration::from(frame.delay());
            let rgba = frame.into_buffer();
            Ok((
                ColorImage::from_rgba_unmultiplied([rgba.width(), rgba.height()], rgba.as_raw()),
                delay,
            ))
        })
        .collect()
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

    #[test]
    fn decodes_animated_gifs_and_plays_them() {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame, Rgba, RgbaImage};
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            for (color, ms) in [([255, 0, 0, 255], 100), ([0, 0, 255, 255], 300)] {
                let image = RgbaImage::from_pixel(2, 2, Rgba(color));
                let delay = Delay::from_numer_denom_ms(ms, 1);
                encoder
                    .encode_frame(Frame::from_parts(image, 0, 0, delay))
                    .unwrap();
            }
        }
        let frames = super::load_animated_image(&bytes).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].0.pixels[0], [255, 0, 0, 255]);
        assert_eq!(frames[1].1, std::time::Duration::from_millis(300));

        let mut ctx = super::Context::new();
        let anim = super::AnimatedTexture::new(&mut ctx, frames);
        let (first, _) = anim.frame_at(0.05).unwrap();
        let (second, left) = anim.frame_at(0.2).unwrap();
        assert_ne!(first.id(), second.id());
        assert!((left - 0.2).abs() < 1e-9, "{left}");
        assert_eq!(anim.frame_at(0.45).unwrap().0.id(), first.id(), "it loops");
    }
}

/// Compiles the README example as a doc test, so it can't go stale.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
