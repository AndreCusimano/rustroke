//! Conversion of winit events into platform-independent [`Event`]s.

use rustroke_core::{
    Event, ImeEvent, Key, Modifiers, POINTS_PER_SCROLL_LINE, Point, PointerButton, Vec2,
};
use rustroke_widgets::CursorIcon;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{
    Key as WinitKey, KeyCode, KeyLocation, ModifiersState, NamedKey, PhysicalKey,
};

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
    /// Last known pointer position, in points.
    pub(crate) fn pointer(&self) -> Option<Point> {
        self.pointer
    }

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
        let key = if event.location == KeyLocation::Numpad {
            numpad_key(event.physical_key)
        } else {
            // The character the key types, so shortcuts follow the
            // keyboard layout (Cmd+Z is "Z" on AZERTY too). With Alt or
            // Shift the character may change (Option+E types a dead key on
            // macOS): fall back to the key's position.
            key(&event.logical_key).or_else(|| physical_key(event.physical_key))
        };
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
                "b" => Some(Key::B),
                "c" => Some(Key::C),
                "d" => Some(Key::D),
                "e" => Some(Key::E),
                "f" => Some(Key::F),
                "g" => Some(Key::G),
                "h" => Some(Key::H),
                "i" => Some(Key::I),
                "j" => Some(Key::J),
                "k" => Some(Key::K),
                "l" => Some(Key::L),
                "m" => Some(Key::M),
                "n" => Some(Key::N),
                "o" => Some(Key::O),
                "p" => Some(Key::P),
                "q" => Some(Key::Q),
                "r" => Some(Key::R),
                "s" => Some(Key::S),
                "t" => Some(Key::T),
                "u" => Some(Key::U),
                "v" => Some(Key::V),
                "w" => Some(Key::W),
                "x" => Some(Key::X),
                "y" => Some(Key::Y),
                "z" => Some(Key::Z),
                "0" => Some(Key::Num0),
                "1" => Some(Key::Num1),
                "2" => Some(Key::Num2),
                "3" => Some(Key::Num3),
                "4" => Some(Key::Num4),
                "5" => Some(Key::Num5),
                "6" => Some(Key::Num6),
                "7" => Some(Key::Num7),
                "8" => Some(Key::Num8),
                "9" => Some(Key::Num9),
                "-" => Some(Key::Minus),
                "=" | "+" => Some(Key::Equals),
                "," => Some(Key::Comma),
                "." => Some(Key::Period),
                "/" => Some(Key::Slash),
                "\\" => Some(Key::Backslash),
                ";" => Some(Key::Semicolon),
                "'" => Some(Key::Quote),
                "`" => Some(Key::Backquote),
                "[" => Some(Key::OpenBracket),
                "]" => Some(Key::CloseBracket),
                _ => None,
            };
        }
        _ => return None,
    };
    Some(match named {
        NamedKey::Tab => Key::Tab,
        NamedKey::F1 => Key::F1,
        NamedKey::F2 => Key::F2,
        NamedKey::F3 => Key::F3,
        NamedKey::F4 => Key::F4,
        NamedKey::F5 => Key::F5,
        NamedKey::F6 => Key::F6,
        NamedKey::F7 => Key::F7,
        NamedKey::F8 => Key::F8,
        NamedKey::F9 => Key::F9,
        NamedKey::F10 => Key::F10,
        NamedKey::F11 => Key::F11,
        NamedKey::F12 => Key::F12,
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
        NamedKey::Insert => Key::Insert,
        _ => return None,
    })
}

/// A key by its position on a US keyboard, when its character is unknown.
fn physical_key(key: PhysicalKey) -> Option<Key> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    Some(match code {
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::Digit0 => Key::Num0,
        KeyCode::Digit1 => Key::Num1,
        KeyCode::Digit2 => Key::Num2,
        KeyCode::Digit3 => Key::Num3,
        KeyCode::Digit4 => Key::Num4,
        KeyCode::Digit5 => Key::Num5,
        KeyCode::Digit6 => Key::Num6,
        KeyCode::Digit7 => Key::Num7,
        KeyCode::Digit8 => Key::Num8,
        KeyCode::Digit9 => Key::Num9,
        KeyCode::Minus => Key::Minus,
        KeyCode::Equal => Key::Equals,
        KeyCode::Comma => Key::Comma,
        KeyCode::Period => Key::Period,
        KeyCode::Slash => Key::Slash,
        KeyCode::Backslash => Key::Backslash,
        KeyCode::Semicolon => Key::Semicolon,
        KeyCode::Quote => Key::Quote,
        KeyCode::Backquote => Key::Backquote,
        KeyCode::BracketLeft => Key::OpenBracket,
        KeyCode::BracketRight => Key::CloseBracket,
        _ => return None,
    })
}

/// Keys of the numeric keypad.
fn numpad_key(key: PhysicalKey) -> Option<Key> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    Some(match code {
        KeyCode::Numpad0 => Key::Numpad0,
        KeyCode::Numpad1 => Key::Numpad1,
        KeyCode::Numpad2 => Key::Numpad2,
        KeyCode::Numpad3 => Key::Numpad3,
        KeyCode::Numpad4 => Key::Numpad4,
        KeyCode::Numpad5 => Key::Numpad5,
        KeyCode::Numpad6 => Key::Numpad6,
        KeyCode::Numpad7 => Key::Numpad7,
        KeyCode::Numpad8 => Key::Numpad8,
        KeyCode::Numpad9 => Key::Numpad9,
        KeyCode::NumpadAdd => Key::NumpadAdd,
        KeyCode::NumpadSubtract => Key::NumpadSubtract,
        KeyCode::NumpadMultiply => Key::NumpadMultiply,
        KeyCode::NumpadDivide => Key::NumpadDivide,
        KeyCode::NumpadDecimal | KeyCode::NumpadComma => Key::NumpadDecimal,
        // Activates buttons and submits fields like the main Enter key.
        KeyCode::NumpadEnter => Key::Enter,
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
        CursorIcon::ResizeVertical => winit::window::CursorIcon::NsResize,
        CursorIcon::Crosshair => winit::window::CursorIcon::Crosshair,
        CursorIcon::Move => winit::window::CursorIcon::Move,
        CursorIcon::NotAllowed => winit::window::CursorIcon::NotAllowed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// INP-06
    #[test]
    fn keys_map_by_character_then_by_position() {
        let ch = |c: &str| key(&WinitKey::Character(c.into()));
        assert_eq!(ch("s"), Some(Key::S));
        assert_eq!(ch("S"), Some(Key::S), "Shift doesn't change the key");
        assert_eq!(ch(","), Some(Key::Comma));
        assert_eq!(ch("\\"), Some(Key::Backslash));
        assert_eq!(ch("´"), None, "Option+E on macOS: a dead key");
        assert_eq!(physical_key(PhysicalKey::Code(KeyCode::KeyE)), Some(Key::E));
        assert_eq!(
            physical_key(PhysicalKey::Code(KeyCode::Digit3)),
            Some(Key::Num3)
        );
        assert_eq!(
            numpad_key(PhysicalKey::Code(KeyCode::Numpad7)),
            Some(Key::Numpad7)
        );
        assert_eq!(
            numpad_key(PhysicalKey::Code(KeyCode::NumpadEnter)),
            Some(Key::Enter)
        );
        assert_eq!(key(&WinitKey::Named(NamedKey::Insert)), Some(Key::Insert));
    }
}
