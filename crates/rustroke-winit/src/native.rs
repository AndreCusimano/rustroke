//! Native menus (the macOS menu bar), a tray icon and notifications.

use rustroke_core::{ColorImage, Key, KeyboardShortcut};

/// A menu of the native menu bar (see [`crate::Frame::set_native_menu`]).
#[derive(Clone, Debug, PartialEq)]
pub struct NativeMenu {
    /// Shown in the menu bar.
    pub title: String,
    /// The entries, top to bottom.
    pub items: Vec<NativeMenuItem>,
}

impl NativeMenu {
    /// A menu titled `title` with `items`.
    pub fn new(title: impl Into<String>, items: Vec<NativeMenuItem>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }
}

/// An entry of a native menu or of the tray icon's menu.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeMenuItem {
    /// A command: choosing it reports `id` in
    /// [`crate::Frame::native_menu_events`].
    Action {
        /// Reported when chosen.
        id: String,
        /// Shown text.
        label: String,
        /// Shown next to it, and handled by the system menu (the app then
        /// receives the menu event, not the key).
        shortcut: Option<KeyboardShortcut>,
        /// Whether it can be chosen.
        enabled: bool,
    },
    /// A command with a check mark.
    Check {
        /// Reported when chosen.
        id: String,
        /// Shown text.
        label: String,
        /// Whether the check mark is shown.
        checked: bool,
        /// Whether it can be chosen.
        enabled: bool,
    },
    /// A line between groups of entries.
    Separator,
    /// A nested menu.
    Submenu(NativeMenu),
}

impl NativeMenuItem {
    /// An enabled command without shortcut.
    pub fn action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Action {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            enabled: true,
        }
    }

    /// An enabled command with a keyboard shortcut.
    pub fn action_with_shortcut(
        id: impl Into<String>,
        label: impl Into<String>,
        shortcut: KeyboardShortcut,
    ) -> Self {
        Self::Action {
            id: id.into(),
            label: label.into(),
            shortcut: Some(shortcut),
            enabled: true,
        }
    }
}

/// An icon in the system tray (Windows) or among the menu bar extras
/// (macOS), with a menu (see [`crate::Frame::set_tray`]).
#[derive(Clone, Debug, PartialEq)]
pub struct TrayOptions {
    /// The icon (about 32 × 32 pixels).
    pub icon: ColorImage,
    /// Shown when hovering the icon.
    pub tooltip: String,
    /// The icon's menu; its choices arrive like native menu events.
    pub menu: Vec<NativeMenuItem>,
}

/// Shows a notification from the system (macOS, Linux with
/// `notify-send`, Windows 10+), without waiting for it. Returns an error
/// when the system command could not be started.
pub fn notify(title: &str, body: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let quote = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            quote(body),
            quote(title)
        );
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map(drop)
    }
    #[cfg(target_os = "windows")]
    {
        let quote = |s: &str| s.replace('\'', "''");
        let script = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
             $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
             $x = $t.GetElementsByTagName('text'); $x.Item(0).AppendChild($t.CreateTextNode('{}')) > $null; \
             $x.Item(1).AppendChild($t.CreateTextNode('{}')) > $null; \
             [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('rustroke').Show([Windows.UI.Notifications.ToastNotification]::new($t))",
            quote(title),
            quote(body)
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .spawn()
            .map(drop)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (title, body);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "notifications are not supported in the browser yet",
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_arch = "wasm32")))]
    {
        std::process::Command::new("notify-send")
            .args([title, body])
            .spawn()
            .map(drop)
    }
}

/// The text form of a shortcut for the system menu ("CmdOrCtrl+Shift+S").
#[cfg_attr(
    not(all(
        any(feature = "native-menu", feature = "tray"),
        any(target_os = "macos", target_os = "windows")
    )),
    allow(dead_code)
)]
pub(crate) fn accelerator(shortcut: &KeyboardShortcut) -> Option<String> {
    let m = shortcut.modifiers;
    let mut parts = Vec::new();
    if m.command() {
        parts.push("CmdOrCtrl");
    }
    if cfg!(target_os = "macos") && m.ctrl {
        parts.push("Control");
    }
    if m.alt {
        parts.push("Alt");
    }
    if m.shift {
        parts.push("Shift");
    }
    let key = match shortcut.key {
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::Enter => "Enter",
        Key::Escape => "Escape",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Tab => "Tab",
        Key::Space => "Space",
        other => other.symbol(),
    };
    if !key.is_ascii() || key.is_empty() {
        return None;
    }
    parts.push(key);
    Some(parts.join("+"))
}

/// The platform side of native menus and the tray icon.
#[cfg(all(
    any(feature = "native-menu", feature = "tray"),
    any(target_os = "macos", target_os = "windows")
))]
pub(crate) mod platform {
    use std::sync::{Arc, Mutex, PoisonError};

    use tray_icon::menu::{
        CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
    };

    use super::{NativeMenu, NativeMenuItem, TrayOptions, accelerator};

    /// Menu choices and tray clicks since the last frame, filled by the
    /// system's handlers.
    #[derive(Default)]
    pub(crate) struct Events {
        pub menu: Vec<String>,
        pub tray_clicked: bool,
    }

    /// The native menu bar and tray icon currently shown.
    #[derive(Default)]
    pub(crate) struct Native {
        menu: Option<(Vec<NativeMenu>, Menu)>,
        tray: Option<(TrayOptions, tray_icon::TrayIcon)>,
        pub events: Arc<Mutex<Events>>,
        handlers_set: bool,
    }

    fn items(list: &[NativeMenuItem]) -> Vec<Box<dyn IsMenuItem>> {
        list.iter()
            .map(|item| -> Box<dyn IsMenuItem> {
                match item {
                    NativeMenuItem::Action {
                        id,
                        label,
                        shortcut,
                        enabled,
                    } => {
                        let accel = shortcut
                            .as_ref()
                            .and_then(accelerator)
                            .and_then(|a| a.parse().ok());
                        Box::new(MenuItem::with_id(id.as_str(), label, *enabled, accel))
                    }
                    NativeMenuItem::Check {
                        id,
                        label,
                        checked,
                        enabled,
                    } => Box::new(CheckMenuItem::with_id(
                        id.as_str(),
                        label,
                        *enabled,
                        *checked,
                        None,
                    )),
                    NativeMenuItem::Separator => Box::new(PredefinedMenuItem::separator()),
                    NativeMenuItem::Submenu(menu) => Box::new(submenu(menu)),
                }
            })
            .collect()
    }

    fn submenu(menu: &NativeMenu) -> Submenu {
        let children = items(&menu.items);
        let refs: Vec<&dyn IsMenuItem> = children.iter().map(|b| b.as_ref()).collect();
        Submenu::with_items(&menu.title, true, &refs)
            .unwrap_or_else(|_| Submenu::new(&menu.title, true))
    }

    impl Native {
        /// Routes the system's menu and tray events to `events` and wakes
        /// the event loop.
        fn set_handlers(&mut self, wake: impl Fn() + Send + Sync + Clone + 'static) {
            if self.handlers_set {
                return;
            }
            self.handlers_set = true;
            let events = Arc::clone(&self.events);
            let w = wake.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                events
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .menu
                    .push(e.id.0);
                w();
            }));
            let events = Arc::clone(&self.events);
            tray_icon::TrayIconEvent::set_event_handler(Some(
                move |e: tray_icon::TrayIconEvent| {
                    if matches!(e, tray_icon::TrayIconEvent::Click { .. }) {
                        events
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .tray_clicked = true;
                        wake();
                    }
                },
            ));
        }

        /// Shows `menus` in the macOS menu bar (rebuilt only when changed).
        pub(crate) fn set_menu(
            &mut self,
            menus: &[NativeMenu],
            app_name: &str,
            wake: impl Fn() + Send + Sync + Clone + 'static,
        ) {
            if !cfg!(target_os = "macos") || self.menu.as_ref().is_some_and(|(m, _)| m == menus) {
                return;
            }
            self.set_handlers(wake);
            let bar = Menu::new();
            // The first menu is the application menu on macOS.
            let app = Submenu::with_items(
                app_name,
                true,
                &[
                    &PredefinedMenuItem::about(None, None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::hide(None),
                    &PredefinedMenuItem::hide_others(None),
                    &PredefinedMenuItem::show_all(None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::quit(None),
                ],
            );
            if let Ok(app) = app {
                let _ = bar.append(&app);
            }
            for menu in menus {
                let _ = bar.append(&submenu(menu));
            }
            #[cfg(target_os = "macos")]
            bar.init_for_nsapp();
            self.menu = Some((menus.to_vec(), bar));
        }

        /// Shows, updates or removes the tray icon.
        pub(crate) fn set_tray(
            &mut self,
            tray: Option<&TrayOptions>,
            wake: impl Fn() + Send + Sync + Clone + 'static,
        ) {
            let Some(options) = tray else {
                self.tray = None;
                return;
            };
            if self.tray.as_ref().is_some_and(|(t, _)| t == options) {
                return;
            }
            self.set_handlers(wake);
            let [w, h] = options.icon.size;
            let rgba: Vec<u8> = options.icon.pixels.iter().flatten().copied().collect();
            let Ok(icon) = tray_icon::Icon::from_rgba(rgba, w, h) else {
                log::warn!("invalid tray icon");
                return;
            };
            let menu = Menu::new();
            for item in items(&options.menu) {
                let _ = menu.append(item.as_ref());
            }
            match tray_icon::TrayIconBuilder::new()
                .with_icon(icon)
                .with_tooltip(&options.tooltip)
                .with_menu(Box::new(menu))
                .build()
            {
                Ok(icon) => self.tray = Some((options.clone(), icon)),
                Err(e) => log::warn!("could not create the tray icon: {e}"),
            }
        }

        /// Takes the events received since the last frame.
        pub(crate) fn take_events(&self) -> Events {
            std::mem::take(&mut *self.events.lock().unwrap_or_else(PoisonError::into_inner))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustroke_core::Modifiers;

    #[test]
    fn shortcuts_become_accelerators() {
        let save = KeyboardShortcut::new(
            Modifiers {
                shift: true,
                ..Modifiers::COMMAND
            },
            Key::S,
        );
        assert_eq!(accelerator(&save).as_deref(), Some("CmdOrCtrl+Shift+S"));
        let up = KeyboardShortcut::new(Modifiers::NONE, Key::ArrowUp);
        assert_eq!(accelerator(&up).as_deref(), Some("Up"));
    }
}
