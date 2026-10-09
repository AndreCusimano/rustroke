//! Icons, lists and scrolling: a toolbar of two-tone SVG icons (lines take
//! the text color, the blue parts the theme's accent), a feature list you
//! can select (Cmd/Ctrl+click, Shift+click, arrows) and reorder by
//! dragging, filtered by a search field, with eye toggles, a side panel
//! as wide as its content, and a feature timeline with a rollback marker
//! to drag between the steps (drawn with the painter, tooltips on free
//! areas).
//!
//! Run with: `cargo run -p rustroke --example lists`

use rustroke::{
    App, Button, CentralPanel, CursorIcon, Frame, IconId, IconToggle, List, Panel, Rect,
    ScrollArea, SearchField, Sense, Stroke, Style, WindowOptions, point, vec2,
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
const EYE_CLOSED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M2 12 C6 5 18 5 22 12 C18 19 6 19 2 12 Z" fill="none" stroke="#000" stroke-width="1.6"/>
  <path d="M4 20 L20 4" stroke="#000" stroke-width="1.6" stroke-linecap="round"/>
</svg>"##;

struct Icons {
    extrude: IconId,
    revolve: IconId,
    hole: IconId,
    eye: IconId,
    eye_closed: IconId,
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
    filter: String,
    /// Features after the rollback marker are not computed.
    rollback: usize,
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

impl Demo {
    /// A feature timeline: one icon per feature on a line, each with a
    /// tooltip, and a marker that can be dragged between them (features
    /// after it are rolled back). Built from `ui.interact` and the painter.
    fn timeline(&mut self, ui: &mut rustroke::Ui) {
        const STEP: f32 = 34.0;
        let count = self.features.len();
        let height = 36.0;
        let width = STEP * (count as f32 + 1.0);
        let area = ui.allocate_rect(vec2(width, height));
        let visuals = ui.style().visuals.clone();
        let y = area.center().y;
        let x_of = |slot: usize| area.min.x + STEP * (slot as f32 + 0.5);
        ui.painter().line(
            point(area.min.x, y),
            point(area.max.x, y),
            Stroke::new(1.0, visuals.window_stroke.color),
        );
        for (i, feature) in self.features.iter().enumerate() {
            let center = point(x_of(i) + STEP / 2.0, y);
            let rect = Rect::from_center_size(center, vec2(26.0, 26.0));
            let id = ui.id().with(("step", i));
            let response = ui.interact(id, rect, Sense::CLICK);
            let rolled_back = i >= self.rollback;
            let fill = if self.selection.contains(&i) {
                visuals.selection
            } else {
                visuals.panel_fill
            };
            ui.painter().rect_filled(rect, 6.0, fill);
            if let Some(icon) = ui.rasterize_icon(self.icon(feature.kind), 18.0) {
                let color = if rolled_back {
                    visuals.weak_text
                } else {
                    visuals.text
                };
                let pos = center - icon.size / 2.0;
                ui.paint_icon(pos, &icon, color);
            }
            if response.clicked() {
                self.selection = vec![i];
            }
            response.on_hover_text(ui, feature.name.clone());
        }
        // The marker sits between two steps; dragging snaps to the gaps.
        let marker_x = x_of(self.rollback);
        let marker = Rect::from_center_size(point(marker_x, y), vec2(10.0, height));
        let id = ui.id().with("rollback");
        let response = ui.interact(id, marker, Sense::DRAG);
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor(CursorIcon::ResizeHorizontal);
        }
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged())
        {
            let slot = ((pos.x - area.min.x) / STEP).round() as usize;
            self.rollback = slot.min(count);
        }
        let color = if response.dragged() {
            visuals.accent
        } else {
            visuals.warning
        };
        ui.painter().line(
            point(marker_x, area.min.y + 2.0),
            point(marker_x, area.max.y - 2.0),
            Stroke::new(3.0, color),
        );
        response.on_hover_text(ui, "Rollback: drag to roll features back");
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
                eye_closed: fonts.add_svg_icon(EYE_CLOSED).expect("valid SVG"),
            });
        }
        let style = if self.dark {
            Style::dark()
        } else {
            Style::light()
        };
        frame.ctx().set_style(style);
        frame.clear_color = frame.ctx().style().visuals.background;
        let icons = self.icons.as_ref().expect("loaded");
        let (eye, eye_closed) = (icons.eye, icons.eye_closed);

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
            ScrollArea::horizontal().show(ui, |ui| self.timeline(ui));
        });

        Panel::left("features").auto_width().show(frame, |ui| {
            ui.heading("Features");
            ui.add(SearchField::new(&mut self.filter).desired_width(180.0));
            let filter = self.filter.to_lowercase();
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
                        // Rows that don't match the search are dimmed.
                        let matches = feature.name.to_lowercase().contains(&filter);
                        let color = if matches {
                            ui.style().visuals.text
                        } else {
                            ui.style().visuals.weak_text.with_alpha(0.5)
                        };
                        ui.add(rustroke::Label::new(&feature.name).color(color));
                        ui.add(IconToggle::new(
                            &mut feature.visible,
                            eye,
                            eye_closed,
                            "Visible",
                        ));
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
            ..Default::default()
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
            filter: String::new(),
            rollback: 4,
        },
    )
}
