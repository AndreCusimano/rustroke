//! Rich text: labels mixing styles, colors, highlights and links, a
//! Markdown document, and right-to-left text (Arabic, Hebrew) mixed with
//! left-to-right text in an editable field.
//!
//! Run with: `cargo run -p rustroke --example rich_text --features markdown`

use rustroke::{
    App, Color, Frame, Label, LayoutJob, Panel, ScrollArea, TextEdit, TextFormat, TextStyle,
    WindowOptions,
};

const DOCUMENT: &str = r#"# Release notes

Rustroke renders **Markdown** with *emphasis*, ~~struck~~ text, `inline code`
and [links](https://crates.io/crates/rustroke) that open in the browser.

## Lists

- Tables with sortable columns
- Trees that only lay out the visible rows
  1. expand with the arrows
  2. or with a double-click
- [x] rich text
- [ ] gradients (next)

> Quotes are indented with a bar,
> and can span several lines.

```
fn main() {
    rustroke::run(Default::default(), |frame| {
        frame.ui(|ui| ui.label("Hello!"));
    })
    .unwrap();
}
```

| Widget | Since |
|--------|-------|
| Table  | 0.10  |
| Tree   | 0.10  |
| Markdown | 0.11 |

---

That's all.
"#;

struct Demo {
    mixed: String,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        let accent = frame.ctx().style().visuals.accent;
        Panel::left("rich").default_size(330.0).show(frame, |ui| {
            ui.heading("Rich labels");
            let mut job = LayoutJob::default();
            job.append("Mix ", TextFormat::new());
            job.append(
                "bold",
                TextFormat::new().style(TextStyle::proportional(14.0).bold()),
            );
            job.append(", ", TextFormat::new());
            job.append("italic", TextFormat::new().italic());
            job.append(", ", TextFormat::new());
            job.append(
                "colored",
                TextFormat::new().color(Color::from_srgb8(243, 139, 168)),
            );
            job.append(", ", TextFormat::new());
            job.append(
                "highlighted",
                TextFormat::new().background(accent.with_alpha(0.3)),
            );
            job.append(", ", TextFormat::new());
            job.append("underlined", TextFormat::new().underline());
            job.append(" and ", TextFormat::new());
            job.append("struck", TextFormat::new().strikethrough());
            job.append(" text, ", TextFormat::new());
            job.append(
                "big ",
                TextFormat::new().style(TextStyle::proportional(22.0)),
            );
            job.append("and a ", TextFormat::new());
            job.append("link", TextFormat::new().link("https://docs.rs/rustroke"));
            job.append(
                " in one label that wraps like any other.",
                TextFormat::new(),
            );
            ui.add(Label::rich(job));

            ui.separator();
            ui.heading("Right to left");
            ui.label("مرحبا بالعالم — Arabic is shaped and aligned to the right.");
            ui.label("שלום עולם");
            ui.label("Mixed: the word שלום inside English text.");
            ui.label(
                "Edit mixed text (arrows move by character, selection follows the visual order):",
            );
            ui.add(TextEdit::singleline(&mut self.mixed).desired_width(300.0));
        });
        frame.ui(|ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.markdown(DOCUMENT);
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — rich text".into(),
            inner_size: (980.0, 680.0),
            ..Default::default()
        },
        Demo {
            mixed: "abc שלום def".into(),
        },
    )
}
