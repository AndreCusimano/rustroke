//! Conversion of winit events into platform-independent [`Event`]s.

use rustroke_core::{
    Event, ImeEvent, Key, Modifiers, POINTS_PER_SCROLL_LINE, Point, PointerButton, Vec2,
};
use rustroke_widgets::CursorIcon;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};

/// Accumulates events between frames.
#[derive(Default)]
pub(crate) struct InputCollector {
    events: Vec<Event>,
    modifiers: Modifiers,
    /// Last pointer position, needed for button events (winit doesn't
    /// include one).
    pointer: Option<Point>,
    /// Created on first use; `None` inside if the platform has none.
    clipboard: Option<Option<arboard::Clipboard>>,
}

impl std::fmt::Debug for InputCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputCollector")
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl InputCollector {
    /// Takes the events collected since the previous frame.
    pub(crate) fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Converts `event`. Returns `true` if it was input that should trigger
    /// a redraw.
    pub(crate) fn on_window_event(&mut self, event: &WindowEvent, scale_factor: f64) -> bool {
        // Points are physical pixels divided by the scale factor.
        let to_points =
            |x: f64, y: f64| Point::new((x / scale_factor) as f32, (y / scale_factor) as f32);
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let pos = to_points(position.x, position.y);
                self.pointer = Some(pos);
                self.events.push(Event::PointerMoved(pos));
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                self.events.push(Event::PointerGone);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let (Some(button), Some(pos)) = (pointer_button(*button), self.pointer) else {
                    return false;
                };
                self.events.push(Event::PointerButton {
                    pos,
                    button,
                    pressed: *state == ElementState::Pressed,
                    modifiers: self.modifiers,
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Vec2::new(*x, *y) * POINTS_PER_SCROLL_LINE,
                    MouseScrollDelta::PixelDelta(p) => {
                        let p = to_points(p.x, p.y);
                        Vec2::new(p.x, p.y)
                    }
                };
                self.events.push(Event::Scroll(delta));
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = modifiers(m.state()),
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event),
            WindowEvent::Focused(focused) => self.events.push(Event::WindowFocused(*focused)),
            WindowEvent::Ime(ime) => self.events.push(Event::Ime(match ime {
                Ime::Enabled => ImeEvent::Enabled,
                Ime::Preedit(text, _) => ImeEvent::Preedit(text.clone()),
                Ime::Commit(text) => ImeEvent::Commit(text.clone()),
                Ime::Disabled => ImeEvent::Disabled,
            })),
            _ => return false,
        }
        true
    }

    fn on_key(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        let key = key(&event.logical_key);
        // Clipboard shortcuts become dedicated events.
        if pressed && self.modifiers.command_only() {
            match key {
                Some(Key::C) => self.events.push(Event::Copy),
                Some(Key::X) => self.events.push(Event::Cut),
                Some(Key::V) => {
                    if let Some(text) = self.clipboard().and_then(|c| c.get_text().ok()) {
                        self.events.push(Event::Paste(text));
                    }
                }
                _ => {}
            }
        }
        if let Some(key) = key {
            self.events.push(Event::Key {
                key,
                pressed,
                repeat: event.repeat,
                modifiers: self.modifiers,
            });
        }
        // Control characters (Enter, Tab, Backspace...) are keys, not text.
        if pressed
            && !self.modifiers.ctrl
            && !self.modifiers.logo
            && let Some(text) = &event.text
            && text.chars().all(|c| !c.is_control())
        {
            self.events.push(Event::Text(text.to_string()));
        }
    }
}

impl InputCollector {
    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        self.clipboard
            .get_or_insert_with(|| {
                arboard::Clipboard::new()
                    .map_err(|e| log::warn!("clipboard unavailable: {e}"))
                    .ok()
            })
            .as_mut()
    }

    /// Puts `text` on the system clipboard.
    pub(crate) fn set_clipboard_text(&mut self, text: String) {
        if let Some(clipboard) = self.clipboard()
            && let Err(e) = clipboard.set_text(text)
        {
            log::warn!("failed to copy to the clipboard: {e}");
        }
    }
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}

fn modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        shift: state.shift_key(),
        ctrl: state.control_key(),
        alt: state.alt_key(),
        logo: state.super_key(),
    }
}

fn key(key: &WinitKey) -> Option<Key> {
    let named = match key {
        WinitKey::Named(named) => named,
        WinitKey::Character(c) => {
            return match c.to_lowercase().as_str() {
                "a" => Some(Key::A),
                "c" => Some(Key::C),
                "v" => Some(Key::V),
                "x" => Some(Key::X),
                "y" => Some(Key::Y),
                "z" => Some(Key::Z),
                _ => None,
            };
        }
        _ => return None,
    };
    Some(match named {
        NamedKey::Tab => Key::Tab,
        NamedKey::Enter => Key::Enter,
        NamedKey::Space => Key::Space,
        NamedKey::Escape => Key::Escape,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::ArrowLeft => Key::ArrowLeft,
        NamedKey::ArrowRight => Key::ArrowRight,
        NamedKey::ArrowUp => Key::ArrowUp,
        NamedKey::ArrowDown => Key::ArrowDown,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        _ => return None,
    })
}

pub(crate) fn cursor_icon(icon: CursorIcon) -> winit::window::CursorIcon {
    match icon {
        CursorIcon::Default => winit::window::CursorIcon::Default,
        CursorIcon::PointingHand => winit::window::CursorIcon::Pointer,
        CursorIcon::Grab => winit::window::CursorIcon::Grab,
        CursorIcon::Grabbing => winit::window::CursorIcon::Grabbing,
        CursorIcon::Text => winit::window::CursorIcon::Text,
        CursorIcon::ResizeHorizontal => winit::window::CursorIcon::EwResize,
        CursorIcon::ResizeNwSe => winit::window::CursorIcon::NwseResize,
    }
}
