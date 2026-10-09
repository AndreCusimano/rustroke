//! Flexbox and grid layouts (computed with taffy): a toolbar whose search
//! field grows, cards that wrap to the window width, items spread with
//! "space between", and a form on a grid with fixed and fractional
//! columns and a cell spanning two columns. Resize the window.
//!
//! Run with: `cargo run -p rustroke --example flex`

use rustroke::{
    App, Button, Flex, FlexAlign, FlexGrid, FlexItem, FlexJustify, Frame, GridCell, TextEdit,
    Track, WindowOptions, vec2,
};

struct Demo {
    search: String,
    name: String,
    notes: String,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        frame.ui(|ui| {
            ui.heading("A toolbar: the search field grows");
            Flex::row("toolbar")
                .align(FlexAlign::Center)
                .show(ui, |flex| {
                    flex.add(FlexItem::new(), |ui| ui.button("Open"));
                    flex.add(FlexItem::new(), |ui| ui.button("Save"));
                    flex.add(FlexItem::new().grow(1.0).basis(120.0), |ui| {
                        let width = ui.available_width();
                        ui.add(
                            TextEdit::singleline(&mut self.search)
                                .hint_text("Search")
                                .desired_width(width),
                        )
                    });
                    flex.add(FlexItem::new(), |ui| ui.button("Go"));
                });

            ui.separator();
            ui.heading("Cards that wrap");
            Flex::row("cards").wrap(true).gap(12.0).show(ui, |flex| {
                for i in 1..=9 {
                    flex.add(FlexItem::new(), |ui| {
                        ui.add(Button::new(format!("Card {i}")).min_size(vec2(120.0, 60.0)))
                    });
                }
            });

            ui.separator();
            ui.heading("Space between");
            Flex::row("spread")
                .justify(FlexJustify::SpaceBetween)
                .show(ui, |flex| {
                    for name in ["Left", "Middle", "Right"] {
                        flex.add(FlexItem::new(), |ui| ui.button(name));
                    }
                });

            ui.separator();
            ui.heading("A grid: 100 pt, 1fr and 2fr columns");
            FlexGrid::new(
                "form",
                vec![
                    Track::Points(100.0),
                    Track::Fraction(1.0),
                    Track::Fraction(2.0),
                ],
            )
            .show(ui, |grid| {
                grid.add(GridCell::at(0, 0), |ui| ui.label("Name"));
                grid.add(GridCell::at(1, 0).span(2, 1), |ui| {
                    let width = ui.available_width();
                    ui.add(TextEdit::singleline(&mut self.name).desired_width(width))
                });
                grid.add(GridCell::at(0, 1), |ui| ui.label("Notes"));
                grid.add(GridCell::at(1, 1), |ui| ui.label("1fr"));
                grid.add(GridCell::at(2, 1), |ui| {
                    let width = ui.available_width();
                    ui.add(
                        TextEdit::multiline(&mut self.notes)
                            .desired_width(width)
                            .desired_rows(2),
                    )
                });
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — flex and grid".into(),
            inner_size: (820.0, 600.0),
            ..Default::default()
        },
        Demo {
            search: String::new(),
            name: String::new(),
            notes: String::new(),
        },
    )
}
