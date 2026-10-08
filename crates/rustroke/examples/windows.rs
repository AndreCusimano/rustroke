//! More than one native window: the main window controls an "assembly"
//! window (e.g. for a second monitor) that the app keeps open by asking
//! for it every frame. Closing it with the window's close button clears
//! the checkbox; both windows share the app's state.
//!
//! Run with: `cargo run -p rustroke --example windows`

use rustroke::{App, Frame, Id, Slider, WindowOptions};

struct Demo {
    show_assembly: bool,
    explode: f32,
    parts: u32,
}

const ASSEMBLY: &str = "assembly";

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        if self.show_assembly {
            frame.show_window(
                Id::new(ASSEMBLY),
                WindowOptions {
                    title: "Assembly view".into(),
                    inner_size: (420.0, 300.0),
                },
            );
        }
        frame.ui(|ui| {
            ui.heading("Main window");
            ui.checkbox(&mut self.show_assembly, "Show the assembly window");
            ui.add(Slider::new(&mut self.explode, 0.0..=1.0).text("Explode"));
            if ui.button("Add part").clicked() {
                self.parts += 1;
            }
        });
    }

    fn update_window(&mut self, id: Id, frame: &mut Frame) {
        if id != Id::new(ASSEMBLY) {
            return;
        }
        frame.clear_color = frame.ctx().style().visuals.background;
        frame.ui(|ui| {
            ui.heading("Assembly");
            ui.label(format!(
                "{} parts, exploded {:.0}%",
                self.parts,
                self.explode * 100.0
            ));
            // Changes here are seen by the main window too.
            ui.add(Slider::new(&mut self.explode, 0.0..=1.0).text("Explode"));
        });
    }

    fn on_window_close_requested(&mut self, _id: Id) -> bool {
        self.show_assembly = false;
        true
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — windows".into(),
            inner_size: (520.0, 320.0),
        },
        Demo {
            show_assembly: true,
            explode: 0.25,
            parts: 3,
        },
    )
}
