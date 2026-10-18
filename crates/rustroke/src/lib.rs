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
//!   [`TextEdit`] (also for passwords, and code: [`TextEdit::font`],
//!   [`TextEdit::code_editor`], [`TextEdit::layouter`] for syntax colors,
//!   [`TextEdit::line_numbers`], [`TextEdit::highlight_line`],
//!   [`TextEdit::set_selection`]), [`SearchField`], [`Hyperlink`],
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
//! - **Layout**: a [`Ui`] stacks widgets top to bottom. Two containers
//!   arrange widgets, both in a box that can have a background (color,
//!   gradient or image, see [`Background`]), border, corners, shadow,
//!   padding, margin and a size (content, fill, fixed, with limits; see
//!   [`BoxStyle`]): [`Stack`] puts them in a row, a column or one over the
//!   other ([`Stack::overlay`]), aligned and spread like flexbox, with
//!   per-item options through [`Ui::stack_item`] (grow, limits, alignment);
//!   [`Grid`] puts them in rows and columns (content-sized, fixed or
//!   fractional [`Track`]s, cells spanning columns and rows with
//!   [`Ui::grid_span`] / [`Ui::grid_cell`]). [`Ui::horizontal`] and
//!   [`Ui::vertical`] are the short forms for a plain row or column.
//! - **Containers**: [`Panel`] (top/bottom/left/right bars, also inside a
//!   Ui with [`Panel::show_inside`]), the central
//!   area ([`Frame::ui`] or [`CentralPanel`]), floating [`Window`]s,
//!   modal dialogs ([`Modal`]), [`ScrollArea`] (vertical, horizontal or
//!   both; [`ScrollArea::show_rows`] lays out only the visible rows,
//!   [`Ui::scroll_to_rect`] brings something into view), popup menus with submenus ([`Ui::menu_button`], with an icon
//!   [`Ui::menu_button_with_icon`]; also below any
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
//!   [`DisplayList::path_filled`] fills concave outlines, holes and
//!   self-intersections with [`FillRule::EvenOdd`] or
//!   [`FillRule::NonZero`].
//! - **Animated images**: [`load_animated_image`] (GIF, APNG, WebP) →
//!   [`AnimatedTexture`] → [`AnimatedImage`].
//! - **Accessibility**: widgets are exposed to screen readers (VoiceOver,
//!   Narrator, Orca) through AccessKit. Custom widgets can describe
//!   themselves with [`Ui::describe`].
//! - **Custom widgets**: draw an area with `ui.interact` and
//!   [`Sense::FOCUSABLE`], then [`Response::request_focus`],
//!   [`Ui::take_tab`] and [`Ui::take_keyboard`] give it the keyboard
//!   while it has focus (Tab between fields, Enter to confirm).
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
//!   next to the window buttons. [`WindowOptions::resizable`] and
//!   [`WindowOptions::min_inner_size`] limit resizing.
//!   [`Frame::focus_window`] brings a window to the front;
//!   [`Modal::movable`] and [`Modal::position`] place and move a dialog.
//! - **Drag and drop**: [`Ui::dnd_drag_source`] (or
//!   [`Ui::dnd_set_payload`] on any widget) and [`Ui::dnd_drop_zone`]
//!   move payloads of any type between widgets; files dropped from the
//!   system arrive in [`InputState::dropped_files`]; pinch and rotation
//!   gestures in [`InputState::zoom_delta`] and `rotation_delta`.
//! - **Platform**: a macOS menu bar ([`Frame::set_native_menu`], feature
//!   `native-menu`), a tray icon ([`Frame::set_tray`], feature `tray`) and
//!   system notifications ([`notify`]).
//! - **More windows**: keep extra native windows open with
//!   [`Frame::show_window`] (e.g. a view on a second monitor), draw them
//!   in [`App::update_window`] and bring one to the front with
//!   [`Frame::focus_window`].
//! - **In the browser**: the same app compiles to `wasm32-unknown-unknown`
//!   and runs in a canvas with WebGPU or WebGL 2 (build an example with
//!   `tools/web.sh <example>`).
//! - **Testing**: [`testing::Harness`] runs your app without a window;
//!   click widgets by label, type text, press keys, inspect
//!   [`Context::widgets`] and the accessibility tree, and render the
//!   frame to an image on the GPU or, without one, with the
//!   [`SoftwareRenderer`]. Extra windows get input with
//!   [`testing::Harness::run_window_with_events`] and
//!   [`testing::Harness::click_in_window`].
//!   [`Context::automation`] drives a running app from another thread the
//!   same way.
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
//! `platform` with `--features native-menu,tray`, `pickers`, `stack`, `charts`,
//! `docking`, `windows`, `files`, `custom_wgpu`, `integration`, `layout`,
//! `containers`, `text_input`, `themes`, `extras`, `text`, `shapes`,
//! `hello`).
//!
//! # Components
//!
//! Every component in one line, by category.
//!
//! **Text**
//!
//! | Component | What it is |
//! |---|---|
//! | [`Label`] | Text, wrapping to the width; rich text with several styles and links (`Label::rich`). Shorthands: `ui.label`, `ui.heading`. |
//! | [`Hyperlink`] | A link that opens a URL, or link-styled text that reacts to clicks. |
//! | [`Markdown`] | A Markdown document: headings, lists, tables, code, images (feature `markdown`). |
//!
//! **Buttons and choices**
//!
//! | Component | What it is |
//! |---|---|
//! | [`Button`] | A clickable button with text, an icon or both. |
//! | [`ToolButton`] | A large frameless toolbar icon, highlighted while its tool is active, optionally with a menu of variants. |
//! | [`IconToggle`] | An icon that switches a `bool` (eye open/closed, lock). |
//! | [`Checkbox`] | A box that toggles a `bool`, with a label. |
//! | [`RadioButton`] | One choice of several; `ui.radio_value` sets a value when chosen. |
//! | [`SelectableLabel`] | Text highlighted when selected, for choices in lists and menus; `ui.selectable_value`. |
//! | [`ComboBox`] | A drop-down list showing the current choice. |
//!
//! **Values and input**
//!
//! | Component | What it is |
//! |---|---|
//! | [`TextEdit`] | A field to edit text: one line, several lines, password, or code (monospace, Tab, line numbers, syntax colors). |
//! | [`SearchField`] | A one-line field for filtering, with a magnifying glass and a clear button. |
//! | [`Slider`] | A handle dragged along a track to pick a number in a range. |
//! | [`DragValue`] | A number changed by dragging sideways or typed, with units and limits. |
//! | [`ColorPicker`] | A color swatch that opens a picker (hue, saturation/value, opacity, hex). |
//! | [`DatePicker`] | A date box that opens a calendar. |
//! | [`TimePicker`] | Hours and minutes, dragged or typed. |
//! | [`ReferenceField`] | A reference to another object (a sketch, a face) with a button to clear it. |
//!
//! **Feedback**
//!
//! | Component | What it is |
//! |---|---|
//! | [`ProgressBar`] | A bar showing how much of a task is done, or that it is running. |
//! | [`Spinner`] | A rotating arc: something is in progress. |
//! | [`Toast`] | A short message in a corner for a few seconds (`Context::toast`). |
//! | Tooltip ([`Response::on_hover_text`]) | Text shown while the pointer rests on a widget. |
//!
//! **Images and icons**
//!
//! | Component | What it is |
//! |---|---|
//! | [`Image`] | An image loaded with `Context::load_texture`, scaled and with rounded corners. |
//! | [`AnimatedImage`] | A GIF, APNG or animated WebP, played with the app's clock. |
//! | [`Icon`] | An SVG icon drawn in the theme's colors. |
//!
//! **Layout**
//!
//! | Component | What it is |
//! |---|---|
//! | [`Stack`] | Widgets in a row, a column or one over the other, aligned, spread, wrapped and grown like flexbox, in a box with background, border, padding and size. `ui.horizontal` / `ui.vertical` are the short forms. |
//! | [`Grid`] | Widgets in rows and columns (content-sized, fixed or fractional), cells spanning several, in the same kind of box. |
//! | [`Separator`] | A thin line between groups; `ui.separator`. |
//!
//! **Areas and windows**
//!
//! | Component | What it is |
//! |---|---|
//! | [`Panel`] | A bar along an edge of the window or of an area: toolbar, sidebar, status bar; resizable, with limits. |
//! | [`CentralPanel`] | Everything left after the panels. |
//! | [`ScrollArea`] | Content larger than its area, scrolled vertically, horizontally or both; only the visible rows of long lists. |
//! | [`Window`] | A floating window inside the app: moved, resized, closed, brought to the front. |
//! | [`Modal`] | A dialog that blocks the rest of the window until it is answered. |
//! | [`CollapsingHeader`] | A section that opens and closes; nested, a tree. |
//! | [`TabBar`] | A row of tabs that can be closed, added and reordered. |
//! | [`DockArea`] | Panels as tab groups in resizable splits; tabs are dragged between groups or to a side. |
//!
//! **Lists and data**
//!
//! | Component | What it is |
//! |---|---|
//! | [`List`] | Rows of your items to select (also several) and reorder by dragging. |
//! | [`Tree`] | Nodes that expand and collapse, with selection, for any amount of data. |
//! | [`Table`] | Rows and columns with a fixed header, resizable and sortable columns, millions of rows. |
//! | [`PropertyGrid`] | Names and editable values in two aligned columns, by sections (an inspector). |
//!
//! **Charts**
//!
//! | Component | What it is |
//! |---|---|
//! | [`Plot`] | Data on x/y axes: lines (solid, dashed, dotted), areas, points, bars (stacked, horizontal), histograms, heat maps, reference lines and text; ticks, grid, log scale, legend, tooltip, pan and zoom, live data with `PlotBuffer` and `Plot::follow_x`. |
//! | [`PieChart`] | A pie or donut chart with percentages and a legend. |
//!
//! **Menus**
//!
//! | Component | What it is |
//! |---|---|
//! | Menu ([`Ui::menu_button`]) | A button that opens a menu, with submenus and icons; menu bars. |
//! | Context menu ([`Response::context_menu`]) | A menu opened with a right click on a widget. |
//! | Popup ([`Ui::popup_below`]) | Any content in a popup below a widget. |
//!
//! # Crates
//!
//! This crate re-exports everything an application needs. Underneath:
//! `rustroke-core` (geometry, shapes, tessellation, input), `rustroke-text` (text
//! layout), `rustroke-render` (wgpu renderer), `rustroke-widgets` (immediate-mode
//! API) and `rustroke-winit` (window and event loop).

pub use rustroke_core::{
    Color, ColorImage, DisplayList, Event, FillRule, Galley, Gradient, GradientKind, ImeEvent,
    InputState, Key, KeyboardShortcut, Mesh, Modifiers, POINTS_PER_SCROLL_LINE, PaintCallback,
    PhysicalSize, Point, PointerButton, RawInput, Rect, Shadow, Shape, Stroke, TextureId,
    TexturesDelta, Transform, Vec2, Vertex, point, vec2,
};
/// Drawing on the CPU, without a GPU (see `testing::Harness::render_software`).
pub use rustroke_soft::SoftwareRenderer;
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
    LineHighlight, List, ListResponse, Margin, Modal, ModalResponse, Numeric, Order, Panel,
    PanelSide, ProgressBar, PropertyGrid, PropertyGridUi, RadioButton, ReferenceField,
    RepaintHandle, Response, ScrollArea, SearchField, SelectableLabel, Sense, Separator, Slider,
    SortOrder, Spinner, SplitAxis, Style, TabBar, TabBarResponse, TabLabel, Table, TableResponse,
    TextEdit, TextureHandle, TimePicker, Toast, ToastLevel, ToolButton, ToolButtonResponse, Tree,
    TreeResponse, Ui, UiRoot, Visuals, Widget, WidgetDescription, WidgetInfo, WidgetRole, Window,
    show_inspector, show_toasts,
};
pub use rustroke_widgets::{
    Axis, Bar, BarChart, Colormap, Corner, HLine, HeatMap, Line, LineStyle, MarkerShape, PieChart,
    PieResponse, Plot, PlotBounds, PlotBuffer, PlotPoints, PlotResponse, PlotText, PlotUi, Points,
    TickFormatter, Ticks, VLine,
};
pub use rustroke_widgets::{
    Background, BoxStyle, GridCell, ImageFit, Justify, Sizing, Stack, StackAlign, StackDirection,
    StackItem, Track,
};
#[allow(deprecated)]
pub use rustroke_widgets::{
    Container, Flex, FlexAlign, FlexDirection, FlexGrid, FlexItem, FlexJustify, FlexUi, GridUi,
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
