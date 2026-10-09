use rustroke_core::{Point, PointerButton, Rect, Vec2};

use crate::Id;

/// Why a widget (usually a text field) gave up keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusLost {
    /// Enter in a single-line field: accept the value.
    Submit,
    /// Escape: the edit was cancelled (text fields restore the text they
    /// had when they got focus).
    Cancel,
    /// Anything else: Tab, a click elsewhere, focus moved by the app.
    Other,
}

/// The result of adding a widget: where it is and how the user
/// interacted with it this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    /// Id of the widget.
    pub id: Id,
    /// The area the widget occupies, in logical points.
    pub rect: Rect,
    pub(crate) hovered: bool,
    pub(crate) pressed: bool,
    pub(crate) clicked: bool,
    pub(crate) drag_started: bool,
    pub(crate) dragged: bool,
    pub(crate) drag_stopped: bool,
    pub(crate) drag_delta: Vec2,
    pub(crate) changed: bool,
    pub(crate) has_focus: bool,
    pub(crate) focus_visible: bool,
    pub(crate) lost_focus: Option<FocusLost>,
    pub(crate) hover_pos: Option<Point>,
    pub(crate) clicked_by: Option<PointerButton>,
    pub(crate) drag_button: Option<PointerButton>,
    pub(crate) interact_pos: Option<Point>,
    /// On the frame the primary button was pressed on the widget: how many
    /// presses in a row (2 = double click).
    pub(crate) press_count: u32,
}

impl Response {
    pub(crate) fn new(id: Id, rect: Rect) -> Self {
        Self {
            id,
            rect,
            hovered: false,
            pressed: false,
            clicked: false,
            drag_started: false,
            dragged: false,
            drag_stopped: false,
            drag_delta: Vec2::ZERO,
            changed: false,
            has_focus: false,
            focus_visible: false,
            lost_focus: None,
            hover_pos: None,
            clicked_by: None,
            drag_button: None,
            interact_pos: None,
            press_count: 0,
        }
    }

    /// The pointer is over the widget (and nothing else is being dragged).
    pub fn hovered(&self) -> bool {
        self.hovered
    }

    /// The primary button was pressed on the widget and is still down.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Clicked with the pointer, or activated with Enter/Space while focused.
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    /// The primary button was pressed on the widget for the second time in
    /// a row (reported on that press, not on the release).
    pub fn double_clicked(&self) -> bool {
        self.press_count == 2
    }

    /// The primary button was pressed on the widget for the third time in
    /// a row (reported on that press).
    pub fn triple_clicked(&self) -> bool {
        self.press_count == 3
    }

    /// Clicked with the secondary (usually right) mouse button, e.g. to
    /// open a context menu (see [`Response::context_menu`]).
    pub fn secondary_clicked(&self) -> bool {
        self.clicked_by == Some(PointerButton::Secondary)
    }

    /// Clicked with the middle mouse button.
    pub fn middle_clicked(&self) -> bool {
        self.clicked_by == Some(PointerButton::Middle)
    }

    /// The button that clicked the widget this frame, if any. (Keyboard
    /// activation reports [`Response::clicked`] without a button.)
    pub fn clicked_by(&self) -> Option<PointerButton> {
        self.clicked_by
    }

    /// Being dragged with `button` this frame (any button, not only the
    /// primary one: e.g. middle-drag to pan a view). [`Response::dragged`]
    /// only reports primary-button drags.
    pub fn dragged_by(&self, button: PointerButton) -> bool {
        self.drag_button == Some(button)
    }

    /// A drag started this frame.
    pub fn drag_started(&self) -> bool {
        self.drag_started
    }

    /// Being dragged this frame.
    pub fn dragged(&self) -> bool {
        self.dragged
    }

    /// A drag ended this frame.
    pub fn drag_stopped(&self) -> bool {
        self.drag_stopped
    }

    /// Pointer movement during the drag this frame.
    pub fn drag_delta(&self) -> Vec2 {
        self.drag_delta
    }

    /// The widget changed the value it edits (checkbox toggled, slider moved...).
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Marks the response as [`Response::changed`] (for custom widgets).
    pub fn mark_changed(&mut self) {
        self.changed = true;
    }

    /// Has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    /// Where the pointer is, in window coordinates (points), while it
    /// hovers the widget. Subtract `rect.min` for a position inside it.
    pub fn hover_pos(&self) -> Option<Point> {
        self.hover_pos
    }

    /// Where the pointer is while the widget is pressed or dragged (also
    /// when it has left the widget), or where it was clicked.
    pub fn interact_pointer_pos(&self) -> Option<Point> {
        self.interact_pos
    }

    /// The widget gave up focus this frame: Enter in a single-line text
    /// field, Escape, Tab or a click elsewhere. See
    /// [`Response::lost_focus_reason`] to tell them apart.
    pub fn lost_focus(&self) -> bool {
        self.lost_focus.is_some()
    }

    /// Why the widget gave up focus this frame, if it did. Text fields
    /// report [`FocusLost::Submit`] for Enter and [`FocusLost::Cancel`]
    /// for Escape (after restoring their text).
    pub fn lost_focus_reason(&self) -> Option<FocusLost> {
        self.lost_focus
    }

    /// Enter was pressed in a single-line text field ("submit").
    pub fn submitted(&self) -> bool {
        self.lost_focus == Some(FocusLost::Submit)
    }

    /// Has focus that was reached with the keyboard, so a focus ring should
    /// be drawn (clicking gives focus without showing the ring).
    pub fn focus_visible(&self) -> bool {
        self.has_focus && self.focus_visible
    }
}
