//! Open and save files with the system's file dialogs. Choosing files is
//! not the GUI library's job: this example uses the `rfd` crate (MIT), which
//! shows the native dialogs of macOS, Windows and Linux.
//!
//! Run with: `cargo run -p rustroke --example files`

use std::path::PathBuf;

use rustroke::{
    App, Frame, Key, KeyboardShortcut, Modifiers, Panel, ScrollArea, TextEdit, WindowOptions,
};

const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

#[derive(Default)]
struct Editor {
    path: Option<PathBuf>,
    text: String,
    export_dir: Option<PathBuf>,
    status: String,
}

impl Editor {
    fn open(&mut self) {
        // Blocks until the user picks a file or cancels.
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Text", &["txt", "md", "rs", "toml"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.text = text;
                self.status = format!("Opened {}", path.display());
                self.path = Some(path);
            }
            Err(e) => self.status = format!("Can't read {}: {e}", path.display()),
        }
    }

    fn save_as(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("Text", &["txt"]);
        if let Some(name) = self.path.as_ref().and_then(|p| p.file_name()) {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        self.status = match std::fs::write(&path, &self.text) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Can't write {}: {e}", path.display()),
        };
        self.path = Some(path);
    }
}

impl App for Editor {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        if frame.ctx().input_mut().consume_shortcut(&OPEN) {
            self.open();
        }
        if frame.ctx().input_mut().consume_shortcut(&SAVE) {
            self.save_as();
        }
        let title = match &self.path {
            Some(path) => format!("{} — Rustroke", path.display()),
            None => "Untitled — Rustroke".to_owned(),
        };
        frame.set_title(title);

        Panel::top("toolbar").show(frame, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button("Open…")
                    .on_hover_text(ui, OPEN.format())
                    .clicked()
                {
                    self.open();
                }
                if ui
                    .button("Save as…")
                    .on_hover_text(ui, SAVE.format())
                    .clicked()
                {
                    self.save_as();
                }
                ui.separator();
                if ui.button("Export folder…").clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    self.export_dir = Some(dir);
                }
                if let Some(dir) = &self.export_dir {
                    ui.label(format!("Exports go to {}", dir.display()));
                }
            });
        });
        Panel::bottom("status").show(frame, |ui| {
            ui.label(&self.status);
        });
        frame.ui(|ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut self.text)
                        .hint_text("Open a file, or type here")
                        .desired_rows(20),
                );
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — files".into(),
            inner_size: (760.0, 520.0),
        },
        Editor {
            status: "Ready".into(),
            ..Editor::default()
        },
    )
}
