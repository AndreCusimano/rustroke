//! The two layout containers: `Stack` (a row, a column, or items one over
//! the other) and `Grid` (rows and columns), each in a box with an
//! optional background (color, gradient, image), border, padding and size.
//!
//! Shown here: a toolbar whose search field grows, cards that wrap to the
//! window width, a badge over an image (overlay), the ways an image can
//! cover a box, and a form on a grid with fixed and fractional columns
//! and a cell spanning two columns. Resize the window.
//!
//! Run with: `cargo run -p rustroke --example stack`

use rustroke::{
    App, Color, ColorImage, Frame, Gradient, Grid, ImageFit, Justify, Label, ScrollArea, Shadow,
    Stack, StackAlign, StackItem, Stroke, TextEdit, TextureHandle, Track, WindowOptions, point,
    vec2,
};

struct Demo {
    search: String,
    name: String,
    notes: String,
    pattern: Option<TextureHandle>,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        let pattern = self
            .pattern
            .get_or_insert_with(|| {
                // A small image made in code: a soft checkerboard.
                let image = ColorImage::from_fn([48, 48], |x, y| {
                    let on = (x / 12 + y / 12) % 2 == 0;
                    let base = Color::from_srgb8(137, 180, 250)
                        .lerp(Color::from_srgb8(203, 166, 247), y as f32 / 48.0);
                    if on { base } else { base.with_alpha(0.35) }
                });
                frame.ctx().load_texture(image)
            })
            .clone();
        frame.ui(|ui| {
            let style = ui.style();
            let visuals = &style.visuals;
            let bold = style.body.clone().bold();
            let card = |stack: Stack| {
                stack
                    .fill(visuals.window_fill)
                    .stroke(visuals.window_stroke)
                    .corner_radius(8.0)
                    .padding(12.0)
                    .shadow(Shadow {
                        offset: vec2(0.0, 2.0),
                        blur: 8.0,
                        spread: 0.0,
                        color: Color::BLACK.with_alpha(0.15),
                    })
            };
            ScrollArea::vertical().show(ui, |ui| {
                ui.heading("A toolbar: the search field grows");
                Stack::row()
                    .fill_width()
                    .align(StackAlign::Center)
                    .fill(visuals.panel_fill)
                    .padding(6.0)
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.button("Open");
                        ui.button("Save");
                        ui.stack_item(
                            StackItem::new().grow(1.0).min_size(vec2(120.0, 0.0)),
                            |ui| {
                                let width = ui.available_width();
                                ui.add(
                                    TextEdit::singleline(&mut self.search)
                                        .hint_text("Search")
                                        .desired_width(width),
                                )
                            },
                        );
                        ui.button("Go");
                    });

                ui.add_space(8.0);
                ui.heading("Cards that wrap");
                Stack::row().wrap(true).gap(12.0).show(ui, |ui| {
                    for i in 1..=6 {
                        card(Stack::column())
                            .size(vec2(150.0, 90.0))
                            .show(ui, |ui| {
                                ui.add(Label::new(format!("Card {i}")).style(bold.clone()));
                                ui.label("A column in a box.");
                            });
                    }
                });

                ui.add_space(8.0);
                ui.heading("Space between, centered");
                Stack::row()
                    .fill_width()
                    .justify(Justify::SpaceBetween)
                    .stroke(Stroke::new(1.0, visuals.weak_text))
                    .padding(6.0)
                    .show(ui, |ui| {
                        for name in ["Left", "Middle", "Right"] {
                            ui.button(name);
                        }
                    });

                ui.add_space(8.0);
                ui.heading("Overlay: a badge over a picture");
                Stack::overlay().show(ui, |ui| {
                    Stack::column()
                        .size(vec2(220.0, 120.0))
                        .image(&pattern, ImageFit::Tile)
                        .corner_radius(8.0)
                        .show(ui, |_| {});
                    ui.stack_item(StackItem::new().align_x(StackAlign::End), |ui| {
                        Stack::row()
                            .fill(visuals.error)
                            .corner_radius(10.0)
                            .padding(vec2(8.0, 2.0))
                            .margin(6.0)
                            .show(ui, |ui| ui.add(Label::new("NEW").color(visuals.on_accent)));
                    });
                    ui.stack_item(StackItem::new().align(StackAlign::Center), |ui| {
                        ui.add(Label::new("Centered caption").style(bold.clone()))
                    });
                });

                ui.add_space(8.0);
                ui.heading("Backgrounds: gradient and image fits");
                Stack::row().wrap(true).gap(12.0).show(ui, |ui| {
                    Stack::column()
                        .size(vec2(120.0, 80.0))
                        .corner_radius(8.0)
                        .gradient(Gradient::linear(
                            point(0.0, 0.0),
                            point(1.0, 1.0),
                            visuals.accent,
                            Color::from_srgb8(243, 139, 168),
                        ))
                        .padding(8.0)
                        .show(ui, |ui| {
                            ui.add(Label::new("Gradient").color(visuals.on_accent))
                        });
                    for (fit, name) in [
                        (ImageFit::Stretch, "Stretch"),
                        (ImageFit::Contain, "Contain"),
                        (ImageFit::Cover, "Cover"),
                        (ImageFit::Center, "Center"),
                        (ImageFit::Tile, "Tile"),
                    ] {
                        Stack::column()
                            .size(vec2(120.0, 80.0))
                            .image(&pattern, fit)
                            .stroke(Stroke::new(1.0, visuals.weak_text))
                            .padding(6.0)
                            .show(ui, |ui| ui.add(Label::new(name).style(bold.clone())));
                    }
                });

                ui.add_space(8.0);
                ui.heading("A grid: 100 pt, 1fr and 2fr columns");
                Grid::new("form")
                    .columns([
                        Track::Points(100.0),
                        Track::Fraction(1.0),
                        Track::Fraction(2.0),
                    ])
                    .style(rustroke::BoxStyle::group(&style))
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Name");
                        ui.grid_span(2, 1, |ui| {
                            let width = ui.available_width();
                            ui.add(TextEdit::singleline(&mut self.name).desired_width(width))
                        });
                        ui.end_row();
                        ui.label("Notes");
                        ui.label("1fr");
                        let width = ui.available_width();
                        ui.add(
                            TextEdit::multiline(&mut self.notes)
                                .desired_width(width)
                                .desired_rows(2),
                        );
                        ui.end_row();
                        ui.label("Wide");
                        ui.grid_span(2, 1, |ui| ui.label("This cell spans two columns."));
                        ui.end_row();
                    });
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — stack and grid".into(),
            inner_size: (860.0, 760.0),
            ..Default::default()
        },
        Demo {
            search: String::new(),
            name: String::new(),
            notes: String::new(),
            pattern: None,
        },
    )
}
