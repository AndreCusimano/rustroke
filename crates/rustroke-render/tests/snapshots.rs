//! Snapshot tests: render a scene offscreen on the GPU and compare it with a
//! reference PNG in `tests/snapshots/`.
//!
//! - Missing references are created on the first run.
//! - Set `UPDATE_SNAPSHOTS=1` to overwrite references after an intended change.
//! - On mismatch, the actual image is written next to the reference as
//!   `<name>.new.png` for inspection.
//! - Without a usable GPU adapter the tests are skipped (with a message).

use std::path::PathBuf;

use rustroke_core::{
    Color, DisplayList, PhysicalSize, Rect, Stroke, Tessellator, TextureAtlas, point, vec2,
};
use rustroke_render::{OffscreenRenderer, PaintJob};
use rustroke_text::{Fonts, TextStyle};

/// Max per-channel difference tolerated, to absorb GPU rounding differences.
const CHANNEL_TOLERANCE: u8 = 3;
/// Fraction of pixels allowed to exceed the tolerance.
const MAX_BAD_PIXEL_RATIO: f64 = 0.001;

fn shapes_scene() -> DisplayList {
    let blue = Color::from_srgb8(137, 180, 250);
    let red = Color::from_srgb8(243, 139, 168);
    let green = Color::from_srgb8(166, 227, 161);
    let text = Color::from_srgb8(205, 214, 244);

    let mut list = DisplayList::new();
    // Plain, rounded and outlined rectangles.
    list.rect_filled(
        Rect::from_min_size(point(10.0, 10.0), vec2(50.0, 30.0)),
        0.0,
        blue,
    );
    list.rect_filled(
        Rect::from_min_size(point(70.0, 10.0), vec2(50.0, 30.0)),
        8.0,
        green,
    );
    list.rect(
        Rect::from_min_size(point(130.0, 10.0), vec2(50.0, 30.0)),
        6.0,
        Color::TRANSPARENT,
        Stroke::new(2.0, red),
    );
    // Circles: filled, outlined, semi-transparent overlap.
    list.circle(point(35.0, 75.0), 20.0, blue, Stroke::new(3.0, text));
    list.circle_filled(point(80.0, 75.0), 20.0, red.with_alpha(0.6));
    list.circle_filled(point(100.0, 75.0), 20.0, green.with_alpha(0.6));
    // Lines of varying width, including a sub-pixel hairline.
    for (i, width) in [0.5, 1.0, 2.0, 4.0].into_iter().enumerate() {
        let y = 105.0 + i as f32 * 12.0;
        list.line(
            point(10.0, y),
            point(120.0, y + 6.0),
            Stroke::new(width, text),
        );
    }
    // A convex polygon and a polyline.
    list.polygon(
        vec![
            point(140.0, 60.0),
            point(180.0, 70.0),
            point(170.0, 100.0),
            point(135.0, 90.0),
        ],
        blue.with_alpha(0.8),
        Stroke::new(1.0, text),
    );
    list.polyline(
        vec![
            point(135.0, 140.0),
            point(150.0, 115.0),
            point(165.0, 140.0),
            point(180.0, 115.0),
        ],
        Stroke::new(2.5, green),
    );
    // Clipping: a big circle cut by a rectangular clip.
    let clip = Rect::from_min_size(point(195.0, 10.0), vec2(50.0, 60.0));
    list.rect_stroke(clip, 0.0, Stroke::new(1.0, text.with_alpha(0.5)));
    list.with_clip(clip, |list| {
        list.circle_filled(point(245.0, 70.0), 45.0, red)
    });
    list
}

/// Text in several sizes, weights, colors and a wrapped paragraph, using
/// only the bundled font so the output is identical on every machine.
fn text_scene(fonts: &mut Fonts, pixels_per_point: f32) -> DisplayList {
    let text = Color::from_srgb8(205, 214, 244);
    let blue = Color::from_srgb8(137, 180, 250);
    let dark = Color::from_srgb8(30, 30, 46);
    let mut list = DisplayList::new();
    let mut y = 8.0;
    for size in [10.0, 13.0, 16.0, 24.0] {
        let galley = fonts.layout(
            &format!("Inter {size}pt — Perché è così? 0123"),
            &TextStyle::proportional(size),
            None,
            pixels_per_point,
        );
        let height = galley.size.y;
        list.galley(point(8.0, y), galley, text);
        y += height;
    }
    let bold = fonts.layout(
        "Bold text",
        &TextStyle::proportional(16.0).bold(),
        None,
        pixels_per_point,
    );
    list.galley(point(8.0, y + 4.0), bold, blue);

    // Dark text on a light button-like rectangle, centered by measuring.
    let button = Rect::from_min_size(point(110.0, y + 2.0), vec2(90.0, 26.0));
    list.rect_filled(button, 6.0, blue);
    let label = fonts.layout(
        "Measured",
        &TextStyle::proportional(14.0),
        None,
        pixels_per_point,
    );
    let pos = button.center() - label.size / 2.0;
    list.galley(pos, label, dark);

    let paragraph = fonts.layout(
        "A wrapped paragraph: lines break at word boundaries when they reach the wrap width.",
        &TextStyle::proportional(12.0),
        Some(180.0),
        pixels_per_point,
    );
    list.galley(point(8.0, y + 36.0), paragraph, text.with_alpha(0.7));
    list
}

fn render(list: &DisplayList, size: PhysicalSize, pixels_per_point: f32) -> Option<Vec<u8>> {
    render_with_atlas(list, size, pixels_per_point, &mut TextureAtlas::new(64))
}

fn render_with_atlas(
    list: &DisplayList,
    size: PhysicalSize,
    pixels_per_point: f32,
    atlas: &mut TextureAtlas,
) -> Option<Vec<u8>> {
    render_with_clear(
        list,
        size,
        pixels_per_point,
        atlas,
        Color::from_srgb8(30, 30, 46),
    )
}

fn render_with_clear(
    list: &DisplayList,
    size: PhysicalSize,
    pixels_per_point: f32,
    atlas: &mut TextureAtlas,
    clear_color: Color,
) -> Option<Vec<u8>> {
    render_full(
        list,
        size,
        pixels_per_point,
        atlas,
        clear_color,
        &rustroke_core::TexturesDelta::default(),
    )
}

fn render_full(
    list: &DisplayList,
    size: PhysicalSize,
    pixels_per_point: f32,
    atlas: &mut TextureAtlas,
    clear_color: Color,
    textures: &rustroke_core::TexturesDelta,
) -> Option<Vec<u8>> {
    let mut renderer = match pollster::block_on(OffscreenRenderer::new(atlas)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping snapshot test: {e}");
            return None;
        }
    };
    let meshes = Tessellator::new(pixels_per_point, atlas).tessellate(list);
    let job = PaintJob {
        meshes: &meshes,
        textures,
        pixels_per_point,
        clear_color,
    };
    Some(renderer.render(size, &job, atlas))
}

fn check_snapshot(name: &str, size: PhysicalSize, pixels: &[u8]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let path = dir.join(format!("{name}.png"));
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();

    if update || !path.exists() {
        std::fs::create_dir_all(&dir).unwrap();
        write_png(&path, size, pixels);
        eprintln!("wrote snapshot {}", path.display());
        return;
    }

    let (ref_size, expected) = read_png(&path);
    assert_eq!(ref_size, size, "snapshot {name}: size changed");
    let bad = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.as_chunks::<4>().0)
        .filter(|(a, b)| {
            a.iter()
                .zip(*b)
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    let ratio = bad as f64 / f64::from(size.width * size.height);
    if ratio > MAX_BAD_PIXEL_RATIO {
        let new_path = dir.join(format!("{name}.new.png"));
        write_png(&new_path, size, pixels);
        panic!(
            "snapshot {name}: {bad} pixels differ ({:.3}%). Actual image: {}. \
             If the change is intended, rerun with UPDATE_SNAPSHOTS=1.",
            ratio * 100.0,
            new_path.display()
        );
    }
}

fn write_png(path: &std::path::Path, size: PhysicalSize, pixels: &[u8]) {
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut encoder = png::Encoder::new(file, size.width, size.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
}

fn read_png(path: &std::path::Path) -> (PhysicalSize, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        info.color_type,
        png::ColorType::Rgba,
        "reference must be RGBA8"
    );
    buf.truncate(info.buffer_size());
    (PhysicalSize::new(info.width, info.height), buf)
}

#[test]
fn shapes_1x() {
    let size = PhysicalSize::new(256, 160);
    if let Some(pixels) = render(&shapes_scene(), size, 1.0) {
        check_snapshot("shapes_1x", size, &pixels);
    }
}

#[test]
fn shapes_2x() {
    // Same scene in logical points, rendered at double density (HiDPI).
    let size = PhysicalSize::new(512, 320);
    if let Some(pixels) = render(&shapes_scene(), size, 2.0) {
        check_snapshot("shapes_2x", size, &pixels);
    }
}

#[test]
fn text_1x() {
    let mut fonts = Fonts::bundled_only();
    let list = text_scene(&mut fonts, 1.0);
    let size = PhysicalSize::new(400, 190);
    if let Some(pixels) = render_with_atlas(&list, size, 1.0, fonts.atlas_mut()) {
        check_snapshot("text_1x", size, &pixels);
    }
}

#[test]
fn text_2x() {
    let mut fonts = Fonts::bundled_only();
    let list = text_scene(&mut fonts, 2.0);
    let size = PhysicalSize::new(800, 380);
    if let Some(pixels) = render_with_atlas(&list, size, 2.0, fonts.atlas_mut()) {
        check_snapshot("text_2x", size, &pixels);
    }
}

/// Widgets in various states, driven by simulated input: a hovered
/// button, a keyboard-focused button (focus ring), checked/unchecked boxes,
/// radio buttons and sliders.
fn widgets_scene(
    fonts: &mut Fonts,
    pixels_per_point: f32,
    style: rustroke_widgets::Style,
) -> DisplayList {
    use rustroke_core::{Event, Key, Modifiers, Point, RawInput};
    use rustroke_widgets::{Context, Slider, Ui};

    let screen = Rect::from_min_size(Point::ZERO, vec2(300.0, 390.0));
    let mut ctx = Context::new();
    ctx.set_style(style);
    let mut list = DisplayList::new();
    let (mut checked, mut unchecked, mut choice, mut value, mut amount) =
        (true, false, 1, 0.35_f32, 7_u8);
    let mut hover_target = Point::ZERO;
    let mut add = |ui: &mut Ui<'_>, hover_target: &mut Point| {
        ui.heading("Widgets");
        ui.button("Focused");
        *hover_target = ui.button("Hovered").rect.center();
        ui.button("Normal");
        ui.checkbox(&mut checked, "Checked");
        ui.checkbox(&mut unchecked, "Unchecked");
        ui.radio_value(&mut choice, 0, "Option A");
        ui.radio_value(&mut choice, 1, "Option B");
        ui.add(Slider::new(&mut value, 0.0..=1.0).text("Float"));
        ui.add(Slider::new(&mut amount, 0..=10).text("Integer"));
    };
    // Each frame is a second later, so hover animations have finished.
    let mut time = 0.0;
    let mut frame = |ctx: &mut Context, list: &mut DisplayList, events, hover: &mut Point| {
        time += 1.0;
        ctx.begin_frame(RawInput {
            time,
            screen_rect: screen,
            pixels_per_point,
            events,
        });
        ctx.ui(screen.expand(-12.0), fonts, |ui| add(ui, hover));
        let mut output = ctx.end_frame();
        *list = std::mem::take(&mut output.shapes);
    };
    frame(&mut ctx, &mut list, vec![], &mut hover_target);
    let tab = Event::Key {
        key: Key::Tab,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let events = vec![tab, Event::PointerMoved(hover_target)];
    frame(&mut ctx, &mut list, events, &mut hover_target);
    frame(&mut ctx, &mut list, vec![], &mut hover_target);
    list
}

#[test]
fn widgets_2x() {
    use rustroke_widgets::Style;
    for (name, style) in [
        ("widgets_2x", Style::dark()),
        ("widgets_light_2x", Style::light()),
    ] {
        let clear = style.visuals.background;
        let mut fonts = Fonts::bundled_only();
        let list = widgets_scene(&mut fonts, 2.0, style);
        let size = PhysicalSize::new(600, 780);
        if let Some(pixels) = render_with_clear(&list, size, 2.0, fonts.atlas_mut(), clear) {
            check_snapshot(name, size, &pixels);
        }
    }
}

/// Panels, a floating window, an open menu and a tooltip, driven by
/// simulated input over several frames.
fn containers_scene(
    fonts: &mut Fonts,
    pixels_per_point: f32,
    style: rustroke_widgets::Style,
) -> DisplayList {
    use rustroke_core::{Event, Modifiers, Point, PointerButton, RawInput};
    use rustroke_widgets::{CentralPanel, Context, Panel, Window};

    let screen = Rect::from_min_size(Point::ZERO, vec2(420.0, 300.0));
    let mut ctx = Context::new();
    ctx.set_style(style);
    let mut list = DisplayList::new();
    let mut menu_rect = Rect::NOTHING;
    let mut button_rect = Rect::NOTHING;
    let mut time = 0.0;
    let mut frame = |events: Vec<Event>, dt: f64, menu_rect: &mut Rect, button_rect: &mut Rect| {
        time += dt;
        ctx.begin_frame(RawInput {
            time,
            screen_rect: screen,
            pixels_per_point,
            events,
        });
        let mut root = (&mut ctx, &mut *fonts);
        Panel::top("menu").show(&mut root, |ui| {
            ui.horizontal(|ui| {
                *menu_rect = ui
                    .menu_button("File", |ui| {
                        ui.button("New");
                        ui.button("Open…");
                        ui.separator();
                        ui.button("Quit");
                    })
                    .response
                    .rect;
                ui.menu_button("View", |_| {});
            });
        });
        Panel::left("side")
            .default_size(110.0)
            .show(&mut root, |ui| {
                ui.label("Side panel");
            });
        CentralPanel.show(&mut root, |ui| {
            ui.add_space(170.0);
            *button_rect = ui.button("Hover me").on_hover_text(ui, "A tooltip").rect;
        });
        Window::new("Window")
            .default_pos(point(220.0, 70.0))
            .default_width(190.0)
            .show(&mut root, |ui| {
                ui.label("Floating window");
            });
        let mut output = ctx.end_frame();
        list = std::mem::take(&mut output.shapes);
    };
    let click = |pos: Point| {
        let b = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        vec![b(true), b(false)]
    };
    frame(vec![], 0.1, &mut menu_rect, &mut button_rect);
    frame(vec![], 0.1, &mut menu_rect, &mut button_rect);
    frame(
        click(menu_rect.center()),
        0.1,
        &mut menu_rect,
        &mut button_rect,
    );
    frame(
        vec![Event::PointerMoved(button_rect.center())],
        0.1,
        &mut menu_rect,
        &mut button_rect,
    );
    for _ in 0..3 {
        frame(vec![], 0.4, &mut menu_rect, &mut button_rect);
    }
    list
}

#[test]
fn containers_2x() {
    use rustroke_widgets::Style;
    for (name, style) in [
        ("containers_2x", Style::dark()),
        ("containers_light_2x", Style::light()),
    ] {
        let clear = style.visuals.background;
        let mut fonts = Fonts::bundled_only();
        let list = containers_scene(&mut fonts, 2.0, style);
        let size = PhysicalSize::new(840, 600);
        if let Some(pixels) = render_with_clear(&list, size, 2.0, fonts.atlas_mut(), clear) {
            check_snapshot(name, size, &pixels);
        }
    }
}

/// Text fields: with `compose`, the second field is focused and composing
/// with an input method (underlined preedit, caret after it); otherwise
/// the first is focused with a selection. The third shows its hint.
fn text_edit_scene(fonts: &mut Fonts, pixels_per_point: f32, compose: bool) -> DisplayList {
    use rustroke_core::{Event, ImeEvent, Key, Modifiers, Point, PointerButton, RawInput};
    use rustroke_widgets::{Context, TextEdit};

    let screen = Rect::from_min_size(Point::ZERO, vec2(300.0, 150.0));
    let mut ctx = Context::new();
    let mut list = DisplayList::new();
    let mut texts = [
        String::from("Selected text"),
        String::from("Composing: "),
        String::new(),
    ];
    let mut rects = [Rect::NOTHING; 3];
    let mut time = 0.0;
    let mut frame = |events: Vec<Event>, texts: &mut [String; 3], rects: &mut [Rect; 3]| {
        time += 0.01;
        ctx.begin_frame(RawInput {
            time,
            screen_rect: screen,
            pixels_per_point,
            events,
        });
        ctx.ui(screen.expand(-12.0), fonts, |ui| {
            let [a, b, c] = texts;
            rects[0] = ui.add(TextEdit::singleline(a)).rect;
            rects[1] = ui.add(TextEdit::singleline(b)).rect;
            rects[2] = ui.add(TextEdit::singleline(c).hint_text("Hint text")).rect;
        });
        let mut output = ctx.end_frame();
        list = std::mem::take(&mut output.shapes);
    };
    let click = |pos: Point| {
        let b = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        vec![b(true), b(false)]
    };
    let shift_left = Event::Key {
        key: Key::ArrowLeft,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    };
    frame(vec![], &mut texts, &mut rects);
    let end = |r: Rect| point(r.max.x - 4.0, r.center().y);
    if compose {
        // Field 2: commit some text, then compose more (not committed).
        frame(click(end(rects[1])), &mut texts, &mut rects);
        frame(
            vec![Event::Ime(ImeEvent::Commit("ab".into()))],
            &mut texts,
            &mut rects,
        );
        frame(
            vec![Event::Ime(ImeEvent::Preedit("cd".into()))],
            &mut texts,
            &mut rects,
        );
    } else {
        // Field 1: select the last word.
        frame(click(end(rects[0])), &mut texts, &mut rects);
        frame(vec![shift_left; 4], &mut texts, &mut rects);
    }
    list
}

#[test]
fn text_edit_2x() {
    for (name, compose) in [("text_edit_2x", false), ("text_edit_ime_2x", true)] {
        let mut fonts = Fonts::bundled_only();
        let list = text_edit_scene(&mut fonts, 2.0, compose);
        let size = PhysicalSize::new(600, 300);
        if let Some(pixels) = render_with_atlas(&list, size, 2.0, fonts.atlas_mut()) {
            check_snapshot(name, size, &pixels);
        }
    }
}

/// User images: a gradient, a translucent checkerboard (alpha blending),
/// a scaled-down copy, a tinted one and a disabled (faded) one.
#[test]
fn images_2x() {
    use rustroke_core::{ColorImage, Point, RawInput};
    use rustroke_widgets::{Context, Image};

    let gradient = ColorImage::from_fn([64, 64], |x, y| {
        Color::from_srgb8((x * 4) as u8, (y * 4) as u8, 200)
    });
    let checker = ColorImage::from_fn([32, 32], |x, y| {
        if (x / 8 + y / 8) % 2 == 0 {
            Color::WHITE
        } else {
            Color::new(1.0, 0.2, 0.2, 0.5)
        }
    });
    let mut fonts = Fonts::bundled_only();
    let mut ctx = Context::new();
    let gradient = ctx.load_texture(gradient);
    let checker = ctx.load_texture(checker);
    let screen = Rect::from_min_size(Point::ZERO, vec2(400.0, 90.0));
    ctx.begin_frame(RawInput {
        time: 1.0,
        screen_rect: screen,
        pixels_per_point: 2.0,
        events: vec![],
    });
    ctx.ui(screen.expand(-12.0), &mut fonts, |ui| {
        ui.horizontal(|ui| {
            ui.image(&gradient);
            ui.add(Image::new(&checker).size(vec2(64.0, 64.0)));
            ui.add(Image::new(&gradient).max_width(32.0));
            ui.add(Image::new(&gradient).tint(Color::from_srgb8(166, 227, 161)));
            ui.add_enabled(false, Image::new(&gradient));
        });
    });
    let output = ctx.end_frame();
    assert_eq!(output.textures.set.len(), 2);
    let size = PhysicalSize::new(800, 180);
    let clear = Color::from_srgb8(30, 30, 46);
    if let Some(pixels) = render_full(
        &output.shapes,
        size,
        2.0,
        fonts.atlas_mut(),
        clear,
        &output.textures,
    ) {
        check_snapshot("images_2x", size, &pixels);
    }
}

#[test]
fn empty_scene_is_clear_color() {
    let size = PhysicalSize::new(8, 8);
    let Some(pixels) = render(&DisplayList::new(), size, 1.0) else {
        return;
    };
    for px in pixels.as_chunks::<4>().0 {
        assert_eq!(*px, [30, 30, 46, 255]);
    }
}
