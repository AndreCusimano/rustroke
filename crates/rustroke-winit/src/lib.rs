//! Desktop integration: window creation and the event loop, built on winit.

mod input;
mod integration;
mod native;
pub mod testing;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use web_time::Instant;

use input::InputCollector;
use rustroke_core::{
    Color, DisplayList, Galley, InputState, PhysicalSize, Point, RawInput, Rect, Tessellator, point,
};
use rustroke_render::{PaintJob, RenderOutcome, Renderer, RendererError};

/// The AccessKit version rustroke uses (accessibility trees in tests).
pub use accesskit;
/// The wgpu version rustroke uses (for [`Frame::wgpu`] and native textures):
/// use these types so the application and rustroke share one GPU device.
pub use integration::{EventResponse, Integration, RunOutput};
pub use native::{NativeMenu, NativeMenuItem, TrayOptions, notify};
pub use rustroke_render::{CallbackFn, CallbackInfo, wgpu};
use rustroke_text::{Fonts, TextStyle};
use rustroke_widgets::{
    CentralPanel, Context, CursorIcon, Id, RepaintHandle, TextureHandle, Ui, UiRoot,
};
/// The winit version rustroke uses, for applications with their own event
/// loop ([`Integration`]).
pub use winit;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

/// Options for the native window.
#[derive(Clone, Debug)]
pub struct WindowOptions {
    /// Window title.
    pub title: String,
    /// Initial inner size in logical (DPI-independent) points.
    pub inner_size: (f64, f64),
    /// On macOS, a transparent title bar with the content extending under
    /// it: the app draws its own top bar (menus, document name) next to
    /// the window buttons, leaving the first [`Frame::titlebar_height`]
    /// points free of widgets on the left. Elsewhere it has no effect.
    pub unified_titlebar: bool,
    /// With the `persistence` feature: a name for this app's saved state
    /// (e.g. "com.example.editor"). The UI state (window positions, panel
    /// sizes, open sections, table columns), the native window's position
    /// and size, and the values set with [`Frame::set_value`] are saved
    /// when the app exits and restored when it starts, in the platform's
    /// configuration folder. Main window only.
    pub persistence_id: Option<String>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "rustroke app".to_owned(),
            inner_size: (800.0, 600.0),
            unified_titlebar: false,
            persistence_id: None,
        }
    }
}

/// Height of the macOS title bar in points, which a unified title bar
/// overlaps.
const MACOS_TITLEBAR_HEIGHT: f32 = 28.0;

/// Per-frame information and output, passed to [`App::update`].
#[derive(Debug)]
pub struct Frame<'a> {
    /// Time since the app started.
    pub time: Duration,
    /// The window's drawable area in logical points; `min` is always zero.
    pub screen_rect: Rect,
    /// Physical pixels per logical point (the DPI scale factor).
    pub pixels_per_point: f32,
    /// Shapes to draw this frame, back to front, in logical points.
    pub shapes: DisplayList,
    /// Color the window is cleared to before drawing.
    pub clear_color: Color,
    /// Set to `true` to draw another frame right away (e.g. for animations).
    pub request_repaint: bool,
    fonts: &'a mut Fonts,
    ctx: &'a mut Context,
    /// The GPU renderer; `None` when the frame isn't drawn to a window
    /// (e.g. in tests).
    renderer: Option<&'a mut Renderer>,
    /// A new window title requested this frame.
    title: Option<String>,
    /// Extra windows to keep open (main window only).
    windows: Vec<(Id, WindowOptions)>,
    /// Which window this frame is for (`None`: the main window).
    window_id: Option<Id>,
    /// [`Frame::close`] was called.
    close: bool,
    /// Height of the title bar drawn over the content, in points.
    titlebar_height: f32,
    /// App values kept between runs (see [`Frame::set_value`]).
    storage: &'a mut std::collections::BTreeMap<String, String>,
    /// Native menu bar and tray icon asked for this frame.
    platform: PlatformRequests,
    /// Native menu choices and tray clicks since the last frame.
    platform_events: PlatformEvents,
}

/// What a frame asked of the platform (menus, tray icon).
#[derive(Clone, Debug, Default)]
struct PlatformRequests {
    menu: Option<Vec<NativeMenu>>,
    tray: Option<Option<TrayOptions>>,
}

/// Native menu choices and tray clicks for a frame.
#[derive(Clone, Debug, Default)]
struct PlatformEvents {
    menu: Vec<String>,
    tray_clicked: bool,
}

impl Frame<'_> {
    /// Keeps a second native window open (e.g. an assembly view on another
    /// monitor), from the main window's [`App::update`]: call it every frame
    /// while the window should exist; when a frame doesn't call it, the
    /// window closes. Its content comes from [`App::update_window`] with
    /// the same `id`. Each window has its own context (focus, style,
    /// textures); fonts and icons are shared.
    pub fn show_window(&mut self, id: Id, options: WindowOptions) {
        if !self.windows.iter().any(|(w, _)| *w == id) {
            self.windows.push((id, options));
        }
    }

    /// Closes the window this frame is drawn for, without asking
    /// [`App::on_close_requested`] (e.g. once the user has answered a
    /// "save changes?" dialog). Closing the main window ends [`run`]; for
    /// an extra window, also stop calling [`Frame::show_window`] for it, or
    /// it opens again.
    pub fn close(&mut self) {
        self.close = true;
    }

    /// Height of the title bar overlapping the top of the window, in
    /// points: 28 on macOS with [`WindowOptions::unified_titlebar`], else 0.
    /// The window buttons sit in its left part (about 70 points wide);
    /// dragging an empty part of it moves the window.
    pub fn titlebar_height(&self) -> f32 {
        self.titlebar_height
    }

    /// A value saved with [`Frame::set_value`] (in this run, or in an
    /// earlier one with [`WindowOptions::persistence_id`]).
    pub fn value(&self, key: &str) -> Option<&str> {
        self.storage.get(key).map(String::as_str)
    }

    /// Keeps a string value under `key` (e.g. the app's settings, as JSON)
    /// and, with [`WindowOptions::persistence_id`] and the `persistence`
    /// feature, saves it for the next run.
    pub fn set_value(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.storage.insert(key.into(), value.into());
    }

    /// Shows `menus` in the native menu bar (macOS, with the
    /// `native-menu` feature; after an application menu with About, Hide
    /// and Quit). Call it every frame, or whenever the menus change: they
    /// are rebuilt only when different. Choices arrive in
    /// [`Frame::native_menu_events`]. Elsewhere it does nothing, so draw
    /// your own menu bar there (`ui.menu_button`).
    pub fn set_native_menu(&mut self, menus: Vec<NativeMenu>) {
        self.platform.menu = Some(menus);
    }

    /// Shows (or with `None` removes) an icon in the system tray (macOS,
    /// Windows; feature `tray`). Its menu's choices arrive in
    /// [`Frame::native_menu_events`]; clicks on the icon in
    /// [`Frame::tray_clicked`].
    pub fn set_tray(&mut self, tray: Option<TrayOptions>) {
        self.platform.tray = Some(tray);
    }

    /// The ids of the native menu and tray menu entries chosen since the
    /// last frame.
    pub fn native_menu_events(&self) -> &[String] {
        &self.platform_events.menu
    }

    /// Whether the tray icon was clicked since the last frame.
    pub fn tray_clicked(&self) -> bool {
        self.platform_events.tray_clicked
    }

    /// Whether [`Frame::close`] was called this frame.
    pub fn close_requested(&self) -> bool {
        self.close
    }

    /// The extra window this frame is drawn for (`None`: the main window).
    pub fn window_id(&self) -> Option<Id> {
        self.window_id
    }

    /// Changes the window title (e.g. the document name, with `*` when
    /// there are unsaved changes). Cheap to call every frame: the window is
    /// only updated when the title changes.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// The GPU device and queue rustroke draws with, to render into the
    /// application's own textures (e.g. a 3D viewport) and show them with
    /// [`Frame::register_native_texture`]. `None` without a window.
    ///
    /// Submit the application's command buffers to this queue during
    /// `update`: they run before the UI is drawn, so the result is visible
    /// in the same frame.
    pub fn wgpu(&self) -> Option<(&wgpu::Device, &wgpu::Queue)> {
        self.renderer.as_deref().map(|r| (r.device(), r.queue()))
    }

    /// Format of the window's render target, for pipelines of
    /// `rustroke_render::CallbackFn`s. `None` without a window.
    pub fn wgpu_target_format(&self) -> Option<wgpu::TextureFormat> {
        self.renderer.as_deref().map(|r| r.target_format())
    }

    /// Shows an application-owned wgpu texture as an image, without copying
    /// it. Returns a handle for [`rustroke_widgets::Image`] or
    /// `DisplayList::image`; the texture is released (by rustroke) when the
    /// last clone of the handle is dropped. `None` without a window.
    ///
    /// The view must be a filterable float 2D texture (e.g.
    /// `Rgba8UnormSrgb`, usage `TEXTURE_BINDING`); its colors are read as
    /// linear with premultiplied alpha. When the application recreates the
    /// texture (e.g. on resize), call [`Frame::update_native_texture`].
    pub fn register_native_texture(
        &mut self,
        view: &wgpu::TextureView,
        size: [u32; 2],
    ) -> Option<TextureHandle> {
        let renderer = self.renderer.as_deref_mut()?;
        let handle = self.ctx.allocate_texture(size);
        renderer.register_native_texture(handle.id(), view);
        Some(handle)
    }

    /// Points an existing native texture handle to a new view (and size).
    pub fn update_native_texture(
        &mut self,
        handle: &TextureHandle,
        view: &wgpu::TextureView,
        size: [u32; 2],
    ) {
        if let Some(renderer) = self.renderer.as_deref_mut() {
            renderer.register_native_texture(handle.id(), view);
            handle.set_size(size);
        }
    }

    /// A handle to wake the UI up from other threads (e.g. when a
    /// background computation finishes).
    pub fn repaint_handle(&self) -> RepaintHandle {
        self.ctx.repaint_handle()
    }

    /// Lays out and draws widgets in the space not taken by panels (the
    /// central panel), minus the style's window margin. Show panels
    /// (`Panel::top(..).show(frame, ..)`) before calling this. Widgets are
    /// drawn on top of everything in [`Frame::shapes`].
    pub fn ui<R>(&mut self, add_contents: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        CentralPanel.show(self, add_contents)
    }

    /// Mouse, keyboard and timing input for this frame.
    pub fn input(&self) -> &InputState {
        self.ctx.input()
    }

    /// Widget state kept between frames: style, focus, ...
    pub fn ctx(&mut self) -> &mut Context {
        self.ctx
    }

    /// Lays out text without drawing it, e.g. to measure it with
    /// [`Galley::size`] before deciding where to put it. Lines longer than
    /// `wrap_width` points are wrapped. Cached across frames.
    pub fn layout_text(
        &mut self,
        text: &str,
        style: &TextStyle,
        wrap_width: Option<f32>,
    ) -> Arc<Galley> {
        self.fonts
            .layout(text, style, wrap_width, self.pixels_per_point)
    }

    /// Draws `text` with its top-left corner at `pos` and returns the area
    /// it occupies.
    pub fn text(&mut self, pos: Point, text: &str, style: &TextStyle, color: Color) -> Rect {
        let galley = self.layout_text(text, style, None);
        let rect = Rect::from_min_size(pos, galley.size);
        self.shapes.galley(pos, galley, color);
        rect
    }

    /// Draws `text` wrapped to `wrap_width` points and returns the area it
    /// occupies.
    pub fn text_wrapped(
        &mut self,
        pos: Point,
        text: &str,
        style: &TextStyle,
        wrap_width: f32,
        color: Color,
    ) -> Rect {
        let galley = self.layout_text(text, style, Some(wrap_width));
        let rect = Rect::from_min_size(pos, galley.size);
        self.shapes.galley(pos, galley, color);
        rect
    }

    /// The font database and glyph atlas, for advanced use.
    pub fn fonts(&mut self) -> &mut Fonts {
        self.fonts
    }
}

impl UiRoot for Frame<'_> {
    fn parts(&mut self) -> (&mut Context, &mut Fonts) {
        (&mut *self.ctx, &mut *self.fonts)
    }
}

/// An application driven by [`run`].
pub trait App {
    /// Called once per frame. The window only redraws when something
    /// happened (input, resize, ...), so idle apps use no CPU.
    fn update(&mut self, frame: &mut Frame<'_>);

    /// The user asked to close the window. Return `false` to keep it open
    /// (e.g. to ask whether to save first, then close by returning `true`
    /// on a later request). Default: close.
    fn on_close_requested(&mut self) -> bool {
        true
    }

    /// Draws the extra window `id` (opened with [`Frame::show_window`]).
    /// Called when that window needs a frame, independently of the main
    /// window. Default: nothing.
    fn update_window(&mut self, _id: Id, _frame: &mut Frame<'_>) {}

    /// The user asked to close the extra window `id`. Return `true` to
    /// close it, and stop calling [`Frame::show_window`] for it (e.g. clear
    /// the flag that shows it), or it opens again. Default: close.
    fn on_window_close_requested(&mut self, _id: Id) -> bool {
        true
    }
}

/// Any closure taking the frame is an app, for small programs:
///
/// ```no_run
/// rustroke_winit::run(Default::default(), |frame: &mut rustroke_winit::Frame| {
///     frame.ui(|ui| ui.label("Hello!"));
/// })
/// .unwrap();
/// ```
impl<F: FnMut(&mut Frame<'_>)> App for F {
    fn update(&mut self, frame: &mut Frame<'_>) {
        self(frame);
    }
}

/// Errors returned by [`run`].
#[derive(Debug)]
pub enum RunError {
    /// The event loop failed.
    EventLoop(winit::error::EventLoopError),
    /// The window could not be created.
    CreateWindow(winit::error::OsError),
    /// The GPU renderer could not be set up.
    Renderer(RendererError),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(e) => write!(f, "event loop error: {e}"),
            Self::CreateWindow(e) => write!(f, "failed to create window: {e}"),
            Self::Renderer(e) => write!(f, "renderer error: {e}"),
        }
    }
}

impl std::error::Error for RunError {}

/// Events sent to the event loop from other threads.
#[derive(Debug)]
enum UserEvent {
    /// From the accessibility adapter (screen reader requests).
    #[cfg(not(target_arch = "wasm32"))]
    AccessKit(accesskit_winit::Event),
    /// A `RepaintHandle` asked for a frame.
    Repaint,
}

#[cfg(not(target_arch = "wasm32"))]
impl From<accesskit_winit::Event> for UserEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

/// Opens a window and runs `app` until the window is closed.
///
/// In the browser (wasm32) the app draws into a canvas added to the page,
/// and this returns right away: the browser drives the event loop.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(options: WindowOptions, app: impl App) -> Result<(), RunError> {
    run_impl(options, app)
}

/// Opens a window and runs `app` until the window is closed.
///
/// In the browser (wasm32) the app draws into a canvas added to the page,
/// and this returns right away: the browser drives the event loop.
#[cfg(target_arch = "wasm32")]
pub fn run(options: WindowOptions, app: impl App + 'static) -> Result<(), RunError> {
    run_impl(options, app)
}

/// The event loop and the runner driving `app` in it.
#[cfg_attr(not(feature = "persistence"), allow(unused_mut))]
fn make_runner<A: App>(
    options: WindowOptions,
    app: A,
) -> Result<(EventLoop<UserEvent>, Runner<A>), RunError> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(RunError::EventLoop)?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut runner = Runner {
        options,
        app,
        start: Instant::now(),
        // Browsers have no system fonts to load.
        fonts: if cfg!(target_arch = "wasm32") {
            Fonts::bundled_only()
        } else {
            Fonts::new()
        },
        proxy,
        main: None,
        extras: Vec::new(),
        requested: Vec::new(),
        error: None,
        storage: std::collections::BTreeMap::new(),
        restored_ui: None,
        restored_position: None,
        #[cfg(all(
            any(feature = "native-menu", feature = "tray"),
            any(target_os = "macos", target_os = "windows")
        ))]
        native: Default::default(),
    };
    #[cfg(feature = "persistence")]
    persistence::restore(&mut runner);
    Ok((event_loop, runner))
}

#[cfg(not(target_arch = "wasm32"))]
fn run_impl<A: App>(options: WindowOptions, app: A) -> Result<(), RunError> {
    let (event_loop, mut runner) = make_runner(options, app)?;
    event_loop
        .run_app(&mut runner)
        .map_err(RunError::EventLoop)?;
    runner.error.map_or(Ok(()), Err)
}

/// In the browser the event loop runs after this returns, driven by the
/// page's events.
#[cfg(target_arch = "wasm32")]
fn run_impl<A: App + 'static>(options: WindowOptions, app: A) -> Result<(), RunError> {
    use winit::platform::web::EventLoopExtWebSys;
    // Panics would otherwise only show as "unreachable" in the console.
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into());
    }));
    let (event_loop, runner) = make_runner(options, app)?;
    event_loop.spawn_app(runner);
    Ok(())
}

struct Runner<A> {
    options: WindowOptions,
    app: A,
    start: Instant,
    /// Shared by every window (glyphs, icons).
    fonts: Fonts,
    proxy: EventLoopProxy<UserEvent>,
    main: Option<WindowState>,
    /// Windows opened with [`Frame::show_window`].
    extras: Vec<(Id, WindowState)>,
    /// The windows the last main frame asked for.
    requested: Vec<(Id, WindowOptions)>,
    error: Option<RunError>,
    /// App values (see [`Frame::set_value`]).
    storage: std::collections::BTreeMap<String, String>,
    /// Saved UI state to restore into the main window's context.
    #[cfg_attr(not(feature = "persistence"), allow(dead_code))]
    restored_ui: Option<String>,
    /// Saved position of the main window, in logical pixels.
    restored_position: Option<(f64, f64)>,
    /// The native menu bar and tray icon.
    #[cfg(all(
        any(feature = "native-menu", feature = "tray"),
        any(target_os = "macos", target_os = "windows")
    ))]
    native: native::platform::Native,
}

/// One native window with its own UI state and renderer.
struct WindowState {
    window: Arc<Window>,
    /// `None` until the GPU is set up (in the browser that happens
    /// asynchronously, see `pending_renderer`).
    renderer: Option<Renderer>,
    /// In the browser: where the renderer arrives once created.
    #[cfg(target_arch = "wasm32")]
    pending_renderer: std::rc::Rc<std::cell::RefCell<Option<Result<Renderer, RendererError>>>>,
    /// Connects the UI to the platform's screen readers.
    #[cfg(not(target_arch = "wasm32"))]
    accesskit: accesskit_winit::Adapter,
    ctx: Context,
    input: InputCollector,
    clear_color: Color,
    title: String,
    /// Cursor currently shown, to avoid setting it every frame.
    cursor: CursorIcon,
    /// A frame was requested for this time (e.g. to show a tooltip).
    repaint_at: Option<Instant>,
    /// Whether the input method is currently enabled on the window.
    ime_allowed: bool,
    /// Kept between frames to reuse its allocation.
    shapes: DisplayList,
    /// See [`Frame::titlebar_height`].
    titlebar_height: f32,
}

impl WindowState {
    fn new(
        event_loop: &ActiveEventLoop,
        options: &WindowOptions,
        fonts: &Fonts,
        proxy: &EventLoopProxy<UserEvent>,
    ) -> Result<Self, RunError> {
        let (w, h) = options.inner_size;
        // AccessKit must be attached before the window is first shown.
        let attributes = Window::default_attributes()
            .with_title(&options.title)
            .with_inner_size(LogicalSize::new(w, h))
            .with_visible(false);
        #[cfg(target_os = "macos")]
        let attributes = {
            use winit::platform::macos::WindowAttributesExtMacOS;
            attributes
                .with_titlebar_transparent(options.unified_titlebar)
                .with_title_hidden(options.unified_titlebar)
                .with_fullsize_content_view(options.unified_titlebar)
        };
        // In the browser the window is a canvas added to the page.
        #[cfg(target_arch = "wasm32")]
        let attributes = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes.with_append(true)
        };
        let titlebar_height = if cfg!(target_os = "macos") && options.unified_titlebar {
            MACOS_TITLEBAR_HEIGHT
        } else {
            0.0
        };
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(RunError::CreateWindow)?,
        );
        #[cfg(not(target_arch = "wasm32"))]
        let accesskit =
            accesskit_winit::Adapter::with_event_loop_proxy(event_loop, &window, proxy.clone());
        // In the browser the main canvas fills the page and follows its
        // size (winit reports the changes as `Resized`).
        #[cfg(target_arch = "wasm32")]
        if let Some(canvas) = winit::platform::web::WindowExtWebSys::canvas(&*window) {
            let style = canvas.style();
            for (name, value) in [
                ("position", "fixed"),
                ("left", "0"),
                ("top", "0"),
                ("width", "100vw"),
                ("height", "100vh"),
                ("display", "block"),
                ("outline", "none"),
            ] {
                let _ = style.set_property(name, value);
            }
        }
        window.set_visible(true);
        let size = to_physical(window.inner_size());
        #[cfg(not(target_arch = "wasm32"))]
        let renderer = Some(
            pollster::block_on(Renderer::new(Arc::clone(&window), size, fonts.atlas()))
                .map_err(RunError::Renderer)?,
        );
        // Browsers set up the GPU asynchronously: frames start once it is
        // ready.
        #[cfg(target_arch = "wasm32")]
        let (renderer, pending_renderer) = {
            let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
            let (target, atlas, ready) = (
                Arc::clone(&window),
                fonts.atlas().clone(),
                std::rc::Rc::clone(&slot),
            );
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = Renderer::new(Arc::clone(&target), size, &atlas).await;
                *ready.borrow_mut() = Some(renderer);
                target.request_redraw();
            });
            (None, slot)
        };
        let mut ctx = Context::new();
        let wake = std::sync::Mutex::new(proxy.clone());
        ctx.set_repaint_callback(move || {
            let proxy = wake
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Fails only if the event loop has already exited.
            let _ = proxy.send_event(UserEvent::Repaint);
        });
        window.request_redraw();
        Ok(Self {
            window,
            renderer,
            #[cfg(target_arch = "wasm32")]
            pending_renderer,
            #[cfg(not(target_arch = "wasm32"))]
            accesskit,
            ctx,
            input: InputCollector::default(),
            clear_color: Color::BLACK,
            title: options.title.clone(),
            cursor: CursorIcon::Default,
            repaint_at: None,
            ime_allowed: false,
            shapes: DisplayList::new(),
            titlebar_height,
        })
    }

    /// Runs one frame: `update` describes the UI, then it is drawn.
    /// Returns the windows the frame asked for, whether it handled input
    /// (which may have changed state other windows show) and whether the
    /// app closed the window.
    fn redraw(
        &mut self,
        fonts: &mut Fonts,
        storage: &mut std::collections::BTreeMap<String, String>,
        platform_events: PlatformEvents,
        start: Instant,
        window_id: Option<Id>,
        update: impl FnOnce(&mut Frame<'_>),
    ) -> (Vec<(Id, WindowOptions)>, bool, bool, PlatformRequests) {
        // Window scale factors are small values like 1.0, 1.5 or 2.0.
        #[allow(clippy::cast_possible_truncation)]
        let pixels_per_point = self.window.scale_factor() as f32 * self.ctx.zoom_factor();
        #[cfg(target_arch = "wasm32")]
        if self.renderer.is_none()
            && let Some(result) = self.pending_renderer.borrow_mut().take()
        {
            match result {
                Ok(mut renderer) => {
                    // The canvas may have been resized while the GPU was
                    // being set up.
                    renderer.resize(to_physical(self.window.inner_size()));
                    self.renderer = Some(renderer);
                }
                Err(e) => {
                    web_sys::console::error_1(
                        &format!("rustroke: could not set up the GPU: {e}").into(),
                    );
                }
            }
        }
        let Some(renderer) = self.renderer.as_mut() else {
            // Not ready yet: a frame is requested when it is.
            return (Vec::new(), false, false, PlatformRequests::default());
        };
        let size = renderer.size();
        let screen_rect = Rect::from_min_max(
            point(0.0, 0.0),
            point(
                size.width as f32 / pixels_per_point,
                size.height as f32 / pixels_per_point,
            ),
        );
        let time = start.elapsed();
        let events = self.input.take_events();
        let had_input = !events.is_empty();
        self.ctx.begin_frame(RawInput {
            time: time.as_secs_f64(),
            screen_rect,
            pixels_per_point,
            events,
        });
        let mut shapes = std::mem::take(&mut self.shapes);
        shapes.clear();
        let mut frame = Frame {
            time,
            screen_rect,
            pixels_per_point,
            shapes,
            clear_color: self.clear_color,
            request_repaint: false,
            fonts: &mut *fonts,
            ctx: &mut self.ctx,
            renderer: Some(&mut *renderer),
            title: None,
            windows: Vec::new(),
            window_id,
            close: false,
            titlebar_height: self.titlebar_height,
            storage,
            platform: PlatformRequests::default(),
            platform_events,
        };
        update(&mut frame);
        rustroke_widgets::show_toasts(&mut frame);
        rustroke_widgets::show_inspector(&mut frame);
        let Frame {
            shapes,
            clear_color,
            request_repaint,
            title,
            windows,
            close,
            platform,
            ..
        } = frame;
        if let Some(title) = title
            && title != self.title
        {
            self.window.set_title(&title);
            self.title = title;
        }
        self.clear_color = clear_color;
        fonts.end_frame();
        let mut output = self.ctx.end_frame();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(update) = output.accesskit_update.take() {
            self.accesskit.update_if_active(|| update);
        }
        let mut shapes = shapes;
        shapes.append(&mut output.shapes);
        self.repaint_at = output
            .repaint_after
            .map(|secs| Instant::now() + Duration::from_secs_f64(secs));
        if let Some(text) = output.copied_text.take() {
            self.input.set_clipboard_text(text);
        }
        if let Some(url) = output.open_url.take() {
            open_url(&url);
        }
        if output.ime_cursor.is_some() != self.ime_allowed {
            self.ime_allowed = output.ime_cursor.is_some();
            self.window.set_ime_allowed(self.ime_allowed);
        }
        if let Some(r) = output.ime_cursor {
            // winit's logical units don't include the zoom.
            let z = self.ctx.zoom_factor();
            self.window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(r.min.x * z, r.min.y * z),
                winit::dpi::LogicalSize::new(r.width().max(1.0) * z, r.height() * z),
            );
        }
        if output.cursor != self.cursor {
            self.cursor = output.cursor;
            self.window.set_cursor(input::cursor_icon(output.cursor));
        }

        // After update: laying out text may have grown the atlas.
        let meshes = Tessellator::new(pixels_per_point, fonts.atlas()).tessellate(&shapes);
        self.shapes = shapes;
        let job = PaintJob {
            meshes: &meshes,
            textures: &output.textures,
            pixels_per_point,
            clear_color,
        };
        self.window.pre_present_notify();
        let outcome = renderer.render(&job, fonts.atlas_mut());
        if outcome == RenderOutcome::Retry || request_repaint || output.repaint {
            self.window.request_redraw();
        }
        (windows, had_input, close, platform)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_accesskit(&mut self, event: accesskit_winit::WindowEvent) {
        match event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.ctx.set_accessibility_active(true);
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                self.ctx.accesskit_action(request);
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.ctx.set_accessibility_active(false);
            }
        }
        self.window.request_redraw();
    }
}

impl<A: App> Runner<A> {
    fn all_windows(&self) -> impl Iterator<Item = &WindowState> {
        self.main.iter().chain(self.extras.iter().map(|(_, w)| w))
    }

    /// Opens the windows the app asked for and closes the others.
    fn sync_windows(&mut self, event_loop: &ActiveEventLoop) {
        let requested = std::mem::take(&mut self.requested);
        self.extras
            .retain(|(id, _)| requested.iter().any(|(r, _)| r == id));
        for (id, options) in &requested {
            if let Some((_, state)) = self.extras.iter_mut().find(|(e, _)| e == id) {
                if options.title != state.title {
                    state.window.set_title(&options.title);
                    state.title.clone_from(&options.title);
                }
                continue;
            }
            match WindowState::new(event_loop, options, &self.fonts, &self.proxy) {
                Ok(state) => self.extras.push((*id, state)),
                Err(e) => log::error!("could not open window {:?}: {e}", options.title),
            }
        }
        self.requested = requested;
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop, window: WindowId) {
        let Self {
            app,
            fonts,
            start,
            storage,
            ..
        } = self;
        let had_input;
        if let Some(main) = self.main.as_mut().filter(|m| m.window.id() == window) {
            #[cfg(all(
                any(feature = "native-menu", feature = "tray"),
                any(target_os = "macos", target_os = "windows")
            ))]
            let events = {
                let e = self.native.take_events();
                PlatformEvents {
                    menu: e.menu,
                    tray_clicked: e.tray_clicked,
                }
            };
            #[cfg(not(all(
                any(feature = "native-menu", feature = "tray"),
                any(target_os = "macos", target_os = "windows")
            )))]
            let events = PlatformEvents::default();
            let (requested, input, close, platform) =
                main.redraw(fonts, storage, events, *start, None, |frame| {
                    app.update(frame)
                });
            #[cfg(all(
                any(feature = "native-menu", feature = "tray"),
                any(target_os = "macos", target_os = "windows")
            ))]
            {
                let proxy = std::sync::Arc::new(std::sync::Mutex::new(self.proxy.clone()));
                let wake = move || {
                    let proxy = proxy
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let _ = proxy.send_event(UserEvent::Repaint);
                };
                if let Some(menus) = &platform.menu {
                    self.native
                        .set_menu(menus, &self.options.title, wake.clone());
                }
                if let Some(tray) = &platform.tray {
                    self.native.set_tray(tray.as_ref(), wake);
                }
            }
            #[cfg(not(all(
                any(feature = "native-menu", feature = "tray"),
                any(target_os = "macos", target_os = "windows")
            )))]
            let _ = platform;
            if close {
                event_loop.exit();
                return;
            }
            had_input = input;
            let changed = requested
                .iter()
                .map(|(id, _)| id)
                .ne(self.requested.iter().map(|(id, _)| id))
                || requested
                    .iter()
                    .zip(&self.requested)
                    .any(|((_, a), (_, b))| a.title != b.title);
            if changed {
                self.requested = requested;
                self.sync_windows(event_loop);
            }
        } else if let Some((id, state)) = self
            .extras
            .iter_mut()
            .find(|(_, s)| s.window.id() == window)
        {
            let id = *id;
            let close;
            (_, had_input, close, _) = state.redraw(
                fonts,
                storage,
                PlatformEvents::default(),
                *start,
                Some(id),
                |frame| {
                    app.update_window(id, frame);
                },
            );
            if close {
                self.extras.retain(|(k, _)| *k != id);
                self.requested.retain(|(k, _)| *k != id);
                if let Some(main) = &self.main {
                    main.window.request_redraw();
                }
                return;
            }
        } else {
            return;
        }
        // Input in one window may change what the others show (shared app
        // state). Frames without input don't propagate, so this can't loop.
        if had_input {
            for state in self.all_windows().filter(|s| s.window.id() != window) {
                state.window.request_redraw();
            }
        }
    }

    fn window_state(&mut self, window: WindowId) -> Option<&mut WindowState> {
        if self.main.as_ref().is_some_and(|m| m.window.id() == window) {
            return self.main.as_mut();
        }
        self.extras
            .iter_mut()
            .find(|(_, s)| s.window.id() == window)
            .map(|(_, s)| s)
    }
}

impl<A: App> ApplicationHandler<UserEvent> for Runner<A> {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            #[cfg(not(target_arch = "wasm32"))]
            UserEvent::AccessKit(event) => {
                if let Some(state) = self.window_state(event.window_id) {
                    state.on_accesskit(event.window_event);
                }
            }
            UserEvent::Repaint => {
                for state in self.all_windows() {
                    state.window.request_redraw();
                }
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(feature = "persistence")]
        persistence::save(self);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        let windows = self
            .main
            .iter_mut()
            .chain(self.extras.iter_mut().map(|(_, w)| w));
        for state in windows {
            match state.repaint_at {
                Some(at) if at <= now => {
                    state.repaint_at = None;
                    state.window.request_redraw();
                }
                Some(at) => next = Some(next.map_or(at, |n| n.min(at))),
                None => {}
            }
        }
        event_loop.set_control_flow(match next {
            Some(at) => ControlFlow::WaitUntil(at),
            None => ControlFlow::Wait,
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main.is_some() {
            return;
        }
        match WindowState::new(event_loop, &self.options, &self.fonts, &self.proxy) {
            #[cfg_attr(not(feature = "persistence"), allow(unused_mut))]
            Ok(mut state) => {
                if let Some((x, y)) = self.restored_position.take() {
                    state
                        .window
                        .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
                }
                #[cfg(feature = "persistence")]
                if let Some(ui) = self.restored_ui.take()
                    && let Err(e) = state.ctx.load_state(&ui)
                {
                    log::warn!("could not restore the UI state: {e}");
                }
                self.main = Some(state);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let is_main = self.main.as_ref().is_some_and(|m| m.window.id() == id);
        let Some(state) = self.window_state(id) else {
            return;
        };
        #[cfg(not(target_arch = "wasm32"))]
        state.accesskit.process_event(&state.window, &event);
        match event {
            WindowEvent::CloseRequested if is_main => {
                if self.app.on_close_requested() {
                    event_loop.exit();
                } else if let Some(main) = &self.main {
                    // The app may show a "save changes?" dialog now.
                    main.window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => {
                let Some(key) = self
                    .extras
                    .iter()
                    .find(|(_, s)| s.window.id() == id)
                    .map(|(key, _)| *key)
                else {
                    return;
                };
                if self.app.on_window_close_requested(key) {
                    self.extras.retain(|(k, _)| *k != key);
                    self.requested.retain(|(k, _)| *k != key);
                }
                // Let the main window catch up (e.g. a "show" checkbox).
                if let Some(main) = &self.main {
                    main.window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = state.renderer.as_mut() {
                    renderer.resize(to_physical(size));
                }
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop, id),
            // Frames are skipped while the window is hidden; draw again as
            // soon as it becomes visible.
            WindowEvent::Occluded(false) => state.window.request_redraw(),
            event => {
                if state.input.on_window_event(
                    &event,
                    state.window.scale_factor() * f64::from(state.ctx.zoom_factor()),
                ) && needs_frame(&event, &state.ctx, &state.input)
                {
                    state.window.request_redraw();
                }
            }
        }
    }
}

/// Saving and restoring state between runs (feature `persistence`).
#[cfg(feature = "persistence")]
mod persistence {
    use std::path::PathBuf;

    use super::{App, Runner};

    /// Where the state of app `id` is saved.
    fn path(id: &str) -> Option<PathBuf> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let base = if cfg!(target_os = "macos") {
            home?.join("Library/Application Support")
        } else if cfg!(target_os = "windows") {
            PathBuf::from(std::env::var_os("APPDATA")?)
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| home.map(|h| h.join(".config")))?
        };
        Some(base.join(id).join("rustroke-state.json"))
    }

    /// Reads the saved state, if any, into the runner before the window
    /// is created.
    pub(super) fn restore<A: App>(runner: &mut Runner<A>) {
        let Some(file) = runner.options.persistence_id.as_deref().and_then(path) else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&file) else {
            return;
        };
        let Ok(saved) = serde_json::from_str::<serde_json::Value>(&text) else {
            log::warn!("ignoring unreadable state file {}", file.display());
            return;
        };
        if let Some(ui) = saved.get("ui").and_then(|v| v.as_str()) {
            runner.restored_ui = Some(ui.to_owned());
        }
        if let Some(app) = saved.get("app").and_then(|v| v.as_object()) {
            for (k, v) in app {
                if let Some(v) = v.as_str() {
                    runner.storage.insert(k.clone(), v.to_owned());
                }
            }
        }
        if let Some([x, y, w, h]) = saved
            .get("window")
            .and_then(|v| serde_json::from_value::<[f64; 4]>(v.clone()).ok())
            && w >= 100.0
            && h >= 100.0
        {
            runner.options.inner_size = (w, h);
            runner.restored_position = Some((x, y));
        }
    }

    /// Writes the state of the main window and the app values.
    pub(super) fn save<A: App>(runner: &Runner<A>) {
        let (Some(file), Some(main)) = (
            runner.options.persistence_id.as_deref().and_then(path),
            runner.main.as_ref(),
        ) else {
            return;
        };
        let scale = main.window.scale_factor();
        let size = main.window.inner_size().to_logical::<f64>(scale);
        let window = main
            .window
            .outer_position()
            .ok()
            .map(|p| p.to_logical::<f64>(scale))
            .map(|p| [p.x, p.y, size.width, size.height]);
        let state = serde_json::json!({
            "ui": main.ctx.save_state(),
            "window": window,
            "app": runner.storage,
        });
        let write = || -> std::io::Result<()> {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(
                &file,
                serde_json::to_string_pretty(&state).unwrap_or_default(),
            )
        };
        if let Err(e) = write() {
            log::warn!("could not save the state to {}: {e}", file.display());
        }
    }
}

/// Whether input `event` (already collected) needs a new frame: pointer
/// moves only do when they can change what is shown.
pub(crate) fn needs_frame(event: &WindowEvent, ctx: &Context, input: &InputCollector) -> bool {
    match event {
        WindowEvent::CursorMoved { .. } => input
            .pointer()
            .is_none_or(|pos| ctx.pointer_move_needs_frame(pos)),
        _ => true,
    }
}

/// Opens `url` with the platform's default handler (the browser for web
/// links), without waiting for it.
pub(crate) fn open_url(url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(url).spawn();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let result = std::process::Command::new("xdg-open").arg(url).spawn();
        if let Err(e) = result {
            log::warn!("could not open {url}: {e}");
        }
    }
}

fn to_physical(size: winit::dpi::PhysicalSize<u32>) -> PhysicalSize {
    PhysicalSize::new(size.width, size.height)
}
