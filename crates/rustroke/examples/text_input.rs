//! Phase 6 demo: text input. Single-line fields with hints, a multi-line
//! editor, live preview of what is typed.
//!
//! Try: click to place the cursor, drag or Shift+arrows to select,
//! Cmd/Ctrl+A/C/X/V, Option/Ctrl+arrows to move by word, accented and
//! Asian input methods, double-click to select a word and triple-click
//! a line. Enter in a single-line field ends editing.
//!
//! Run with: `cargo run -p rustroke --example text_input`

use rustroke::{App, Frame, Grid, ScrollArea, TextEdit, WindowOptions};

struct Demo {
    name: String,
    city: String,
    password: String,
    notes: String,
    submitted: Vec<String>,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        frame.ui(|ui| {
            ui.heading("Text input");
            Grid::new("form").show(ui, |ui| {
                ui.label("Name");
                let name = ui.add(TextEdit::singleline(&mut self.name).hint_text("Your name"));
                if name.lost_focus() && !self.name.is_empty() {
                    self.submitted.push(self.name.clone());
                }
                ui.end_row();

                ui.label("City");
                ui.add(
                    TextEdit::singleline(&mut self.city)
                        .hint_text("e.g. Milan")
                        .desired_width(180.0),
                );
                ui.end_row();

                ui.label("Password");
                ui.add(
                    TextEdit::singleline(&mut self.password)
                        .password(true)
                        .desired_width(180.0),
                );
                ui.end_row();
            });

            ui.label("Notes (multi-line, wraps by itself):");
            ui.add(TextEdit::multiline(&mut self.notes).desired_rows(5));

            ui.separator();
            let greeting = match (self.name.trim(), self.city.trim()) {
                ("", _) => "Type your name…".to_owned(),
                (name, "") => format!("Hello {name}!"),
                (name, city) => format!("Hello {name} from {city}!"),
            };
            ui.label(greeting);
            ui.label(format!(
                "Notes: {} characters, {} lines",
                self.notes.chars().count(),
                self.notes.lines().count().max(1)
            ));
            if !self.submitted.is_empty() {
                ui.label("Submitted with Enter:");
                ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
                    for entry in &self.submitted {
                        ui.label(format!("• {entry}"));
                    }
                });
            }
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "text_input".to_owned(),
            inner_size: (560.0, 560.0),
            ..Default::default()
        },
        Demo {
            name: String::new(),
            city: String::new(),
            password: "secret".into(),
            notes: "This is a multi-line note.\nLong lines wrap automatically when they reach the edge of the field, and the field grows with its content.".to_owned(),
            submitted: Vec::new(),
        },
    )
}
