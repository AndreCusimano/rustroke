//! A small CAD-like editor: a feature tree (right-click a node for its
//! context menu), a property panel with collapsible sections, drop-down
//! lists and numbers you can drag or type, menu shortcuts and a progress
//! bar.
//!
//! Run with: `cargo run -p rustroke --example properties`

use rustroke::{
    App, CentralPanel, CollapsingHeader, ComboBox, DragValue, Frame, Grid, Key, KeyboardShortcut,
    Modifiers, Panel, ProgressBar, Spinner, TextEdit, WindowOptions,
};

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const RECOMPUTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Operation {
    Extrude,
    Revolve,
    Cut,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Axis {
    X,
    Y,
    Z,
}

struct Feature {
    name: String,
    operation: Operation,
    axis: Axis,
    length: f64,
    angle: f64,
    count: u32,
}

struct Demo {
    features: Vec<Feature>,
    selected: usize,
    /// Seconds of the simulated recompute, while it runs.
    recompute_started: Option<f64>,
    status: String,
}

impl Demo {
    fn recompute(&mut self, now: f64) {
        self.recompute_started = Some(now);
        self.status = "Recomputing…".into();
    }
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        let now = frame.time.as_secs_f64();

        // Shortcuts work anywhere, except while typing in a text field.
        if !frame.ctx().wants_keyboard_input() {
            if frame.ctx().input_mut().consume_shortcut(&SAVE) {
                self.status = "Saved".into();
            }
            if frame.ctx().input_mut().consume_shortcut(&RECOMPUTE) {
                self.recompute(now);
            }
        }

        Panel::top("menu").show(frame, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui
                        .add(rustroke::Button::new("Save").shortcut_text(SAVE.format()))
                        .clicked()
                    {
                        self.status = "Saved".into();
                    }
                });
                ui.menu_button("Model", |ui| {
                    if ui
                        .add(rustroke::Button::new("Recompute").shortcut_text(RECOMPUTE.format()))
                        .clicked()
                    {
                        self.recompute(now);
                    }
                });
            });
        });

        Panel::bottom("status").show(frame, |ui| {
            ui.horizontal(|ui| {
                if let Some(start) = self.recompute_started {
                    let progress = ((now - start) / 3.0) as f32;
                    if progress >= 1.0 {
                        self.recompute_started = None;
                        self.status = "Model up to date".into();
                    } else {
                        ui.add(Spinner::new());
                        let done = (progress * 7.0) as u32;
                        ui.add(
                            ProgressBar::new(progress)
                                .text(format!("{done}/7"))
                                .desired_width(160.0),
                        );
                    }
                }
                ui.label(&self.status);
            });
        });

        Panel::left("tree").default_size(200.0).show(frame, |ui| {
            CollapsingHeader::new("Part 1")
                .default_open(true)
                .show(ui, |ui| {
                    let mut delete = None;
                    for (i, feature) in self.features.iter().enumerate() {
                        let node = CollapsingHeader::new(&feature.name)
                            .id_salt(i)
                            .selected(self.selected == i)
                            .show(ui, |ui| {
                                ui.label(format!("{:?}", feature.operation));
                            });
                        let header = node.header_response;
                        if header.clicked() {
                            self.selected = i;
                        }
                        header.context_menu(ui, |ui| {
                            if ui.button("Select").clicked() {
                                self.selected = i;
                            }
                            if ui.button("Delete").clicked() {
                                delete = Some(i);
                            }
                        });
                    }
                    if let Some(i) = delete {
                        self.features.remove(i);
                        self.selected = self.selected.min(self.features.len().saturating_sub(1));
                    }
                });
        });

        CentralPanel.show(frame, |ui| {
            let Some(feature) = self.features.get_mut(self.selected) else {
                ui.label("No feature selected");
                return;
            };
            ui.heading(&feature.name);
            CollapsingHeader::new("General")
                .default_open(true)
                .show(ui, |ui| {
                    Grid::new("general").show(ui, |ui| {
                        ui.label("Name");
                        ui.add(
                            TextEdit::singleline(&mut feature.name)
                                .select_all_on_focus(true)
                                .desired_width(180.0),
                        );
                        ui.end_row();
                        ui.label("Operation");
                        ComboBox::from_id_salt("operation")
                            .selected_text(format!("{:?}", feature.operation))
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                for op in [Operation::Extrude, Operation::Revolve, Operation::Cut] {
                                    ui.selectable_value(
                                        &mut feature.operation,
                                        op,
                                        format!("{op:?}"),
                                    );
                                }
                            });
                        ui.end_row();
                    });
                });
            CollapsingHeader::new("Parameters")
                .default_open(true)
                .show(ui, |ui| {
                    Grid::new("parameters").show(ui, |ui| {
                        ui.label("Length");
                        ui.add(
                            DragValue::new(&mut feature.length)
                                .speed(0.5)
                                .range(0.0..=1000.0)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label("Angle");
                        ui.add(
                            DragValue::new(&mut feature.angle)
                                .speed(1.0)
                                .range(-360.0..=360.0)
                                .suffix("°"),
                        );
                        ui.end_row();
                        ui.label("Axis");
                        ComboBox::from_id_salt("axis")
                            .selected_text(format!("{:?}", feature.axis))
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                for axis in [Axis::X, Axis::Y, Axis::Z] {
                                    ui.selectable_value(
                                        &mut feature.axis,
                                        axis,
                                        format!("{axis:?}"),
                                    );
                                }
                            });
                        ui.end_row();
                        ui.label("Count");
                        ui.add(DragValue::new(&mut feature.count).range(1..=50).speed(0.1));
                        ui.end_row();
                    });
                });
            CollapsingHeader::new("Advanced").show(ui, |ui| {
                ui.label("Nothing here yet.");
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    let feature = |name: &str, operation, length| Feature {
        name: name.into(),
        operation,
        axis: Axis::Z,
        length,
        angle: 0.0,
        count: 1,
    };
    rustroke::run(
        WindowOptions {
            title: "Rustroke — properties".into(),
            inner_size: (960.0, 620.0),
        },
        Demo {
            features: vec![
                feature("Base plate", Operation::Extrude, 10.0),
                feature("Shaft", Operation::Revolve, 42.5),
                feature("Holes", Operation::Cut, 8.0),
            ],
            selected: 0,
            recompute_started: None,
            status: "Ready".into(),
        },
    )
}
