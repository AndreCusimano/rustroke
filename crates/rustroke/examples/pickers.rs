//! Pickers and toasts: a color picker (with and without opacity), a date
//! picker, a time picker, and toasts of every level.
//!
//! Run with: `cargo run -p rustroke --example pickers`

use rustroke::{
    App, Color, ColorPicker, Date, DatePicker, Frame, Grid, TimePicker, Toast, WindowOptions,
};

struct Demo {
    accent: Color,
    overlay: Color,
    date: Date,
    hour: u32,
    minute: u32,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        let style = frame.ctx().style().as_ref().clone();
        if style.visuals.accent != self.accent {
            let style = rustroke::Style {
                visuals: style.visuals.with_accent(self.accent),
                ..style
            };
            frame.ctx().set_style(style);
        }
        let mut toast = None;
        frame.ui(|ui| {
            ui.heading("Pickers");
            Grid::new("pickers").show(ui, |ui| {
                ui.label("Accent color");
                ui.add(ColorPicker::new(&mut self.accent));
                ui.end_row();

                ui.label("Overlay (with opacity)");
                ui.add(ColorPicker::new(&mut self.overlay).alpha(true));
                ui.end_row();

                ui.label("Delivery date");
                if ui.add(DatePicker::new(&mut self.date)).changed() {
                    toast = Some(Toast::info(format!("Delivery moved to {}", self.date)));
                }
                ui.end_row();

                ui.label("Time");
                ui.add(TimePicker::new(&mut self.hour, &mut self.minute));
                ui.end_row();
            });
            ui.separator();
            ui.heading("Toasts");
            ui.horizontal(|ui| {
                if ui.button("Info").clicked() {
                    toast = Some(Toast::info("Rebuilding the model…"));
                }
                if ui.button("Success").clicked() {
                    toast = Some(Toast::success("Saved bracket.cad"));
                }
                if ui.button("Warning").clicked() {
                    toast = Some(Toast::warning("The sketch is under-constrained"));
                }
                if ui.button("Error").clicked() {
                    toast = Some(Toast::error("Fillet failed: radius too large for the edge"));
                }
            });
            let summary = format!("{} at {:02}:{:02}", self.date, self.hour, self.minute);
            ui.label(summary);
        });
        if let Some(toast) = toast {
            frame.ctx().toast(toast);
        }
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — pickers".into(),
            inner_size: (640.0, 460.0),
            ..Default::default()
        },
        Demo {
            accent: Color::from_srgb8(137, 180, 250),
            overlay: Color::from_srgba8(243, 139, 168, 128),
            date: Date::today_utc(),
            hour: 9,
            minute: 30,
        },
    )
}
