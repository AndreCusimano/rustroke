use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use rustroke_core::{
    ColorImage, DisplayList, InputState, Key, Modifiers, Point, PointerButton, RawInput, Rect,
    TextureId, TexturesDelta, Vec2,
};
use rustroke_text::Fonts;

use crate::accessibility::{self, PendingAction, WidgetDescription, WidgetInfo};
use crate::{Id, Response, Style, Ui};

/// What a widget reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sense {
    /// Reacts to clicks.
    pub click: bool,
    /// Reacts to drags.
    pub drag: bool,
    /// Can receive keyboard focus (Tab).
    pub focusable: bool,
    /// Enter/Space click the widget while it has focus.
    pub activate_with_keys: bool,
}

impl Sense {
    /// Only reports hovering; doesn't block widgets below it.
    pub const HOVER: Self = Self {
        click: false,
        drag: false,
        focusable: false,
        activate_with_keys: false,
    };
    /// Clickable and focusable (buttons, checkboxes).
    pub const CLICK: Self = Self {
        click: true,
        drag: false,
        focusable: true,
        activate_with_keys: true,
    };
    /// Clickable, draggable and focusable (sliders).
    pub const DRAG: Self = Self {
        click: true,
        drag: true,
        focusable: true,
        activate_with_keys: true,
    };
    /// Text fields: focusable and draggable (to select), but Enter and
    /// Space are text input, not clicks.
    pub const TEXT: Self = Self {
        click: true,
        drag: true,
        focusable: true,
        activate_with_keys: false,
    };
    /// Clicks and drags with the pointer, but not reachable with Tab
    /// (backgrounds, title bars, resize handles, scroll bars).
    pub const POINTER_DRAG: Self = Self {
        click: true,
        drag: true,
        focusable: false,
        activate_with_keys: false,
    };

    fn interactive(self) -> bool {
        self.click || self.drag
    }
}

/// All pointer buttons, in the order presses are looked for.
const BUTTONS: [PointerButton; 3] = [
    PointerButton::Primary,
    PointerButton::Secondary,
    PointerButton::Middle,
];

/// Paint and hit-test order of layers, back to front.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Order {
    /// Panels and the central area.
    Background,
    /// Floating windows, ordered among themselves by last use.
    Middle,
    /// Popups and menus.
    Foreground,
    /// Tooltips, above everything.
    Tooltip,
}

/// A drawing layer: everything in a layer is drawn above all layers that
/// sort before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId {
    /// Where the layer sorts.
    pub order: Order,
    /// Identifies the layer within its order.
    pub id: Id,
}

impl LayerId {
    /// A layer with the given order and id.
    pub fn new(order: Order, id: Id) -> Self {
        Self { order, id }
    }

    /// The layer of panels and the central area.
    pub fn background() -> Self {
        Self::new(Order::Background, Id::new("background"))
    }
}

/// Mouse cursor shape requested for this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorIcon {
    /// The normal arrow.
    #[default]
    Default,
    /// A hand, for links.
    PointingHand,
    /// An open hand: something can be dragged.
    Grab,
    /// A closed hand: something is being dragged.
    Grabbing,
    /// A text cursor (I-beam).
    Text,
    /// Resize horizontally.
    ResizeHorizontal,
    /// Resize diagonally (bottom-right corner).
    ResizeNwSe,
    /// Resize vertically.
    ResizeVertical,
    /// A crosshair, for precise picking and drawing.
    Crosshair,
    /// Four arrows: something can be moved (e.g. panning a view).
    Move,
    /// The action is not allowed here.
    NotAllowed,
}

/// What the platform layer should draw and do after a frame.
#[derive(Clone, Debug, Default)]
pub struct FrameOutput {
    /// Everything the Uis drew this frame, all layers merged back to front.
    pub shapes: DisplayList,
    /// Mouse cursor to show.
    pub cursor: CursorIcon,
    /// Run another frame soon, because this one's result depends on
    /// information only available after it (e.g. which widget is on top).
    pub repaint: bool,
    /// Run another frame after this many seconds (e.g. to show a tooltip
    /// after a delay), even without input.
    pub repaint_after: Option<f64>,
    /// Text to put on the system clipboard (copy/cut).
    pub copied_text: Option<String>,
    /// A text field has focus: enable the input method (IME) and show its
    /// candidate window near this rectangle (the text cursor, in points).
    pub ime_cursor: Option<Rect>,
    /// Images to upload to (or remove from) the GPU before drawing.
    pub textures: TexturesDelta,
    /// The full accessibility tree, while assistive technology is active.
    pub accesskit_update: Option<accesskit::TreeUpdate>,
}

/// A loaded image on the GPU. Cheap to clone; the texture is freed when
/// the last clone is dropped.
#[derive(Clone, Debug)]
pub struct TextureHandle {
    inner: Arc<HandleInner>,
}

#[derive(Debug)]
struct HandleInner {
    id: TextureId,
    size: Mutex<[u32; 2]>,
    /// Shared with the [`TextureManager`]: images waiting to be uploaded.
    pending: Arc<Mutex<Vec<(TextureId, ColorImage)>>>,
    freed: Arc<Mutex<Vec<TextureId>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Drop for HandleInner {
    fn drop(&mut self) {
        self.freed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.id);
    }
}

impl TextureHandle {
    /// Identifies the texture in shapes and meshes.
    pub fn id(&self) -> TextureId {
        self.inner.id
    }

    /// Size in pixels.
    pub fn size(&self) -> [u32; 2] {
        *lock(&self.inner.size)
    }

    /// Size in pixels as a vector (one point per pixel).
    pub fn size_vec2(&self) -> Vec2 {
        let [w, h] = self.size();
        Vec2::new(w as f32, h as f32)
    }

    /// Replaces the image, keeping the same id (so shapes and widgets that
    /// show this texture show the new image from the next frame). The size
    /// may change.
    pub fn set(&self, image: ColorImage) {
        *lock(&self.inner.size) = image.size;
        lock(&self.inner.pending).push((self.inner.id, image));
    }

    /// Records a new size without uploading anything (textures whose
    /// pixels come from elsewhere, e.g. a native GPU texture).
    pub fn set_size(&self, size: [u32; 2]) {
        *lock(&self.inner.size) = size;
    }
}

/// Wakes the UI up from any thread, e.g. when a background computation
/// finishes: `handle.request_repaint()` schedules a new frame. Cheap to
/// clone; get one with [`Context::repaint_handle`].
#[derive(Clone)]
pub struct RepaintHandle {
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for RepaintHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepaintHandle")
            .field("connected", &self.wake.is_some())
            .finish()
    }
}

impl RepaintHandle {
    /// Asks for a new frame as soon as possible. Does nothing when the
    /// context isn't driven by a window (e.g. in tests).
    pub fn request_repaint(&self) {
        if let Some(wake) = &self.wake {
            wake();
        }
    }
}

/// Images waiting to be uploaded, and ids of dropped handles.
#[derive(Debug, Default)]
struct TextureManager {
    next_id: u64,
    pending: Arc<Mutex<Vec<(TextureId, ColorImage)>>>,
    freed: Arc<Mutex<Vec<TextureId>>>,
}

impl TextureManager {
    fn new_handle(&mut self, size: [u32; 2]) -> TextureHandle {
        self.next_id += 1;
        TextureHandle {
            inner: Arc::new(HandleInner {
                id: TextureId::User(self.next_id),
                size: Mutex::new(size),
                pending: Arc::clone(&self.pending),
                freed: Arc::clone(&self.freed),
            }),
        }
    }
}

/// How the platform wakes the event loop up.
#[derive(Clone)]
struct RepaintCallback(Option<Arc<dyn Fn() + Send + Sync>>);

impl std::fmt::Debug for RepaintCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RepaintCallback({})", self.0.is_some())
    }
}

/// A widget placed in a frame, for hit testing in the next one.
#[derive(Clone, Copy, Debug)]
struct Placed {
    layer: LayerId,
    id: Id,
    rect: Rect,
}

/// Widgets placed during one frame, used for hit testing and focus
/// navigation in the next one.
#[derive(Debug)]
struct FrameState {
    /// Interactive widgets in the order they were added (later = on top
    /// within a layer).
    widgets: Vec<Placed>,
    focusables: Vec<Id>,
    /// Every widget id seen, interactive or not.
    seen: Vec<Id>,
    /// Shapes of each layer, in creation order.
    layers: Vec<(LayerId, DisplayList)>,
    root_count: usize,
    /// Space not yet taken by panels.
    available_rect: Rect,
    output: FrameOutput,
    /// A tooltip-capable widget was hovered this frame.
    hover_tracked: bool,
    /// Widget descriptions for screen readers (only while active).
    described: Vec<WidgetDescription>,
    /// The focused widget takes keyboard input (a text field).
    keyboard_owner: Option<Id>,
    /// Areas of the top-level Uis (panels, windows, popups).
    ui_areas: Vec<Rect>,
}

impl Default for FrameState {
    fn default() -> Self {
        Self {
            widgets: Vec::new(),
            focusables: Vec::new(),
            seen: Vec::new(),
            layers: Vec::new(),
            root_count: 0,
            available_rect: Rect::EVERYTHING,
            output: FrameOutput::default(),
            hover_tracked: false,
            described: Vec::new(),
            keyboard_owner: None,
            ui_areas: Vec::new(),
        }
    }
}

/// Per-widget state of any type, kept between frames.
#[derive(Default)]
struct DataMap(HashMap<Id, Box<dyn Any + Send + Sync>>);

impl std::fmt::Debug for DataMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DataMap({} entries)", self.0.len())
    }
}

/// State kept between frames: input, style, which widget is pressed or
/// focused, window order, per-widget data. Create one per window and pass
/// it every frame.
#[derive(Debug)]
pub struct Context {
    input: InputState,
    style: Arc<Style>,
    /// Widget a pointer button was pressed on, until it is released.
    active: Option<Id>,
    /// The button that made `active` active.
    active_button: PointerButton,
    /// The primary button was pressed on empty space and is still down:
    /// no widget should react until it is released.
    pressed_on_background: bool,
    focused: Option<Id>,
    /// Focus came from the keyboard, so it should be visible.
    focus_visible: bool,
    this_frame: FrameState,
    prev_frame: FrameState,
    /// Topmost interactive widget under the pointer, according to the
    /// previous frame's layout.
    hit: Option<Placed>,
    /// Middle-layer (window) order, back to front.
    window_order: Vec<LayerId>,
    /// The id of the widget whose popup is open, if any.
    open_popup: Option<Id>,
    /// Widget being hovered for tooltip purposes, and since when.
    hover_start: Option<(Id, f64)>,
    textures: TextureManager,
    repaint_callback: RepaintCallback,
    /// Assistive technology (a screen reader) is listening.
    accessibility_active: bool,
    /// Between `begin_frame` and `end_frame`.
    in_frame: bool,
    /// Widgets described in the last completed frame.
    last_widgets: Vec<WidgetDescription>,
    /// Requests from assistive technology, applied at the next frame.
    accesskit_requests: Vec<accesskit::ActionRequest>,
    /// Widgets to report as clicked because assistive technology asked.
    pending_clicks: Vec<Id>,
    data: DataMap,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            input: InputState::default(),
            style: Arc::new(Style::default()),
            active: None,
            active_button: PointerButton::Primary,
            pressed_on_background: false,
            focused: None,
            focus_visible: false,
            this_frame: FrameState::default(),
            prev_frame: FrameState::default(),
            hit: None,
            window_order: Vec::new(),
            open_popup: None,
            hover_start: None,
            textures: TextureManager::default(),
            repaint_callback: RepaintCallback(None),
            in_frame: false,
            accessibility_active: false,
            last_widgets: Vec::new(),
            accesskit_requests: Vec::new(),
            pending_clicks: Vec::new(),
            data: DataMap::default(),
        }
    }
}

impl Context {
    /// A new context with the default (dark) style.
    pub fn new() -> Self {
        Self::default()
    }

    /// The style used by default by every Ui.
    pub fn style(&self) -> &Arc<Style> {
        &self.style
    }

    /// Changes the style for the whole app, from this frame on.
    pub fn set_style(&mut self, style: impl Into<Arc<Style>>) {
        self.style = style.into();
    }

    /// Input of the current frame.
    pub fn input(&self) -> &InputState {
        &self.input
    }

    /// Mutable input, e.g. to consume keys or scrolling.
    pub fn input_mut(&mut self) -> &mut InputState {
        &mut self.input
    }

    /// The widget with keyboard focus, if any.
    pub fn focused(&self) -> Option<Id> {
        self.focused
    }

    /// Turns the accessibility tree on or off. The platform calls this when
    /// a screen reader starts or stops listening.
    pub fn set_accessibility_active(&mut self, active: bool) {
        self.accessibility_active = active;
    }

    /// The widgets of the last completed frame (after `end_frame`), in the
    /// order they were added, with role, label, state and rectangle. Handy
    /// to find widgets by label in tests.
    pub fn widgets(&self) -> &[WidgetDescription] {
        &self.last_widgets
    }

    /// The first widget of the last frame whose label is `label`.
    pub fn find_widget(&self, label: &str) -> Option<&WidgetDescription> {
        self.last_widgets.iter().find(|w| w.info.label == label)
    }

    /// True while a screen reader is listening.
    pub fn is_accessibility_active(&self) -> bool {
        self.accessibility_active
    }

    /// Queues a request from assistive technology (e.g. "press this
    /// button"), applied at the start of the next frame.
    pub fn accesskit_action(&mut self, request: accesskit::ActionRequest) {
        self.accesskit_requests.push(request);
    }

    /// Records what widget `id` is (for screen readers and [`Context::widgets`]).
    pub(crate) fn describe(&mut self, id: Id, info: WidgetInfo, rect: Rect, enabled: bool) {
        let focusable = self.this_frame.focusables.contains(&id);
        self.this_frame.described.push(WidgetDescription {
            id,
            info,
            rect,
            enabled,
            focusable,
        });
    }

    /// Uploads an image for drawing with [`crate::Image`] or
    /// `DisplayList::image`. Load once and keep the handle: the texture
    /// lives until the last clone of the handle is dropped.
    pub fn load_texture(&mut self, image: ColorImage) -> TextureHandle {
        let handle = self.textures.new_handle(image.size);
        handle.set(image);
        handle
    }

    /// A texture id whose pixels are provided by the platform layer rather
    /// than by an image (e.g. a wgpu texture the application renders into;
    /// see `Frame::register_native_texture`). Freed when the last clone of
    /// the handle is dropped, like any other texture.
    pub fn allocate_texture(&mut self, size: [u32; 2]) -> TextureHandle {
        self.textures.new_handle(size)
    }

    /// A handle to wake the UI up from other threads.
    pub fn repaint_handle(&self) -> RepaintHandle {
        RepaintHandle {
            wake: self.repaint_callback.0.clone(),
        }
    }

    /// Connects [`RepaintHandle`]s to the platform's event loop (called by
    /// the platform layer once at startup).
    pub fn set_repaint_callback(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.repaint_callback = RepaintCallback(Some(Arc::new(wake)));
    }

    /// State stored for `id` with [`Context::insert_data`], if it has type `T`.
    pub fn data<T: Clone + 'static>(&self, id: Id) -> Option<T> {
        self.data.0.get(&id)?.downcast_ref::<T>().cloned()
    }

    /// Stores state for `id` (e.g. a widget's scroll offset) until replaced.
    pub fn insert_data<T: Send + Sync + 'static>(&mut self, id: Id, value: T) {
        self.data.0.insert(id, Box::new(value));
    }

    /// Forgets the state stored for `id`.
    pub fn remove_data(&mut self, id: Id) {
        self.data.0.remove(&id);
    }

    /// Runs another frame right after this one.
    pub fn request_repaint(&mut self) {
        self.this_frame.output.repaint = true;
    }

    /// Asks for another frame in `seconds`, even if no input arrives.
    pub fn request_repaint_after(&mut self, seconds: f64) {
        let after = &mut self.this_frame.output.repaint_after;
        *after = Some(after.map_or(seconds, |a| a.min(seconds)));
    }

    /// Puts `text` on the system clipboard at the end of the frame.
    pub fn copy_text(&mut self, text: String) {
        self.this_frame.output.copied_text = Some(text);
    }

    /// Enables text composition (IME) for this frame, with the candidate
    /// window placed near `cursor_rect`.
    pub fn set_ime_cursor(&mut self, cursor_rect: Rect) {
        self.this_frame.output.ime_cursor = Some(cursor_rect);
    }

    /// Removes keyboard focus from whatever widget has it.
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Gives keyboard focus to widget `id` (e.g. a text field when a
    /// dialog opens). Takes effect this frame for widgets added after the
    /// call, otherwise in the next one.
    pub fn request_focus(&mut self, id: Id) {
        self.focused = Some(id);
        self.focus_visible = false;
    }

    /// A text field has keyboard focus: the app should not treat key
    /// presses as shortcuts (e.g. a bare "Delete" or letter keys).
    pub fn wants_keyboard_input(&self) -> bool {
        self.focused.is_some_and(|id| {
            self.this_frame.keyboard_owner == Some(id) || self.prev_frame.keyboard_owner == Some(id)
        })
    }

    /// Whether `pos` (window coordinates, points) is over the UI of the
    /// last frame: a panel, window, popup or widget. When rustroke is
    /// embedded in an app that draws its own content (a 3D view), events
    /// there belong to the UI, not to the app.
    pub fn is_pointer_over_ui(&self, pos: Point) -> bool {
        let frame = if self.in_frame {
            &self.prev_frame
        } else {
            &self.this_frame
        };
        frame.ui_areas.iter().any(|r| r.contains(pos))
            || frame.widgets.iter().any(|w| w.rect.contains(pos))
    }

    /// A widget is being pressed or dragged (e.g. a slider), so pointer
    /// movement belongs to the UI even outside it.
    pub fn wants_pointer_input(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn add_ui_area(&mut self, rect: Rect) {
        self.this_frame.ui_areas.push(rect);
    }

    /// Marks the focused widget `id` as taking keyboard input, so Escape
    /// is left to it and [`Context::wants_keyboard_input`] is true.
    pub(crate) fn set_keyboard_owner(&mut self, id: Id) {
        self.this_frame.keyboard_owner = Some(id);
    }

    /// Mouse cursor to show for this frame (the last call wins).
    pub fn set_cursor(&mut self, cursor: CursorIcon) {
        self.this_frame.output.cursor = cursor;
    }

    /// The area not yet taken by panels this frame.
    pub fn available_rect(&self) -> Rect {
        self.this_frame.available_rect
    }

    /// Takes `rect` away from the available area (used by panels).
    pub(crate) fn set_available_rect(&mut self, rect: Rect) {
        self.this_frame.available_rect = rect;
    }

    // ---- Frame ----

    /// Starts a frame: applies input, brings a clicked window to the front,
    /// closes popups clicked outside of, and handles keyboard focus
    /// navigation (Tab / Shift+Tab / Escape) using the previous frame.
    pub fn begin_frame(&mut self, raw: RawInput) {
        self.in_frame = true;
        self.input.begin_frame(raw);
        self.prev_frame = std::mem::take(&mut self.this_frame);
        self.this_frame.available_rect = self.input.screen_rect;
        self.hit = self.topmost(&self.prev_frame.widgets, self.input.pointer.interact_pos());

        if self.input.pointer.primary_pressed() {
            match self.hit {
                Some(hit) => {
                    if hit.layer.order == Order::Middle {
                        self.move_to_top(hit.layer);
                    }
                }
                None => {
                    // Clicking on empty space removes focus.
                    self.focused = None;
                    self.pressed_on_background = true;
                }
            }
        }
        // Any button pressed outside the open popup (and its opener) closes it.
        let any_pressed = BUTTONS
            .into_iter()
            .any(|b| self.input.pointer.pressed_at(b).is_some());
        if any_pressed && let Some(popup) = self.open_popup {
            let in_popup = self
                .hit
                .is_some_and(|h| h.layer == popup_layer(popup) || h.id == popup);
            if !in_popup {
                self.open_popup = None;
            }
        }

        for request in std::mem::take(&mut self.accesskit_requests) {
            match accessibility::pending_action(&request, &self.last_widgets) {
                Some(PendingAction::Focus(id)) => {
                    self.focused = Some(id);
                    self.focus_visible = true;
                }
                Some(PendingAction::Click(id)) => self.pending_clicks.push(id),
                None => {}
            }
        }

        if self.input.consume_key(Key::Tab, Modifiers::NONE) {
            self.move_focus(true);
        }
        if self.input.consume_key(Key::Tab, Modifiers::SHIFT) {
            self.move_focus(false);
        }
        // Escape closes the open popup; otherwise it takes focus away from
        // the focused widget, except text fields, which handle it
        // themselves (cancel). When there is nothing to close, the app gets
        // it (e.g. to leave a tool).
        let text_focused = self.focused.is_some() && self.focused == self.prev_frame.keyboard_owner;
        let escape_closes = self.open_popup.is_some() || (self.focused.is_some() && !text_focused);
        if escape_closes && self.input.consume_key(Key::Escape, Modifiers::NONE) {
            if self.open_popup.is_some() {
                self.open_popup = None;
            } else {
                self.focused = None;
            }
        }
    }

    /// Ends a frame: returns the shapes of all layers merged in z-order,
    /// and tells the platform layer what to do next.
    pub fn end_frame(&mut self) -> FrameOutput {
        self.in_frame = false;
        let seen = &self.this_frame.seen;
        let active_button_down = self.input.pointer.is_down(self.active_button);
        if !active_button_down || self.active.is_some_and(|id| !seen.contains(&id)) {
            self.active = None;
        }
        if !self.input.pointer.primary_down() {
            self.pressed_on_background = false;
        }
        if self.focused.is_some_and(|id| !seen.contains(&id)) {
            self.focused = None;
        }
        if self.open_popup.is_some_and(|id| !seen.contains(&id)) {
            self.open_popup = None;
        }
        if !self.this_frame.hover_tracked {
            self.hover_start = None;
        }
        // Forget windows that were not shown.
        let layers = &self.this_frame.layers;
        self.window_order
            .retain(|w| layers.iter().any(|(l, _)| l == w));

        // If the widget under the pointer differs from the one hover was
        // computed with, run again so hover highlighting is correct.
        let pos = self.input.pointer.pos();
        let hit_now = self.topmost(&self.this_frame.widgets, pos).map(|p| p.id);
        let hit_before = self.topmost(&self.prev_frame.widgets, pos).map(|p| p.id);
        if hit_now != hit_before {
            self.this_frame.output.repaint = true;
        }

        let mut layers = std::mem::take(&mut self.this_frame.layers);
        layers.sort_by_key(|(layer, _)| self.layer_key(*layer));
        if self.accessibility_active {
            self.this_frame.output.accesskit_update = Some(accessibility::build_tree(
                &self.this_frame.described,
                self.focused,
                self.input.screen_rect,
                self.input.pixels_per_point,
            ));
        }
        self.pending_clicks.clear();
        self.last_widgets = std::mem::take(&mut self.this_frame.described);
        let mut output = std::mem::take(&mut self.this_frame.output);
        output.textures = TexturesDelta {
            set: std::mem::take(&mut *lock(&self.textures.pending)),
            free: std::mem::take(&mut *lock(&self.textures.freed)),
        };
        for (_, mut list) in layers {
            output.shapes.append(&mut list);
        }
        output
    }

    /// Creates a root [`Ui`] laying out widgets inside `rect` (in points),
    /// on the background layer.
    pub fn ui<R>(
        &mut self,
        rect: Rect,
        fonts: &mut Fonts,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> R {
        let id = Id::new("root").with(self.this_frame.root_count);
        self.this_frame.root_count += 1;
        self.ui_in_layer(LayerId::background(), id, rect, fonts, add_contents)
    }

    /// Creates a root [`Ui`] drawing into `layer`.
    pub fn ui_in_layer<R>(
        &mut self,
        layer: LayerId,
        id: Id,
        rect: Rect,
        fonts: &mut Fonts,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> R {
        if layer.order == Order::Middle && !self.window_order.contains(&layer) {
            self.window_order.push(layer); // new windows open on top
        }
        let mut ui = Ui::new(self, fonts, layer, id, rect);
        add_contents(&mut ui)
    }

    /// The shapes of `layer` for this frame.
    pub(crate) fn layer_shapes(&mut self, layer: LayerId) -> &mut DisplayList {
        let layers = &mut self.this_frame.layers;
        let index = match layers.iter().position(|(l, _)| *l == layer) {
            Some(i) => i,
            None => {
                layers.push((layer, DisplayList::new()));
                layers.len() - 1
            }
        };
        &mut layers[index].1
    }

    // ---- Interaction ----

    /// Registers a widget for this frame and computes its interaction.
    /// `clip` is the visible area; the widget can't be hit outside it.
    pub fn interact(
        &mut self,
        layer: LayerId,
        id: Id,
        rect: Rect,
        clip: Rect,
        sense: Sense,
    ) -> Response {
        let visible = rect.intersect(clip);
        self.this_frame.seen.push(id);
        if sense.interactive() {
            self.this_frame.widgets.push(Placed {
                layer,
                id,
                rect: visible,
            });
        }
        if sense.focusable {
            self.this_frame.focusables.push(id);
        }

        let mut response = Response::new(id, rect);
        let pointer = &self.input.pointer;
        let over = pointer.interact_pos().is_some_and(|p| visible.contains(p));
        // An interactive widget needs to be the topmost one; a hover-only
        // widget (label, area) just needs nothing in a higher layer.
        let covered = match self.hit {
            Some(hit) if sense.interactive() => hit.id != id,
            Some(hit) => self.layer_key(hit.layer) > self.layer_key(layer),
            None => false,
        };
        let other_active =
            self.active.is_some_and(|active| active != id) || self.pressed_on_background;
        response.hovered = over && !covered && !other_active;

        if sense.interactive() {
            // A press of any button starts an interaction with the widget
            // under the pointer; it lasts until that button is released.
            let pressed = BUTTONS
                .into_iter()
                .find(|b| pointer.pressed_at(*b).is_some());
            if let (true, Some(button)) = (response.hovered, pressed) {
                self.active = Some(id);
                self.active_button = button;
                let primary = button == PointerButton::Primary;
                response.drag_started = sense.drag && primary;
                if primary && sense.focusable {
                    self.focused = Some(id);
                    self.focus_visible = false;
                }
            }
            if self.active == Some(id) {
                let button = self.active_button;
                let primary = button == PointerButton::Primary;
                if let Some(pos) = pointer.released_at(button) {
                    if sense.click && visible.contains(pos) {
                        response.clicked_by = Some(button);
                        response.clicked = primary;
                    }
                    response.drag_stopped = sense.drag && primary;
                } else if pointer.is_down(button) {
                    response.pressed = primary;
                    if sense.drag {
                        response.dragged = primary;
                        response.drag_button = Some(button);
                        // On the press frame only the movement after the press
                        // counts; earlier movement is not part of the drag.
                        response.drag_delta = match (pointer.pressed_at(button), pointer.pos()) {
                            (Some(start), Some(now)) => now - start,
                            _ => pointer.delta(),
                        };
                    }
                }
            }
        }

        if sense.click
            && let Some(i) = self.pending_clicks.iter().position(|c| *c == id)
        {
            self.pending_clicks.remove(i);
            response.clicked = true;
        }

        let pointer = &self.input.pointer;
        if response.hovered {
            response.hover_pos = pointer.pos();
        }
        if response.pressed || response.clicked || response.drag_started || response.drag_stopped {
            response.interact_pos = pointer.pos().or(pointer.interact_pos());
        }

        response.has_focus = self.focused == Some(id);
        response.focus_visible = self.focus_visible;
        if response.has_focus
            && sense.click
            && sense.activate_with_keys
            && (self.input.consume_key(Key::Enter, Modifiers::NONE)
                || self.input.consume_key(Key::Space, Modifiers::NONE))
        {
            response.clicked = true;
        }
        response
    }

    /// Seconds `id` has been continuously hovered (0 when not hovered).
    /// Used for tooltips.
    pub(crate) fn hover_duration(&mut self, id: Id, hovered: bool) -> f64 {
        if !hovered {
            return 0.0;
        }
        self.this_frame.hover_tracked = true;
        let now = self.input.time;
        match self.hover_start {
            Some((hovered_id, start)) if hovered_id == id => now - start,
            _ => {
                self.hover_start = Some((id, now));
                0.0
            }
        }
    }

    // ---- Animation ----

    /// A value that moves smoothly from 0 to 1 while `on` is true, and
    /// back to 0 when it becomes false, over the style's animation time.
    /// Requests frames while it is moving.
    pub fn animate_bool(&mut self, id: Id, on: bool) -> f32 {
        let duration = self.style.animation_time;
        self.animate_bool_with_time(id, on, duration)
    }

    /// Like [`Context::animate_bool`] with an explicit duration in seconds.
    pub fn animate_bool_with_time(&mut self, id: Id, on: bool, duration: f32) -> f32 {
        let target = if on { 1.0 } else { 0.0 };
        if duration <= 0.0 {
            return target;
        }
        let now = self.input.time;
        let Some(mut anim) = self.data::<Animation>(id) else {
            // First time: start at rest in the current state.
            let anim = Animation {
                from: target,
                to: target,
                start: now,
            };
            self.insert_data(id, anim);
            return target;
        };
        let value = |a: &Animation| {
            let t = ((now - a.start) as f32 / duration).clamp(0.0, 1.0);
            // Ease out: fast start, gentle stop.
            let eased = 1.0 - (1.0 - t) * (1.0 - t);
            a.from + (a.to - a.from) * eased
        };
        if anim.to != target {
            anim = Animation {
                from: value(&anim),
                to: target,
                start: now,
            };
            self.insert_data(id, anim);
        }
        let current = value(&anim);
        if current != target {
            self.request_repaint();
        }
        current
    }

    // ---- Popups ----

    /// A popup (menu, combo box list, context menu) is open: keys like
    /// Escape, arrows and Enter belong to it, not to app shortcuts.
    pub fn any_popup_open(&self) -> bool {
        self.open_popup.is_some()
    }

    /// The widget whose popup is open, if any.
    pub fn open_popup_id(&self) -> Option<Id> {
        self.open_popup
    }

    /// True if the popup of widget `id` is open.
    pub fn is_popup_open(&self, id: Id) -> bool {
        self.open_popup == Some(id)
    }

    /// Opens the popup belonging to widget `id`, closing any other.
    pub fn open_popup(&mut self, id: Id) {
        self.open_popup = Some(id);
    }

    /// Closes the open popup, if any.
    pub fn close_popup(&mut self) {
        self.open_popup = None;
    }

    /// Marks `id` as used this frame (things that are not widgets but must
    /// not be forgotten, e.g. an open context menu).
    pub(crate) fn keep_alive(&mut self, id: Id) {
        self.this_frame.seen.push(id);
    }

    // ---- Helpers ----

    /// Sort key of a layer: paint and hit-test order.
    fn layer_key(&self, layer: LayerId) -> (Order, usize) {
        let index = match layer.order {
            Order::Middle => self
                .window_order
                .iter()
                .position(|l| *l == layer)
                .unwrap_or(usize::MAX),
            _ => 0,
        };
        (layer.order, index)
    }

    fn move_to_top(&mut self, layer: LayerId) {
        self.window_order.retain(|l| *l != layer);
        self.window_order.push(layer);
    }

    /// The widget that would receive a click at `pos`: in the highest
    /// layer, and the last added within it.
    fn topmost(&self, widgets: &[Placed], pos: Option<Point>) -> Option<Placed> {
        let pos = pos?;
        widgets
            .iter()
            .enumerate()
            .filter(|(_, w)| w.rect.contains(pos))
            .max_by_key(|(i, w)| (self.layer_key(w.layer), *i))
            .map(|(_, w)| *w)
    }

    fn move_focus(&mut self, forward: bool) {
        let list = &self.prev_frame.focusables;
        if list.is_empty() {
            return;
        }
        let current = self
            .focused
            .and_then(|id| list.iter().position(|f| *f == id));
        let next = match (current, forward) {
            (None, true) => 0,
            (None, false) => list.len() - 1,
            (Some(i), true) => (i + 1) % list.len(),
            (Some(i), false) => (i + list.len() - 1) % list.len(),
        };
        self.focused = Some(list[next]);
        self.focus_visible = true;
    }
}

/// State of one [`Context::animate_bool`] value.
#[derive(Clone, Copy, Debug)]
struct Animation {
    from: f32,
    to: f32,
    /// Time the animation towards `to` started.
    start: f64,
}

/// The layer a popup opened by widget `id` is drawn in.
pub(crate) fn popup_layer(id: Id) -> LayerId {
    LayerId::new(Order::Foreground, id.with("popup"))
}
