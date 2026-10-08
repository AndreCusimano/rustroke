//! Icons, lists and scrolling: a toolbar of two-tone SVG icons (lines take
//! the text color, the blue parts the theme's accent), a feature list you
//! can select (Cmd/Ctrl+click, Shift+click, arrows) and reorder by
//! dragging, a side panel as wide as its content, and a timeline that
//! scrolls sideways.
//!
//! Run with: `cargo run -p rustroke --example lists`

use rustroke::{
    App, Button, CentralPanel, Frame, IconId, List, Panel, ScrollArea, Style, WindowOptions,
};

// Small icons drawn for this example: black = line, #1E6FFF = accent.
const EXTRUDE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M4 16 L12 20 L20 16 L12 12 Z" fill="none" stroke="#000" stroke-width="1.6" stroke-linejoin="round"/>
  <path d="M12 3 V10 M9 6 L12 3 L15 6" fill="none" stroke="#1E6FFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##;
const REVOLVE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <ellipse cx="12" cy="12" rx="8" ry="4" fill="none" stroke="#000" stroke-width="1.6"/>
  <path d="M12 3 V21" stroke="#1E6FFF" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##;
const HOLE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3" y="5" width="18" height="14" rx="2" fill="none" stroke="#000" stroke-width="1.6"/>
  <circle cx="12" cy="12" r="3.5" fill="#1E6FFF"/>
</svg>"##;
const EYE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M2 12 C6 5 18 5 22 12 C18 19 6 19 2 12 Z" fill="none" stroke="#000" stroke-width="1.6"/>
  <circle cx="12" cy="12" r="3" fill="#000"/>
</svg>"##;

struct Icons {
    extrude: IconId,
    revolve: IconId,
    hole: IconId,
    eye: IconId,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Extrude,
    Revolve,
    Hole,
}

struct Feature {
    name: String,
    kind: Kind,
    visible: bool,
}

struct Demo {
    icons: Option<Icons>,
    tool: Kind,
    features: Vec<Feature>,
    selection: Vec<usize>,
    dark: bool,
    status: String,
}

impl Demo {
    fn icon(&self, kind: Kind) -> IconId {
        let icons = self.icons.as_ref().expect("loaded");
        match kind {
            Kind::Extrude => icons.extrude,
            Kind::Revolve => icons.revolve,
            Kind::Hole => icons.hole,
        }
    }
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        if self.icons.is_none() {
            let fonts = frame.fonts();
            self.icons = Some(Icons {
                extrude: fonts.add_svg_icon(EXTRUDE).expect("valid SVG"),
                revolve: fonts.add_svg_icon(REVOLVE).expect("valid SVG"),
                hole: fonts.add_svg_icon(HOLE).expect("valid SVG"),
                eye: fonts.add_svg_icon(EYE).expect("valid SVG"),
            });
        }
        let style = if self.dark {
            Style::dark()
        } else {
            Style::light()
        };
        frame.ctx().set_style(style);
        frame.clear_color = frame.ctx().style().visuals.background;
        let eye = self.icons.as_ref().expect("loaded").eye;

        Panel::top("toolbar").show(frame, |ui| {
            ui.horizontal(|ui| {
                for (kind, name) in [
                    (Kind::Extrude, "Extrude"),
                    (Kind::Revolve, "Revolve"),
                    (Kind::Hole, "Hole"),
                ] {
                    let clicked = ui
                        .add(
                            Button::icon_only(self.icon(kind))
                                .icon_size(22.0)
                                .frame(false)
                                .selected(self.tool == kind)
                                .accessible_label(name),
                        )
                        .on_hover_text(ui, name)
                        .clicked();
                    if clicked {
                        self.tool = kind;
                        self.features.push(Feature {
                            name: format!("{name} {}", self.features.len() + 1),
                            kind,
                            visible: true,
                        });
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.dark, "Dark theme");
            });
        });

        Panel::bottom("timeline").show(frame, |ui| {
            ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for i in 0..40 {
                        let kind = [Kind::Extrude, Kind::Revolve, Kind::Hole][i % 3];
                        ui.add(Button::icon_only(self.icon(kind)).frame(false))
                            .on_hover_text(ui, format!("Step {}", i + 1));
                    }
                });
            });
        });

        Panel::left("features").auto_width().show(frame, |ui| {
            ui.heading("Features");
            let icons: Vec<IconId> = self.features.iter().map(|f| self.icon(f.kind)).collect();
            let list = List::new("features")
                .multi_select(true)
                .reorderable(true)
                .accessible_label("Features")
                .show(
                    ui,
                    &mut self.features,
                    &mut self.selection,
                    |ui, i, feature| {
                        ui.icon(icons[i]);
                        ui.label(&feature.name);
                        let eye_button = Button::icon_only(eye)
                            .frame(false)
                            .selected(!feature.visible)
                            .accessible_label("Hide");
                        if ui.add(eye_button).clicked() {
                            feature.visible = !feature.visible;
                        }
                    },
                );
            if let Some((from, to)) = list.moved {
                self.status = format!("Moved feature {} to position {}", from + 1, to + 1);
            }
            let mut delete = None;
            for (i, row) in list.rows.iter().enumerate() {
                row.context_menu(ui, |ui| {
                    if ui.button("Delete").clicked() {
                        delete = Some(i);
                    }
                });
            }
            if let Some(i) = delete {
                self.features.remove(i);
                self.selection.clear();
            }
        });

        CentralPanel.show(frame, |ui| {
            ui.label(format!(
                "{} selected · {}",
                self.selection.len(),
                self.status
            ));
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    let feature = |name: &str, kind| Feature {
        name: name.into(),
        kind,
        visible: true,
    };
    rustroke::run(
        WindowOptions {
            title: "Rustroke — lists and icons".into(),
            inner_size: (820.0, 520.0),
        },
        Demo {
            icons: None,
            tool: Kind::Extrude,
            features: vec![
                feature("Base plate", Kind::Extrude),
                feature("Shaft", Kind::Revolve),
                feature("Mounting holes", Kind::Hole),
                feature("Boss", Kind::Extrude),
            ],
            selection: vec![1],
            dark: true,
            status: "drag a row to reorder".into(),
        },
    )
}
