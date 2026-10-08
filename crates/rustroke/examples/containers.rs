//! Phase 5 demo: containers. A menu bar, a resizable side panel with a
//! scrolling list, a status bar, floating windows (drag the title, resize
//! from the corner, close with ×) and tooltips.
//!
//! Run with: `cargo run -p rustroke --example containers`

use rustroke::{App, CentralPanel, Frame, Panel, ScrollArea, Slider, WindowOptions, point};

struct Demo {
    selected: usize,
    show_inspector: bool,
    show_about: bool,
    zoom: f32,
    grid: bool,
    status: String,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;

        Panel::top("menu").show(frame, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.status = "File › New".into();
                    }
                    if ui.button("Open…").clicked() {
                        self.status = "File › Open".into();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        self.status = "File › Quit (not implemented)".into();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.grid, "Show grid");
                    if ui.button("Inspector").clicked() {
                        self.show_inspector = true;
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about = true;
                    }
                });
            });
        });

        Panel::bottom("status").show(frame, |ui| {
            ui.label(&self.status);
        });

        Panel::left("files").default_size(190.0).show(frame, |ui| {
            ui.heading("Screens");
            ScrollArea::vertical().show(ui, |ui| {
                for i in 0..40 {
                    let name = format!("Screen {}", i + 1);
                    let marker = if i == self.selected { "▸ " } else { "" };
                    if ui
                        .button(format!("{marker}{name}"))
                        .on_hover_text(ui, format!("Open {name}"))
                        .clicked()
                    {
                        self.selected = i;
                        self.status = format!("Selected: {name}");
                    }
                }
            });
        });

        CentralPanel.show(frame, |ui| {
            ui.heading(format!("Screen {}", self.selected + 1));
            ui.label("Drag the edge of the left panel to resize it. Use the wheel or the bar to scroll the list.");
            ui.horizontal(|ui| {
                if ui.button("Open inspector").on_hover_text(ui, "A floating window with properties").clicked() {
                    self.show_inspector = true;
                }
                if ui.button("About").clicked() {
                    self.show_about = true;
                }
            });
            ui.add(Slider::new(&mut self.zoom, 25.0..=400.0).step(5.0).text("Zoom %"));
            ui.checkbox(&mut self.grid, "Show grid");
        });

        rustroke::Window::new("Inspector")
            .open(&mut self.show_inspector)
            .default_pos(point(420.0, 140.0))
            .show(frame, |ui| {
                ui.label("Selection properties");
                ui.add(
                    Slider::new(&mut self.zoom, 25.0..=400.0)
                        .step(5.0)
                        .text("Zoom %"),
                );
                ui.checkbox(&mut self.grid, "Grid");
            });

        rustroke::Window::new("About")
            .open(&mut self.show_about)
            .default_pos(point(480.0, 260.0))
            .default_width(260.0)
            .show(frame, |ui| {
                ui.label("Rustroke — an immediate-mode GUI library for Rust. Windows, panels, menus and tooltips.");
            });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "containers".to_owned(),
            inner_size: (900.0, 600.0),
        },
        Demo {
            selected: 0,
            show_inspector: true,
            show_about: true,
            zoom: 100.0,
            grid: true,
            status: "Ready".into(),
        },
    )
}
