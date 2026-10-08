//! Phase 0 demo: an empty window whose background slowly cycles color.
//! Resize it or move it between screens with different DPI.
//!
//! Run with: `cargo run -p rustroke --example hello`

use rustroke::{App, Color, Frame, WindowOptions};

struct Hello;

impl App for Hello {
    fn update(&mut self, frame: &mut Frame) {
        let t = frame.time.as_secs_f32();
        let wave = |phase: f32| 0.15 + 0.1 * (t + phase).sin();
        frame.clear_color = Color::new(wave(0.0), wave(2.0), wave(4.0), 1.0);
        frame.request_repaint = true; // animate continuously
        log::debug!(
            "frame: {:?} points, scale {:.2}",
            frame.screen_rect.size(),
            frame.pixels_per_point
        );
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "hello".to_owned(),
            ..Default::default()
        },
        Hello,
    )
}
