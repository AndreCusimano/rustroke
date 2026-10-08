//! Desktop integration: window creation and the event loop, built on winit.

mod input;
mod integration;
pub mod testing;

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use input::InputCollector;
use rustroke_core::{
    Color, DisplayList, Galley, InputState, PhysicalSize, Point, RawInput, Rect, Tessellator, point,
};
use rustroke_render::{PaintJob, RenderOutcome, Renderer, RendererError};

/// The wgpu version rustroke uses (for [`Frame::wgpu`] and native textures):
/// use these types so the application and rustroke share one GPU device.
pub use integration::{EventResponse, Integration, RunOutput};
pub use rustroke_render::{CallbackFn, CallbackInfo, wgpu};
use rustroke_text::{Fonts, TextStyle};
use rustroke_widgets::{
    CentralPanel, Context, CursorIcon, RepaintHandle, TextureHandle, Ui, UiRoot,
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
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "rustroke app".to_owned(),
            inner_size: (800.0, 600.0),
        }
    }
}

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
}

impl Frame<'_> {
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
    AccessKit(accesskit_winit::Event),
    /// A `RepaintHandle` asked for a frame.
    Repaint,
}

impl From<accesskit_winit::Event> for UserEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

/// Opens a window and runs `app` until the window is closed.
pub fn run(options: WindowOptions, app: impl App) -> Result<(), RunError> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(RunError::EventLoop)?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut ctx = Context::new();
    let wake = std::sync::Mutex::new(proxy.clone());
    ctx.set_repaint_callback(move || {
        let proxy = wake
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Fails only if the event loop has already exited.
        let _ = proxy.send_event(UserEvent::Repaint);
    });
    let mut runner = Runner {
        options,
        app,
        start: Instant::now(),
        clear_color: Color::BLACK,
        fonts: Fonts::new(),
        ctx,
        input: InputCollector::default(),
        cursor: CursorIcon::Default,
        repaint_at: None,
        ime_allowed: false,
        shapes: DisplayList::new(),
        proxy,
        state: None,
        error: None,
    };
    event_loop
        .run_app(&mut runner)
        .map_err(RunError::EventLoop)?;
    runner.error.map_or(Ok(()), Err)
}

struct Runner<A> {
    options: WindowOptions,
    app: A,
    start: Instant,
    clear_color: Color,
    fonts: Fonts,
    ctx: Context,
    input: InputCollector,
    /// Cursor currently shown, to avoid setting it every frame.
    cursor: CursorIcon,
    /// A frame was requested for this time (e.g. to show a tooltip).
    repaint_at: Option<Instant>,
    /// Whether the input method is currently enabled on the window.
    ime_allowed: bool,
    /// Kept between frames to reuse its allocation.
    shapes: DisplayList,
    proxy: EventLoopProxy<UserEvent>,
    state: Option<WindowState>,
    error: Option<RunError>,
}

struct WindowState {
    window: Arc<Window>,
    renderer: Renderer,
    /// Connects the UI to the platform's screen readers.
    accesskit: accesskit_winit::Adapter,
}

impl<A: App> Runner<A> {
    fn create_window(&self, event_loop: &ActiveEventLoop) -> Result<WindowState, RunError> {
        let (w, h) = self.options.inner_size;
        // AccessKit must be attached before the window is first shown.
        let attributes = Window::default_attributes()
            .with_title(&self.options.title)
            .with_inner_size(LogicalSize::new(w, h))
            .with_visible(false);
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(RunError::CreateWindow)?,
        );
        let accesskit = accesskit_winit::Adapter::with_event_loop_proxy(
            event_loop,
            &window,
            self.proxy.clone(),
        );
        window.set_visible(true);
        let size = to_physical(window.inner_size());
        let renderer =
            pollster::block_on(Renderer::new(Arc::clone(&window), size, self.fonts.atlas()))
                .map_err(RunError::Renderer)?;
        Ok(WindowState {
            window,
            renderer,
            accesskit,
        })
    }

    fn redraw(&mut self) {
        let Some(state) = &mut self.state else { return };
        // Window scale factors are small values like 1.0, 1.5 or 2.0.
        #[allow(clippy::cast_possible_truncation)]
        let pixels_per_point = state.window.scale_factor() as f32;
        let size = state.renderer.size();
        let screen_rect = Rect::from_min_max(
            point(0.0, 0.0),
            point(
                size.width as f32 / pixels_per_point,
                size.height as f32 / pixels_per_point,
            ),
        );
        let time = self.start.elapsed();
        self.ctx.begin_frame(RawInput {
            time: time.as_secs_f64(),
            screen_rect,
            pixels_per_point,
            events: self.input.take_events(),
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
            fonts: &mut self.fonts,
            ctx: &mut self.ctx,
            renderer: Some(&mut state.renderer),
            title: None,
        };
        self.app.update(&mut frame);
        let Frame {
            shapes,
            clear_color,
            request_repaint,
            title,
            ..
        } = frame;
        if let Some(title) = title
            && title != self.options.title
        {
            state.window.set_title(&title);
            self.options.title = title;
        }
        self.clear_color = clear_color;
        self.fonts.end_frame();
        let mut output = self.ctx.end_frame();
        if let Some(update) = output.accesskit_update.take() {
            state.accesskit.update_if_active(|| update);
        }
        let mut shapes = shapes;
        shapes.append(&mut output.shapes);
        self.repaint_at = output
            .repaint_after
            .map(|secs| Instant::now() + Duration::from_secs_f64(secs));
        if let Some(text) = output.copied_text.take() {
            self.input.set_clipboard_text(text);
        }
        if output.ime_cursor.is_some() != self.ime_allowed {
            self.ime_allowed = output.ime_cursor.is_some();
            state.window.set_ime_allowed(self.ime_allowed);
        }
        if let Some(r) = output.ime_cursor {
            state.window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(r.min.x, r.min.y),
                winit::dpi::LogicalSize::new(r.width().max(1.0), r.height()),
            );
        }
        if output.cursor != self.cursor {
            self.cursor = output.cursor;
            state.window.set_cursor(input::cursor_icon(output.cursor));
        }

        // After update: laying out text may have grown the atlas.
        let meshes = Tessellator::new(pixels_per_point, self.fonts.atlas()).tessellate(&shapes);
        self.shapes = shapes;
        let job = PaintJob {
            meshes: &meshes,
            textures: &output.textures,
            pixels_per_point,
            clear_color,
        };
        state.window.pre_present_notify();
        let outcome = state.renderer.render(&job, self.fonts.atlas_mut());
        if outcome == RenderOutcome::Retry || request_repaint || output.repaint {
            state.window.request_redraw();
        }
    }
}

impl<A: App> ApplicationHandler<UserEvent> for Runner<A> {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        let event = match event {
            UserEvent::AccessKit(event) => event,
            UserEvent::Repaint => {
                if let Some(state) = &self.state {
                    state.window.request_redraw();
                }
                return;
            }
        };
        match event.window_event {
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
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.repaint_at {
            Some(at) if at <= Instant::now() => {
                self.repaint_at = None;
                if let Some(state) = &self.state {
                    state.window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match self.create_window(event_loop) {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(state) = &mut self.state {
            state.accesskit.process_event(&state.window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.app.on_close_requested() {
                    event_loop.exit();
                } else if let Some(state) = &self.state {
                    // The app may show a "save changes?" dialog now.
                    state.window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.renderer.resize(to_physical(size));
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            // Frames are skipped while the window is hidden; draw again as
            // soon as it becomes visible.
            WindowEvent::Occluded(false) => {
                if let Some(state) = &self.state {
                    state.window.request_redraw();
                }
            }
            event => {
                if let Some(state) = &self.state
                    && self
                        .input
                        .on_window_event(&event, state.window.scale_factor())
                {
                    state.window.request_redraw();
                }
            }
        }
    }
}

fn to_physical(size: winit::dpi::PhysicalSize<u32>) -> PhysicalSize {
    PhysicalSize::new(size.width, size.height)
}
