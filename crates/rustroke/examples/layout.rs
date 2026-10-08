//! Phase 4 demo: layout. Rows, right-aligned buttons, wrapping, a striped
//! grid form, centered content and equally sized buttons. Resize the
//! window to see wrapping and alignment follow.
//!
//! Run with: `cargo run -p rustroke --example layout`

use rustroke::{Align, App, Button, Frame, Grid, Layout, Slider, WindowOptions, vec2};

struct LayoutDemo {
    name: String,
    volume: f32,
    quality: u8,
    notifications: bool,
    dark: bool,
    clicks: u32,
}

const TAGS: [&str; 12] = [
    "rust",
    "gui",
    "immediate mode",
    "wgpu",
    "cosmic-text",
    "layout",
    "grid",
    "widgets",
    "desktop",
    "open source",
    "designer",
    "accessibility",
];

impl App for LayoutDemo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        frame.ui(|ui| {
            // Toolbar: buttons on the left, a separator, then right-aligned actions.
            ui.horizontal(|ui| {
                ui.heading("Layout");
                ui.separator();
                if ui.button("New").clicked() {
                    self.clicks += 1;
                }
                ui.button("Open");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.button("Quit");
                    ui.label(format!("{} clicks", self.clicks));
                });
            });
            ui.separator();

            ui.label("Tags that wrap when the window is narrow:");
            ui.horizontal_wrapped(|ui| {
                for tag in TAGS {
                    ui.button(tag);
                }
            });
            ui.separator();

            ui.label("Striped grid: columns align to their widest cell.");
            Grid::new("settings").striped(true).show(ui, |ui| {
                ui.label("Name");
                ui.label(&self.name);
                ui.end_row();

                ui.label("Volume");
                ui.add(Slider::new(&mut self.volume, 0.0..=100.0).step(1.0));
                ui.end_row();

                ui.label("Quality");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.quality, 0, "Low");
                    ui.radio_value(&mut self.quality, 1, "Medium");
                    ui.radio_value(&mut self.quality, 2, "High");
                });
                ui.end_row();

                ui.label("Notifications");
                ui.checkbox(&mut self.notifications, "Enabled");
                ui.end_row();

                ui.label("Dark theme");
                ui.checkbox(&mut self.dark, "");
                ui.end_row();
            });
            ui.separator();

            ui.vertical_centered(|ui| {
                ui.label("Centered content");
                ui.horizontal(|ui| {
                    for label in ["One", "Two", "Three"] {
                        ui.add_sized(vec2(90.0, 32.0), Button::new(label));
                    }
                });
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "layout".to_owned(),
            inner_size: (640.0, 640.0),
        },
        LayoutDemo {
            name: "André".to_owned(),
            volume: 65.0,
            quality: 1,
            notifications: true,
            dark: true,
            clicks: 0,
        },
    )
}
