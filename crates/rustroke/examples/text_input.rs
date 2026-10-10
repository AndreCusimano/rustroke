//! Phase 6 demo: text input. Single-line fields with hints, a multi-line
//! editor, live preview of what is typed.
//!
//! Try: click to place the cursor, drag or Shift+arrows to select,
//! Cmd/Ctrl+A/C/X/V, Option/Ctrl+arrows to move by word, accented and
//! Asian input methods, double-click to select a word and triple-click
//! a line. Enter in a single-line field ends editing.
//!
//! The code editor below uses a monospace font, line numbers, syntax
//! colors (`TextEdit::layouter`), Tab to indent, no wrapping (it scrolls
//! sideways) and marks the lines that call `unwrap` as errors.
//!
//! Run with: `cargo run -p rustroke --example text_input`

use rustroke::{
    App, Color, Frame, Grid, Id, LayoutJob, LineHighlight, ScrollArea, TextEdit, TextFormat,
    TextStyle, WindowOptions,
};

/// Words the code editor shows in the keyword color.
const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "for", "in", "while", "return", "match", "struct", "impl",
    "use", "pub",
];

/// Splits `code` into sections: keywords, comments, strings and the rest.
fn highlight(code: &str, dark: bool) -> LayoutJob {
    let (keyword, comment, string) = if dark {
        (
            Color::from_srgb8(255, 123, 114),
            Color::from_srgb8(139, 148, 158),
            Color::from_srgb8(165, 214, 255),
        )
    } else {
        (
            Color::from_srgb8(207, 34, 46),
            Color::from_srgb8(110, 119, 129),
            Color::from_srgb8(10, 48, 105),
        )
    };
    let mut job = LayoutJob::default();
    let mut rest = code;
    while !rest.is_empty() {
        let (len, color) = if rest.starts_with("//") {
            (rest.find('\n').unwrap_or(rest.len()), Some(comment))
        } else if let Some(after) = rest.strip_prefix('"') {
            let end = after.find('"').map_or(rest.len(), |i| i + 2);
            (end, Some(string))
        } else if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let word = &rest[..end];
            (end, KEYWORDS.contains(&word).then_some(keyword))
        } else {
            (rest.chars().next().map_or(1, char::len_utf8), None)
        };
        let format = TextFormat {
            color,
            ..TextFormat::new()
        };
        job.append(&rest[..len], format);
        rest = &rest[len..];
    }
    job
}

struct Demo {
    name: String,
    city: String,
    password: String,
    notes: String,
    code: String,
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

            ui.separator();
            ui.label("Code (Tab indents, Shift+Tab outdents):");
            let code_id = Id::new("code");
            let dark = ui.style().visuals.dark_mode;
            let error = ui.style().visuals.error;
            let errors: Vec<usize> = self
                .code
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains("unwrap"))
                .map(|(i, _)| i)
                .collect();
            let mut field = TextEdit::multiline(&mut self.code)
                .id(code_id)
                .font(TextStyle::monospace(13.0))
                .code_editor()
                .wrap(false)
                .line_numbers(true)
                .desired_rows(8)
                .layouter(move |code| highlight(code, dark));
            for &line in &errors {
                field = field
                    .highlight_line(line, error.with_alpha(0.12), LineHighlight::Background)
                    .highlight_line(line, error, LineHighlight::Underline);
            }
            ScrollArea::both().max_height(180.0).show(ui, |ui| {
                ui.add(field);
            });
            if let Some(&line) = errors.first()
                && ui
                    .button(format!("Go to the error (line {})", line + 1))
                    .clicked()
            {
                let index = TextEdit::line_column_to_index(&self.code, line, 0);
                TextEdit::set_selection(ui.ctx(), code_id, index, index);
                ui.ctx().request_focus(code_id);
            }

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
            inner_size: (640.0, 860.0),
            ..Default::default()
        },
        Demo {
            name: String::new(),
            city: String::new(),
            password: "secret".into(),
            notes: "This is a multi-line note.\nLong lines wrap automatically when they reach the edge of the field, and the field grows with its content.".to_owned(),
            code: CODE.to_owned(),
            submitted: Vec::new(),
        },
    )
}

const CODE: &str = r#"// A tiny program to edit.
use std::collections::HashMap;

fn main() {
    let mut words: HashMap<&str, usize> = HashMap::new();
    for word in "the quick brown fox jumps over the lazy dog, then the fox sleeps".split_whitespace() {
        *words.entry(word).or_default() += 1;
    }
    let most = words.iter().max_by_key(|(_, n)| **n).unwrap();
    println!("most common: {most:?}");
}
"#;
