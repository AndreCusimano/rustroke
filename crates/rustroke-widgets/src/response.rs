use rustroke_core::{Rect, Vec2};

use crate::Id;

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
    pub(crate) lost_focus: bool,
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
            lost_focus: false,
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

    /// The widget gave up focus this frame (e.g. Enter in a single-line
    /// text field: "submit").
    pub fn lost_focus(&self) -> bool {
        self.lost_focus
    }

    /// Has focus that was reached with the keyboard, so a focus ring should
    /// be drawn (clicking gives focus without showing the ring).
    pub fn focus_visible(&self) -> bool {
        self.has_focus && self.focus_visible
    }
}
