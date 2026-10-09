//! Platform integration: drag cards between columns, drop files from the
//! system on the window, pinch and rotate on a trackpad, a native menu bar
//! (macOS), a tray icon and a system notification.
//!
//! Run with:
//! `cargo run -p rustroke --example platform --features native-menu,tray`

use rustroke::{
    App, Color, ColorImage, Frame, Id, Key, KeyboardShortcut, Modifiers, NativeMenu,
    NativeMenuItem, Panel, TrayOptions, WindowOptions, vec2,
};

struct Demo {
    columns: [Vec<String>; 3],
    files: Vec<String>,
    zoom: f32,
    rotation: f32,
    log: Vec<String>,
    tray_shown: bool,
}

const TITLES: [&str; 3] = ["To do", "Doing", "Done"];

fn tray_icon() -> ColorImage {
    ColorImage::from_fn([32, 32], |x, y| {
        let d = ((x as f32 - 15.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt();
        Color::from_srgb8(137, 180, 250).with_alpha((14.0 - d).clamp(0.0, 1.0))
    })
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;

        // Native menu bar (macOS) and tray icon: rebuilt only on change.
        let save = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
        frame.set_native_menu(vec![
            NativeMenu::new(
                "File",
                vec![
                    NativeMenuItem::action("new", "New card"),
                    NativeMenuItem::action_with_shortcut("save", "Save", save),
                ],
            ),
            NativeMenu::new(
                "Help",
                vec![NativeMenuItem::action("notify", "Send a notification")],
            ),
        ]);
        if !self.tray_shown {
            self.tray_shown = true;
            frame.set_tray(Some(TrayOptions {
                icon: tray_icon(),
                tooltip: "rustroke platform demo".into(),
                menu: vec![NativeMenuItem::action("notify", "Send a notification")],
            }));
        }
        for id in frame.native_menu_events().to_vec() {
            match id.as_str() {
                "new" => self.columns[0].push(format!("Card {}", self.log.len() + 1)),
                "notify" => {
                    let _ = rustroke::notify("rustroke", "Hello from the platform example!");
                }
                _ => {}
            }
            self.log.push(format!("menu: {id}"));
        }
        if frame.tray_clicked() {
            self.log.push("tray icon clicked".into());
        }

        // Files from the system and gestures.
        let input = frame.input();
        for path in &input.dropped_files {
            self.files.push(path.display().to_string());
        }
        let hovering = !input.hovered_files.is_empty();
        self.zoom *= input.zoom_delta;
        self.rotation += input.rotation_delta;

        Panel::bottom("log").show(frame, |ui| {
            ui.label(format!(
                "Pinch zoom {:.2} · rotation {:.0}° · {}",
                self.zoom,
                self.rotation.to_degrees(),
                self.log.last().map_or("no menu events yet", String::as_str)
            ));
        });
        Panel::right("files").default_size(260.0).show(frame, |ui| {
            ui.heading("Files");
            if hovering {
                ui.label("Drop them here!");
            } else if self.files.is_empty() {
                ui.label("Drag files from the Finder or Explorer onto the window.");
            }
            for f in &self.files {
                ui.label(f);
            }
            ui.separator();
            if ui.button("Send a notification").clicked() {
                let _ = rustroke::notify("rustroke", "Hello from the platform example!");
            }
        });
        frame.ui(|ui| {
            ui.heading("Drag the cards between the columns");
            let mut moved = None;
            ui.horizontal(|ui| {
                for (c, column) in self.columns.iter().enumerate() {
                    let (_, dropped) = ui.dnd_drop_zone::<(usize, usize), _>(|ui| {
                        ui.vertical(|ui| {
                            ui.add_sized(vec2(150.0, 24.0), rustroke::Label::new(TITLES[c]));
                            for (i, card) in column.iter().enumerate() {
                                ui.dnd_drag_source(Id::new(("card", card)), (c, i), |ui| {
                                    ui.add(
                                        rustroke::Button::new(card.as_str())
                                            .min_size(vec2(150.0, 32.0)),
                                    );
                                });
                            }
                            ui.add_space(40.0);
                        });
                    });
                    if let Some(from) = dropped {
                        moved = Some((*from, c));
                    }
                }
            });
            if let Some(((from, i), to)) = moved {
                let card = self.columns[from].remove(i);
                self.columns[to].push(card);
            }
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — platform".into(),
            inner_size: (900.0, 520.0),
            ..Default::default()
        },
        Demo {
            columns: [
                vec!["Sketch the part".into(), "Pick a material".into()],
                vec!["Model the housing".into()],
                vec!["Set up the project".into()],
            ],
            files: Vec::new(),
            zoom: 1.0,
            rotation: 0.0,
            log: Vec::new(),
            tray_shown: false,
        },
    )
}
