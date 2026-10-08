//! Phase 3 demo: the immediate-mode API. All state lives in the app
//! struct; the UI is described again every frame.
//!
//! Try the mouse, and the keyboard: Tab / Shift+Tab move the focus,
//! Enter/Space press buttons and toggle checkboxes, arrows move sliders.
//!
//! Run with: `cargo run -p rustroke --example widgets`

use rustroke::{App, Color, Frame, Slider, Stroke, Ui, WindowOptions, vec2};

#[derive(Clone, Copy, PartialEq)]
enum Size {
    Small,
    Medium,
    Large,
}

struct Demo {
    clicks: u32,
    dark_corners: bool,
    show_preview: bool,
    size: Size,
    color: [u8; 3],
    opacity: f32,
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            clicks: 0,
            dark_corners: true,
            show_preview: true,
            size: Size::Medium,
            color: [137, 180, 250],
            opacity: 0.8,
        }
    }
}

impl Demo {
    fn preview(&self, ui: &mut Ui<'_>) {
        let side = match self.size {
            Size::Small => 40.0,
            Size::Medium => 70.0,
            Size::Large => 100.0,
        };
        let radius = if self.dark_corners { 16.0 } else { 0.0 };
        let [r, g, b] = self.color;
        let fill = Color::from_srgb8(r, g, b).with_alpha(self.opacity);
        let rect = ui.allocate_rect(vec2(side, side));
        let outline = ui.style().visuals.weak_text;
        ui.painter()
            .rect(rect, radius, fill, Stroke::new(1.0, outline));
    }
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        frame.ui(|ui| {
            ui.heading("Widgets");
            ui.label("Tab moves the focus, Enter/Space activate, arrows move sliders.");
            ui.separator();

            if ui.button("Click me").clicked() {
                self.clicks += 1;
            }
            ui.label(format!("Clicks: {}", self.clicks));
            if self.clicks > 0 && ui.button("Reset").clicked() {
                self.clicks = 0;
            }
            ui.separator();

            ui.checkbox(&mut self.show_preview, "Show preview");
            ui.checkbox(&mut self.dark_corners, "Rounded corners");

            ui.label("Size");
            ui.radio_value(&mut self.size, Size::Small, "Small");
            ui.radio_value(&mut self.size, Size::Medium, "Medium");
            ui.radio_value(&mut self.size, Size::Large, "Large");

            for (channel, name) in self.color.iter_mut().zip(["Red", "Green", "Blue"]) {
                ui.add(Slider::new(channel, 0..=255).text(name));
            }
            ui.add(
                Slider::new(&mut self.opacity, 0.0..=1.0)
                    .step(0.05)
                    .text("Opacity"),
            );

            if self.show_preview {
                self.preview(ui);
            }
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "widgets".to_owned(),
            inner_size: (520.0, 720.0),
        },
        Demo::default(),
    )
}
