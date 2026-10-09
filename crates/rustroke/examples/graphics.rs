//! Graphics: linear and radial gradients, soft shadows, dashed and dotted
//! lines, Bézier curves, a group of shapes rotated with a transform, text
//! drawn on a sheared face (like a view cube) and an animated image.
//!
//! Run with: `cargo run -p rustroke --example graphics`

use std::f32::consts::TAU;

use rustroke::{
    AnimatedImage, AnimatedTexture, App, Color, ColorImage, Frame, Gradient, Rect, Shadow, Stroke,
    TextStyle, Transform, WindowOptions, point, vec2,
};

struct Demo {
    spinner: Option<AnimatedTexture>,
}

/// Eight frames of a small spinning dot, made in code (a GIF or APNG file
/// loaded with `rustroke::load_animated_image` works the same way).
fn spinner_frames() -> Vec<(ColorImage, std::time::Duration)> {
    (0..8)
        .map(|k| {
            let angle = k as f32 / 8.0 * TAU;
            let (cx, cy) = (16.0 + 10.0 * angle.cos(), 16.0 + 10.0 * angle.sin());
            let image = ColorImage::from_fn([32, 32], |x, y| {
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                let a = (4.0 - d).clamp(0.0, 1.0);
                Color::from_srgb8(250, 179, 135).with_alpha(a)
            });
            (image, std::time::Duration::from_millis(80))
        })
        .collect()
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        let bg = Color::from_srgb8(30, 30, 46);
        let card = Color::from_srgb8(49, 50, 68);
        let text = Color::from_srgb8(205, 214, 244);
        let blue = Color::from_srgb8(137, 180, 250);
        let pink = Color::from_srgb8(245, 194, 231);
        let green = Color::from_srgb8(166, 227, 161);
        frame.clear_color = bg;
        frame.request_repaint = true; // the rotating group animates
        let t = frame.time.as_secs_f32();
        if self.spinner.is_none() {
            self.spinner = Some(AnimatedTexture::new(frame.ctx(), spinner_frames()));
        }
        let title = TextStyle::proportional(16.0).weight(600);
        let label = |frame: &mut Frame, pos, s: &str| {
            frame.text(pos, s, &title, text);
        };

        // Gradients.
        let r = Rect::from_min_size(point(30.0, 50.0), vec2(220.0, 120.0));
        label(frame, point(30.0, 20.0), "Linear and radial gradients");
        let sunset = Gradient::linear(r.left_top(), r.right_top(), blue, pink)
            .with_stop(0.5, Color::from_srgb8(203, 166, 247));
        frame.shapes.rect_gradient(r, 12.0, sunset, Stroke::NONE);
        let c = point(330.0, 110.0);
        let glow =
            Gradient::radial(c, 60.0, Color::WHITE, green.with_alpha(0.0)).with_stop(0.4, green);
        frame.shapes.circle_gradient(c, 60.0, glow, Stroke::NONE);

        // Shadows.
        label(frame, point(430.0, 20.0), "Soft shadows");
        for (i, blur) in [4.0, 12.0, 28.0].into_iter().enumerate() {
            let r = Rect::from_min_size(point(430.0 + i as f32 * 90.0, 60.0), vec2(70.0, 90.0));
            let shadow = Shadow {
                offset: vec2(0.0, blur / 3.0),
                blur,
                spread: 0.0,
                color: Color::new(0.0, 0.0, 0.0, 0.6),
            };
            frame.shapes.shadow(r, 10.0, shadow);
            frame.shapes.rect_filled(r, 10.0, card);
        }

        // Lines and curves.
        label(frame, point(30.0, 200.0), "Dashes, dots and Bézier curves");
        let y = 240.0;
        frame.shapes.dashed_line(
            &[point(30.0, y), point(250.0, y)],
            Stroke::new(2.0, blue),
            10.0,
            6.0,
        );
        frame.shapes.dotted_line(
            &[point(30.0, y + 25.0), point(250.0, y + 25.0)],
            2.0,
            9.0,
            pink,
        );
        frame.shapes.cubic_bezier(
            [
                point(30.0, 360.0),
                point(90.0, 270.0),
                point(190.0, 420.0),
                point(250.0, 300.0),
            ],
            Stroke::new(3.0, green),
        );
        frame.shapes.quadratic_bezier(
            [
                point(280.0, 380.0),
                point(340.0, 260.0),
                point(400.0, 380.0),
            ],
            Stroke::new(2.0, text),
        );

        // A rotating group: shapes and text through one transform.
        label(frame, point(430.0, 200.0), "Transforms");
        let center = point(560.0, 320.0);
        let galley = frame.layout_text("rotated", &TextStyle::proportional(18.0), None);
        frame
            .shapes
            .with_transform(Transform::rotate_around(center, t * 0.8), |s| {
                s.rect(
                    Rect::from_center_size(center, vec2(140.0, 70.0)),
                    10.0,
                    card,
                    Stroke::new(2.0, blue),
                );
                s.galley(center - galley.size / 2.0, galley.clone(), text);
            });

        // Text on a sheared face, as on a view cube.
        label(frame, point(30.0, 440.0), "Text on a sheared face");
        let origin = point(60.0, 525.0);
        let (x_axis, y_axis) = (vec2(1.0, -0.35), vec2(0.45, 0.9));
        let face = [
            origin,
            origin + x_axis * 160.0,
            origin + x_axis * 160.0 + y_axis * 70.0,
            origin + y_axis * 70.0,
        ];
        frame
            .shapes
            .polygon(face.to_vec(), card, Stroke::new(1.5, blue));
        let front = frame.layout_text("FRONT", &TextStyle::proportional(28.0).bold(), None);
        let place = Transform::from_axes(origin + x_axis * 30.0 + y_axis * 18.0, x_axis, y_axis);
        frame.shapes.galley_transformed(front, place, text);

        // An animated image.
        let spinner = self.spinner.as_ref().expect("loaded");
        frame.ui(|ui| {
            ui.add_space(440.0);
            ui.horizontal(|ui| {
                ui.add_space(400.0);
                ui.label("Animated image:");
                ui.add(AnimatedImage::new(spinner).alt_text("Spinner"));
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — graphics".into(),
            inner_size: (760.0, 600.0),
            ..Default::default()
        },
        Demo { spinner: None },
    )
}
