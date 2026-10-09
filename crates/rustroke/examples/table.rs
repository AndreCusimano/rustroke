//! Tables: 100 000 parts in a table that only lays out the visible rows.
//! Click a header to sort, drag a header's edge to resize a column
//! (double-click it to fit the content), scroll sideways while the name
//! column stays, select rows with clicks or the arrow keys, edit the
//! quantity in place and expand a row with ▸ to see its details. On the
//! left, a tree of 1 000 assemblies with 100 parts each (open with the
//! arrows, →/← or a double-click).
//!
//! Run with: `cargo run -p rustroke --example table`

use rustroke::{Align, App, Column, DragValue, Frame, Panel, Table, TextEdit, Tree, WindowOptions};

struct Part {
    name: String,
    material: &'static str,
    quantity: u32,
    mass: f32,
    notes: String,
    expanded: bool,
}

struct Demo {
    parts: Vec<Part>,
    selected: Option<usize>,
    status: String,
    /// Selected tree node: an assembly (`< ASSEMBLIES`) or a part.
    tree_selected: Option<u32>,
}

/// Tree nodes: assemblies are 0..ASSEMBLIES, their parts come after.
const ASSEMBLIES: u32 = 1_000;
const PARTS_PER_ASSEMBLY: u32 = 100;

const MATERIALS: [&str; 4] = ["Steel", "Aluminium", "ABS", "Brass"];

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        Panel::bottom("status").show(frame, |ui| {
            ui.label(&self.status);
        });
        Panel::left("assemblies")
            .default_size(220.0)
            .show(frame, |ui| {
                ui.heading("Assemblies");
                let roots: Vec<u32> = (0..ASSEMBLIES).collect();
                Tree::new("assemblies").accessible_label("Assemblies").show(
                    ui,
                    &roots,
                    |n| {
                        if n < ASSEMBLIES {
                            let first = ASSEMBLIES + n * PARTS_PER_ASSEMBLY;
                            (first..first + PARTS_PER_ASSEMBLY).collect()
                        } else {
                            Vec::new()
                        }
                    },
                    &mut self.tree_selected,
                    |ui, n| {
                        if n < ASSEMBLIES {
                            ui.label(format!("Assembly {n}"));
                        } else {
                            let part = (n - ASSEMBLIES) % PARTS_PER_ASSEMBLY;
                            ui.label(format!("Part {part}"));
                        }
                    },
                );
            });
        frame.ui(|ui| {
            ui.heading(format!("{} parts", self.parts.len()));
            let row_height = ui.style().spacing.interact_height;
            let parts = &mut self.parts;
            let heights: Vec<bool> = parts.iter().map(|p| p.expanded).collect();
            let table = Table::new("parts")
                .column(Column::new("Name").width(170.0).sortable(true))
                .column(Column::new("Material").width(110.0).sortable(true))
                .column(
                    Column::new("Qty")
                        .width(90.0)
                        .align(Align::Max)
                        .sortable(true),
                )
                .column(
                    Column::new("Mass (kg)")
                        .width(100.0)
                        .align(Align::Max)
                        .sortable(true),
                )
                .column(Column::new("Notes").width(320.0))
                .sticky_columns(1)
                .accessible_label("Parts")
                .show_with_heights(
                    ui,
                    parts.len(),
                    |row| {
                        if heights[row] {
                            row_height * 2.5
                        } else {
                            row_height
                        }
                    },
                    &mut self.selected,
                    |ui, row, col| {
                        let part = &mut parts[row];
                        match col {
                            0 => {
                                let arrow = if part.expanded { "▾" } else { "▸" };
                                if ui.add(rustroke::Button::new(arrow).frame(false)).clicked() {
                                    part.expanded = !part.expanded;
                                }
                                if part.expanded {
                                    ui.vertical(|ui| {
                                        ui.label(&part.name);
                                        ui.add(
                                            rustroke::Label::new(format!("ID P-{row:06}"))
                                                .color(ui.style().visuals.weak_text),
                                        );
                                    });
                                } else {
                                    ui.label(&part.name);
                                }
                            }
                            1 => {
                                ui.label(part.material);
                            }
                            2 => {
                                ui.add(DragValue::new(&mut part.quantity).range(0..=9999));
                            }
                            3 => {
                                ui.label(format!("{:.2}", part.mass));
                            }
                            _ => {
                                ui.add(
                                    TextEdit::singleline(&mut part.notes)
                                        .hint_text("Add a note")
                                        .desired_width(300.0),
                                );
                            }
                        }
                    },
                );
            if table.sort_changed
                && let Some(sort) = table.sort
            {
                let selected = self.selected.map(|i| self.parts[i].name.clone());
                self.parts.sort_by(|a, b| {
                    let order = match sort.column {
                        0 => a.name.cmp(&b.name),
                        1 => a.material.cmp(b.material),
                        2 => a.quantity.cmp(&b.quantity),
                        _ => a.mass.total_cmp(&b.mass),
                    };
                    if sort.ascending {
                        order
                    } else {
                        order.reverse()
                    }
                });
                // Keep the same part selected.
                self.selected = selected.and_then(|n| self.parts.iter().position(|p| p.name == n));
            }
            if let Some(row) = table.double_clicked_row {
                self.parts[row].expanded = !self.parts[row].expanded;
            }
            self.status = match self.selected {
                Some(i) => format!(
                    "Selected {} · rows {}–{} laid out",
                    self.parts[i].name, table.visible_rows.start, table.visible_rows.end
                ),
                None => format!(
                    "Rows {}–{} laid out",
                    table.visible_rows.start, table.visible_rows.end
                ),
            };
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    // A cheap pseudo-random sequence, so the example needs no dependency.
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let parts = (0..100_000)
        .map(|i| Part {
            name: format!("Part {:05}", next() % 100_000),
            material: MATERIALS[(next() % 4) as usize],
            quantity: next() % 50,
            mass: (next() % 10_000) as f32 / 100.0,
            notes: if i % 7 == 0 {
                "check tolerance".into()
            } else {
                String::new()
            },
            expanded: false,
        })
        .collect();
    rustroke::run(
        WindowOptions {
            title: "Rustroke — table".into(),
            inner_size: (760.0, 520.0),
            ..Default::default()
        },
        Demo {
            parts,
            selected: None,
            status: String::new(),
            tree_selected: None,
        },
    )
}
