//! Phase 1 demo: the 2D renderer. Rectangles, rounded corners, circles,
//! lines, polygons, transparency and clipping, plus a little animation.
//! Resize the window: the layout follows the window size.
//!
//! Run with: `cargo run -p rustroke --example shapes`

use std::f32::consts::TAU;

use rustroke::{App, Color, Frame, Rect, Stroke, WindowOptions, point, vec2};

struct Shapes;

impl App for Shapes {
    fn update(&mut self, frame: &mut Frame) {
        let bg = Color::from_srgb8(30, 30, 46);
        let surface = Color::from_srgb8(49, 50, 68);
        let text = Color::from_srgb8(205, 214, 244);
        let blue = Color::from_srgb8(137, 180, 250);
        let red = Color::from_srgb8(243, 139, 168);
        let green = Color::from_srgb8(166, 227, 161);
        let yellow = Color::from_srgb8(249, 226, 175);

        frame.clear_color = bg;
        frame.request_repaint = true; // animate continuously
        let t = frame.time.as_secs_f32();
        let screen = frame.screen_rect;
        let s = &mut frame.shapes;

        // A card per demo, in a grid that adapts to the window width.
        let card_size = vec2(220.0, 160.0);
        let gap = 16.0;
        let columns = ((screen.width() - gap) / (card_size.x + gap))
            .floor()
            .max(1.0) as usize;
        let card = |i: usize| {
            let (col, row) = ((i % columns) as f32, (i / columns) as f32);
            Rect::from_min_size(
                point(
                    gap + col * (card_size.x + gap),
                    gap + row * (card_size.y + gap),
                ),
                card_size,
            )
        };
        for i in 0..6 {
            s.rect(
                card(i),
                12.0,
                surface,
                Stroke::new(1.0, text.with_alpha(0.1)),
            );
        }

        // 1. Rectangles with increasing corner radius.
        let c = card(0);
        for (i, radius) in [0.0, 6.0, 14.0, 30.0].into_iter().enumerate() {
            let r =
                Rect::from_min_size(c.min + vec2(16.0 + i as f32 * 50.0, 50.0), vec2(40.0, 60.0));
            s.rect_filled(r, radius, blue);
        }

        // 2. Outlines of different widths.
        let c = card(1);
        for (i, width) in [0.5, 1.0, 2.0, 4.0].into_iter().enumerate() {
            let r =
                Rect::from_center_size(c.center(), vec2(40.0, 30.0) + vec2(40.0, 30.0) * i as f32);
            s.rect_stroke(r, 8.0, Stroke::new(width, green));
        }

        // 3. Overlapping translucent circles.
        let c = card(2);
        for (i, color) in [red, green, blue].into_iter().enumerate() {
            let angle = t * 0.8 + i as f32 * TAU / 3.0;
            let center = c.center() + vec2(angle.cos(), angle.sin()) * 22.0;
            s.circle_filled(center, 40.0, color.with_alpha(0.55));
        }

        // 4. A spinner made of lines with fading alpha.
        let c = card(3);
        for i in 0..12 {
            let angle = i as f32 * TAU / 12.0;
            let dir = vec2(angle.cos(), angle.sin());
            let phase = ((i as f32 / 12.0) - t * 1.2).rem_euclid(1.0);
            s.line(
                c.center() + dir * 20.0,
                c.center() + dir * 50.0,
                Stroke::new(5.0, yellow.with_alpha(0.15 + 0.85 * phase)),
            );
        }

        // 5. A rotating polygon and an animated polyline (a sine wave).
        let c = card(4);
        let hexagon = (0..6)
            .map(|i| {
                let angle = t * 0.5 + i as f32 * TAU / 6.0;
                point(
                    c.min.x + 60.0 + 40.0 * angle.cos(),
                    c.center().y + 40.0 * angle.sin(),
                )
            })
            .collect();
        s.polygon(hexagon, red.with_alpha(0.8), Stroke::new(2.0, text));
        let wave = (0..=40)
            .map(|i| {
                let x = i as f32 / 40.0;
                point(
                    c.min.x + 120.0 + x * 85.0,
                    c.center().y + 25.0 * (x * TAU * 1.5 - t * 3.0).sin(),
                )
            })
            .collect();
        s.polyline(wave, Stroke::new(2.5, blue));

        // 6. Clipping: content moving inside a window that cuts it off.
        let c = card(5);
        let viewport = c.expand(-24.0);
        s.rect_stroke(viewport, 0.0, Stroke::new(1.0, text.with_alpha(0.4)));
        s.with_clip(viewport, |s| {
            for i in 0..8 {
                let x = viewport.min.x + ((i as f32 * 40.0 + t * 40.0) % 320.0) - 60.0;
                let r = Rect::from_min_size(
                    point(x, viewport.min.y + 15.0 + (i % 3) as f32 * 30.0),
                    vec2(50.0, 22.0),
                );
                s.rect_filled(r, 11.0, [blue, green, red][i % 3]);
            }
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "shapes".to_owned(),
            inner_size: (950.0, 400.0),
        },
        Shapes,
    )
}
