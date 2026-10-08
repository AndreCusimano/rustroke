//! Phase 8 demo: images and disabled widgets. The UI is also exposed to
//! screen readers: turn on VoiceOver (Cmd+F5) and move through it with
//! VO+arrows; buttons and checkboxes can be activated with VO+Space.
//!
//! Run with: `cargo run -p rustroke --example extras`

use rustroke::{
    App, Color, ColorImage, Frame, Grid, Image, Sense, Slider, TextEdit, TextureHandle,
    WindowOptions, vec2,
};

struct Demo {
    logo: Option<TextureHandle>,
    pattern: Option<TextureHandle>,
    size: f32,
    editing: bool,
    name: String,
    volume: f32,
    notifications: bool,
    clicks: u32,
}

impl Demo {
    /// Textures are created on the first frame (they need the Context) and
    /// kept for the whole life of the app.
    fn load_textures(&mut self, frame: &mut Frame) {
        if self.logo.is_none() {
            let logo = rustroke::load_image(include_bytes!("assets/logo.png")).expect("valid PNG");
            self.logo = Some(frame.ctx().load_texture(logo));
        }
        if self.pattern.is_none() {
            // An image computed in code: a soft checkerboard.
            let pattern = ColorImage::from_fn([96, 96], |x, y| {
                let on = (x / 12 + y / 12) % 2 == 0;
                let t = y as f32 / 96.0;
                let base =
                    Color::from_srgb8(137, 180, 250).lerp(Color::from_srgb8(203, 166, 247), t);
                if on { base } else { base.with_alpha(0.35) }
            });
            self.pattern = Some(frame.ctx().load_texture(pattern));
        }
    }
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        self.load_textures(frame);
        frame.clear_color = frame.ctx().style().visuals.background;
        let (logo, pattern) = (self.logo.clone().unwrap(), self.pattern.clone().unwrap());

        frame.ui(|ui| {
            ui.heading("Images");
            ui.horizontal(|ui| {
                let clicked = ui
                    .add(
                        Image::new(&logo)
                            .size(vec2(self.size, self.size))
                            .sense(Sense::CLICK)
                            .alt_text("Logo"),
                    )
                    .on_hover_text(ui, "A PNG loaded from a file: click it!")
                    .clicked();
                if clicked {
                    self.clicks += 1;
                }
                ui.add(
                    Image::new(&pattern)
                        .size(vec2(self.size, self.size))
                        .alt_text("Checkerboard pattern"),
                );
                ui.add(
                    Image::new(&logo)
                        .size(vec2(self.size, self.size))
                        .tint(Color::from_srgb8(166, 227, 161)),
                );
            });
            ui.add(
                Slider::new(&mut self.size, 32.0..=160.0)
                    .step(1.0)
                    .text("Size"),
            );
            ui.label(format!("Logo clicks: {}", self.clicks));
            ui.separator();

            ui.heading("Disabled widgets");
            ui.checkbox(&mut self.editing, "Editing enabled");
            ui.add_enabled_ui(self.editing, |ui| {
                Grid::new("form").show(ui, |ui| {
                    ui.label("Name");
                    ui.add(TextEdit::singleline(&mut self.name).hint_text("Name"));
                    ui.end_row();
                    ui.label("Volume");
                    ui.add(Slider::new(&mut self.volume, 0.0..=100.0).step(1.0));
                    ui.end_row();
                    ui.label("Notifications");
                    ui.checkbox(&mut self.notifications, "Enabled");
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    ui.button("Save");
                    ui.button("Cancel");
                });
            });
            ui.separator();
            ui.label(
                "Accessibility: with VoiceOver on (Cmd+F5) this window is read aloud: \
                 the role, name and state of every widget.",
            );
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "extras".to_owned(),
            inner_size: (620.0, 640.0),
        },
        Demo {
            logo: None,
            pattern: None,
            size: 96.0,
            editing: false,
            name: "André".to_owned(),
            volume: 60.0,
            notifications: true,
            clicks: 0,
        },
    )
}
