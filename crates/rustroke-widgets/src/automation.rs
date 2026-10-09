//! Driving a running app from another thread (end-to-end tests, scripts,
//! remote control or an AI agent): read the widgets of the last frame and
//! send clicks, text and keys.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use rustroke_core::{Event, Key, Modifiers, PointerButton};

use crate::WidgetDescription;

/// Waking the event loop when commands arrive.
type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
pub(crate) struct AutomationShared {
    widgets: Mutex<Vec<WidgetDescription>>,
    pending: Mutex<Vec<Event>>,
    wake: Mutex<Option<Wake>>,
    frames: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl std::fmt::Debug for AutomationShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutomationShared").finish_non_exhaustive()
    }
}

impl AutomationShared {
    pub(crate) fn take_events(&self) -> Vec<Event> {
        std::mem::take(&mut *lock(&self.pending))
    }

    pub(crate) fn end_frame(&self, widgets: &[WidgetDescription], wake: Option<Wake>) {
        widgets.clone_into(&mut lock(&self.widgets));
        *lock(&self.wake) = wake;
        self.frames.fetch_add(1, Ordering::SeqCst);
    }
}

/// A handle to drive the app from any thread (get it with
/// [`crate::Context::automation`]). Commands are applied at the start of
/// the next frame, which they wake up. Widgets are found by the label
/// they report to screen readers, as in `testing::Harness`.
///
/// ```ignore
/// let robot = frame.ctx().automation();
/// std::thread::spawn(move || {
///     robot.wait_frames(1);
///     robot.click("Save");
/// });
/// ```
#[derive(Clone)]
pub struct Automation {
    pub(crate) shared: Arc<AutomationShared>,
}

impl std::fmt::Debug for Automation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Automation")
            .field("frames", &self.frame_count())
            .finish_non_exhaustive()
    }
}

impl Automation {
    /// The widgets of the last frame: role, label, state and rectangle.
    pub fn widgets(&self) -> Vec<WidgetDescription> {
        lock(&self.shared.widgets).clone()
    }

    /// The first widget of the last frame labelled `label`.
    pub fn find(&self, label: &str) -> Option<WidgetDescription> {
        lock(&self.shared.widgets)
            .iter()
            .find(|w| w.info.label == label)
            .cloned()
    }

    /// Clicks the center of the widget labelled `label` (as found in the
    /// last frame). Returns `false` if there is no such widget.
    pub fn click(&self, label: &str) -> bool {
        let Some(w) = self.find(label) else {
            return false;
        };
        let pos = w.rect.center();
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.send(vec![Event::PointerMoved(pos), button(true), button(false)]);
        true
    }

    /// Types `text` into the focused widget.
    pub fn type_text(&self, text: &str) {
        self.send(vec![Event::Text(text.to_owned())]);
    }

    /// Presses and releases `key` with `modifiers`.
    pub fn press_key(&self, key: Key, modifiers: Modifiers) {
        let event = |pressed| Event::Key {
            key,
            pressed,
            repeat: false,
            modifiers,
        };
        self.send(vec![event(true), event(false)]);
    }

    /// Queues any input events for the next frame.
    pub fn send(&self, events: Vec<Event>) {
        lock(&self.shared.pending).extend(events);
        if let Some(wake) = lock(&self.shared.wake).clone() {
            wake();
        }
    }

    /// Frames completed since the handle was created (to wait until the
    /// app has reacted to a command).
    pub fn frame_count(&self) -> u64 {
        self.shared.frames.load(Ordering::SeqCst)
    }

    /// Blocks the calling thread (never the UI thread) until `frames` more
    /// frames have completed, or two seconds have passed. Returns whether
    /// they did.
    pub fn wait_frames(&self, frames: u64) -> bool {
        let target = self.frame_count() + frames;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while self.frame_count() < target {
            if std::time::Instant::now() > deadline {
                return false;
            }
            if let Some(wake) = lock(&self.shared.wake).clone() {
                wake();
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        true
    }
}
