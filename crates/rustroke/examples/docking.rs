//! Documents in a tab bar and dockable panels: drag a panel's tab into
//! another group, or onto a side of a group to split it; drag the lines
//! between groups to resize them; right-click a tab for its menu.
//!
//! Run with: `cargo run -p rustroke --example docking`

use rustroke::{
    App, DockArea, DockState, DockViewer, Frame, Panel, SplitAxis, TabBar, TabLabel, Ui,
    WindowOptions,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pane {
    Viewport,
    Features,
    Properties,
    Log,
    Notes(u32),
}

struct Viewer<'a> {
    log: &'a mut Vec<String>,
    next_note: &'a mut u32,
}

impl DockViewer for Viewer<'_> {
    type Tab = Pane;

    fn label(&mut self, tab: &Pane) -> TabLabel {
        match tab {
            Pane::Notes(n) => TabLabel::new(format!("Notes {n}")),
            other => TabLabel::new(format!("{other:?}")),
        }
    }

    fn ui(&mut self, ui: &mut Ui<'_>, tab: &mut Pane) {
        match tab {
            Pane::Viewport => {
                ui.label("The 3D view would be here.");
            }
            Pane::Features => {
                for name in ["Base plate", "Shaft", "Holes"] {
                    ui.label(name);
                }
            }
            Pane::Properties => {
                ui.label("Length: 42.5 mm");
                ui.label("Material: aluminium");
            }
            Pane::Log => {
                for line in self.log.iter().rev().take(10) {
                    ui.label(line);
                }
            }
            Pane::Notes(n) => {
                ui.label(format!("Notes page {n}"));
            }
        }
    }

    fn closable(&mut self, tab: &Pane) -> bool {
        *tab != Pane::Viewport
    }

    fn on_close(&mut self, tab: &mut Pane) -> bool {
        self.log.push(format!("Closed {tab:?}"));
        true
    }

    fn context_menu(&mut self, ui: &mut Ui<'_>, tab: &mut Pane) {
        if ui.button("Log this tab").clicked() {
            self.log.push(format!("Context menu of {tab:?}"));
        }
    }

    fn add_tab(&mut self) -> Option<Pane> {
        *self.next_note += 1;
        Some(Pane::Notes(*self.next_note))
    }
}

struct Demo {
    documents: Vec<(String, bool)>,
    active_document: usize,
    dock: DockState<Pane>,
    log: Vec<String>,
    next_note: u32,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        Panel::top("documents").show(frame, |ui| {
            let bar = TabBar::new("documents").add_button(true).show(
                ui,
                &mut self.documents,
                &mut self.active_document,
                |(name, dirty)| TabLabel::new(name.as_str()).modified(*dirty),
            );
            if let Some(i) = bar.close_requested {
                self.log
                    .push(format!("Closed document {}", self.documents[i].0));
                self.documents.remove(i);
                self.active_document = self
                    .active_document
                    .min(self.documents.len().saturating_sub(1));
            }
            if bar.add_clicked {
                self.documents
                    .push((format!("Part {}", self.documents.len() + 1), false));
                self.active_document = self.documents.len() - 1;
            }
        });
        let mut viewer = Viewer {
            log: &mut self.log,
            next_note: &mut self.next_note,
        };
        frame.ui(|ui| {
            DockArea::new("panels")
                .add_button(true)
                .show(ui, &mut self.dock, &mut viewer);
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    let mut dock = DockState::new(vec![Pane::Viewport]);
    let center = dock.groups()[0];
    let left = dock
        .split(
            center,
            SplitAxis::Horizontal,
            true,
            0.22,
            vec![Pane::Features],
        )
        .expect("group exists");
    dock.split(
        center,
        SplitAxis::Horizontal,
        false,
        0.3,
        vec![Pane::Properties],
    );
    dock.split(left, SplitAxis::Vertical, false, 0.35, vec![Pane::Log]);
    rustroke::run(
        WindowOptions {
            title: "Rustroke — docking".into(),
            inner_size: (1000.0, 640.0),
            ..Default::default()
        },
        Demo {
            documents: vec![
                ("Motor mount".into(), false),
                ("Base plate".into(), true),
                ("Assembly".into(), false),
            ],
            active_document: 0,
            dock,
            log: vec!["Started".into()],
            next_note: 0,
        },
    )
}
