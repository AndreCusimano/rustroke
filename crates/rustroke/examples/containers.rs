//! Phase 5 demo: containers. A menu bar, a resizable side panel with a
//! scrolling list, a status bar, floating windows (drag the title, resize
//! from the corner, close with ×), tooltips and a modal dialog
//! (File › Quit). On macOS the menu bar sits in a unified title bar,
//! next to the window buttons.
//!
//! Run with: `cargo run -p rustroke --example containers` (add
//! `--features persistence` to keep the layout between runs).

use rustroke::{App, CentralPanel, Frame, Modal, Panel, ScrollArea, Slider, WindowOptions, point};

struct Demo {
    selected: usize,
    show_inspector: bool,
    show_about: bool,
    zoom: f32,
    grid: bool,
    status: String,
    confirm_quit: bool,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;

        // With a unified title bar (macOS) the menu bar shares the top
        // strip with the window buttons.
        let titlebar = frame.titlebar_height();
        Panel::top("menu").show(frame, |ui| {
            ui.horizontal(|ui| {
                if titlebar > 0.0 {
                    ui.add_space(70.0);
                }
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.status = "File › New".into();
                    }
                    if ui.button("Open…").clicked() {
                        self.status = "File › Open".into();
                    }
                    ui.menu_button("Open recent", |ui| {
                        for name in ["bracket.cad", "gearbox.cad", "housing.cad"] {
                            if ui.button(name).clicked() {
                                self.status = format!("File › Open recent › {name}");
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        self.confirm_quit = true;
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

        CentralPanel::default().show(frame, |ui| {
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
                ui.hyperlink_to("rustroke on crates.io", "https://crates.io/crates/rustroke");
            });

        // A modal dialog: everything else is blocked until it is answered.
        if self.confirm_quit {
            let dialog = Modal::new("quit").title("Quit?").show(frame, |ui| {
                ui.label("Unsaved screens will be lost.");
                ui.horizontal(|ui| (ui.button("Quit").clicked(), ui.button("Cancel").clicked()))
                    .inner
            });
            let (quit, cancel) = dialog.inner;
            if quit {
                frame.close();
            }
            if cancel || dialog.should_close {
                self.confirm_quit = false;
            }
        }
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "containers".to_owned(),
            inner_size: (900.0, 600.0),
            unified_titlebar: true,
            // With `--features persistence` window positions, the panel
            // width and the window size are restored on the next run.
            persistence_id: Some("rustroke.containers-example".into()),
            min_inner_size: Some((640.0, 420.0)),
            ..Default::default()
        },
        Demo {
            selected: 0,
            show_inspector: true,
            show_about: true,
            zoom: 100.0,
            grid: true,
            status: "Ready".into(),
            confirm_quit: false,
        },
    )
}
