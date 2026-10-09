//! Phase 7 demo: styling. Switch between the dark and light theme, pick an
//! accent color and tune corner radius, spacing and animation speed while
//! the app runs. Hover the widgets to see the color transitions.
//!
//! Run with: `cargo run -p rustroke --example themes`

use rustroke::{App, Color, Frame, Grid, Panel, Slider, Style, TextEdit, WindowOptions, point};

#[derive(Clone, Copy, PartialEq)]
enum Theme {
    Dark,
    Light,
}

/// Accent colors: (name, for the dark theme, for the light theme).
const ACCENTS: [(&str, [u8; 3], [u8; 3]); 4] = [
    ("Blue", [137, 180, 250], [30, 102, 245]),
    ("Purple", [203, 166, 247], [136, 57, 239]),
    ("Green", [166, 227, 161], [64, 160, 43]),
    ("Peach", [250, 179, 135], [254, 100, 11]),
];

struct Demo {
    theme: Theme,
    accent: usize,
    radius: f32,
    spacing: f32,
    animation: f32,
    // Sample widgets.
    check: bool,
    choice: u8,
    value: f32,
    text: String,
    show_window: bool,
}

impl Demo {
    fn style(&self) -> Style {
        let mut style = match self.theme {
            Theme::Dark => Style::dark(),
            Theme::Light => Style::light(),
        };
        let (_, dark, light) = ACCENTS[self.accent];
        let [r, g, b] = if self.theme == Theme::Dark {
            dark
        } else {
            light
        };
        style.visuals = style.visuals.with_accent(Color::from_srgb8(r, g, b));
        style.visuals.corner_radius = self.radius;
        style.visuals.small_corner_radius = (self.radius * 0.6).min(6.0);
        style.visuals.window_corner_radius = self.radius + 4.0;
        style.spacing.item_spacing = rustroke::vec2(self.spacing, self.spacing);
        style.animation_time = self.animation;
        style
    }
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        let style = self.style();
        frame.clear_color = style.visuals.background;
        if **frame.ctx().style() != style {
            frame.ctx().set_style(style);
        }

        Panel::left("settings")
            .default_size(270.0)
            .show(frame, |ui| {
                ui.heading("Style");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.theme, Theme::Dark, "Dark");
                    ui.radio_value(&mut self.theme, Theme::Light, "Light");
                });
                ui.label("Accent");
                ui.horizontal_wrapped(|ui| {
                    for (i, (name, _, _)) in ACCENTS.iter().enumerate() {
                        ui.radio_value(&mut self.accent, i, *name);
                    }
                });
                ui.separator();
                Grid::new("tuning").show(ui, |ui| {
                    ui.label("Radius");
                    ui.add(Slider::new(&mut self.radius, 0.0..=14.0).step(1.0));
                    ui.end_row();
                    ui.label("Spacing");
                    ui.add(Slider::new(&mut self.spacing, 2.0..=16.0).step(1.0));
                    ui.end_row();
                    ui.label("Animation");
                    ui.add(
                        Slider::new(&mut self.animation, 0.0..=1.0)
                            .step(0.05)
                            .text("s"),
                    );
                    ui.end_row();
                });
            });

        frame.ui(|ui| {
            ui.heading("Preview");
            ui.label("Hover the widgets to see the color transitions.");
            ui.horizontal(|ui| {
                if ui.button("Open window").clicked() {
                    self.show_window = true;
                }
                ui.button("Button");
                ui.menu_button("Menu", |ui| {
                    ui.button("Item one");
                    ui.button("Item two");
                });
            });
            ui.checkbox(&mut self.check, "Checkbox");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.choice, 0, "One");
                ui.radio_value(&mut self.choice, 1, "Two");
                ui.radio_value(&mut self.choice, 2, "Three");
            });
            ui.add(Slider::new(&mut self.value, 0.0..=100.0).text("Value"));
            ui.add(TextEdit::singleline(&mut self.text).hint_text("Type something…"));
            Grid::new("table").striped(true).show(ui, |ui| {
                for (k, v) in [
                    ("Name", "Rustroke"),
                    ("Version", "0.1"),
                    ("Theme", "live"),
                    ("Rows", "striped"),
                ] {
                    ui.label(k);
                    ui.label(v);
                    ui.end_row();
                }
            });
        });

        rustroke::Window::new("Window")
            .open(&mut self.show_window)
            .default_pos(point(560.0, 260.0))
            .show(frame, |ui| {
                ui.label("Windows, menus and tooltips follow the theme too.");
                ui.button("OK").on_hover_text(ui, "A themed tooltip");
            });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "themes".to_owned(),
            inner_size: (900.0, 560.0),
            ..Default::default()
        },
        Demo {
            theme: Theme::Light,
            accent: 0,
            radius: 6.0,
            spacing: 8.0,
            animation: 0.12,
            check: true,
            choice: 1,
            value: 40.0,
            text: String::new(),
            show_window: true,
        },
    )
}
