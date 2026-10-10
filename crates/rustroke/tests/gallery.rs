//! Renders the README's gallery images (docs/images/*.png) from the same
//! code the README shows. Run after a visual change:
//!
//! `cargo test -p rustroke --test gallery --features markdown -- --ignored`

use rustroke::testing::Harness;
use rustroke::{
    Align, Button, CentralPanel, CollapsingHeader, Color, Column, ComboBox, Frame, Gradient, Grid,
    Label, LayoutJob, Modal, Panel, Rect, Shadow, Slider, Stroke, Table, TextEdit, TextFormat,
    TextStyle, Toast, Transform, point, vec2,
};

/// Runs `app` until it settles and saves the frame as `docs/images/<name>.png`.
fn shoot(name: &str, size: [f32; 2], mut app: impl FnMut(&mut Frame)) {
    let mut harness = Harness::with_size(size[0], size[1]).with_pixels_per_point(2.0);
    let mut app = |frame: &mut Frame| {
        frame.clear_color = frame.ctx().style().visuals.background;
        app(frame);
    };
    for _ in 0..4 {
        harness.run(&mut app);
        harness.advance_time(1.0);
    }
    harness.run(&mut app);
    let image = harness
        .render()
        .unwrap_or_else(|_| harness.render_software());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/images");
    std::fs::create_dir_all(&dir).unwrap();
    image.save_png(dir.join(format!("{name}.png"))).unwrap();
}

#[test]
#[ignore = "writes the README images; run with --ignored"]
fn gallery() {
    // Widgets.
    let (mut dark, mut volume, mut quality, mut name) = (true, 65, "High", String::new());
    shoot("widgets", [380.0, 300.0], |frame| {
        frame.ui(|ui| {
            ui.heading("Settings");
            ui.checkbox(&mut dark, "Dark theme");
            ui.add(Slider::new(&mut volume, 0..=100).text("Volume"));
            ComboBox::from_label("Quality")
                .selected_text(quality)
                .show_ui(ui, |ui| {
                    for q in ["Low", "Medium", "High"] {
                        ui.selectable_value(&mut quality, q, q);
                    }
                });
            ui.add(TextEdit::singleline(&mut name).hint_text("Your name"));
            ui.horizontal(|ui| {
                ui.button("Cancel");
                ui.button("Save");
            });
        });
    });

    // Panels, menus, a tree and a grid.
    shoot("layout", [480.0, 300.0], |frame| {
        Panel::top("menu").show(frame, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| ui.button("Open"));
                ui.menu_button("Edit", |ui| ui.button("Undo"));
                ui.menu_button("View", |ui| ui.button("Zoom"));
            });
        });
        Panel::bottom("status").show(frame, |ui| ui.label("Ready"));
        Panel::left("tree").show(frame, |ui| {
            CollapsingHeader::new("Parts")
                .default_open(true)
                .show(ui, |ui| {
                    ui.label("Housing");
                    ui.label("Cover");
                    ui.label("Screws");
                });
        });
        CentralPanel::default().show(frame, |ui| {
            ui.heading("Housing");
            Grid::new("properties").striped(true).show(ui, |ui| {
                ui.label("Material");
                ui.label("Aluminium");
                ui.end_row();
                ui.label("Mass");
                ui.label("0.42 kg");
                ui.end_row();
                ui.label("Finish");
                ui.label("Anodized");
                ui.end_row();
            });
        });
    });

    // A table.
    let parts = [
        ("Bracket", "Steel", 4),
        ("Housing", "Aluminium", 1),
        ("Cover", "ABS", 1),
        ("Screw M3", "Steel", 12),
        ("Gasket", "Rubber", 2),
        ("Shaft", "Brass", 1),
    ];
    let mut selected = Some(1);
    shoot("table", [420.0, 250.0], |frame| {
        frame.ui(|ui| {
            Table::new("parts")
                .column(Column::new("Part").sortable(true))
                .column(Column::new("Material"))
                .column(Column::new("Qty").align(Align::Max))
                .show(ui, parts.len(), &mut selected, |ui, row, col| {
                    let (part, material, qty) = parts[row];
                    match col {
                        0 => ui.label(part),
                        1 => ui.label(material),
                        _ => ui.label(qty.to_string()),
                    };
                });
        });
    });

    // Rich text and Markdown.
    shoot("text", [380.0, 300.0], |frame| {
        let pink = Color::from_srgb8(243, 139, 168);
        frame.ui(|ui| {
            let mut job = LayoutJob::default();
            job.append("Rich text: ", TextFormat::new());
            job.append(
                "bold",
                TextFormat::new().style(TextStyle::proportional(14.0).bold()),
            );
            job.append(", ", TextFormat::new());
            job.append("colored", TextFormat::new().color(pink));
            job.append(" and a ", TextFormat::new());
            job.append("link", TextFormat::new().link("https://docs.rs/rustroke"));
            ui.add(Label::rich(job));
            ui.markdown(
                "## Markdown\n\
                 - **bold**, *italic*, `code`\n\
                 - [x] task lists\n\n\
                 > quotes, tables, code blocks…",
            );
        });
    });

    // Drawing.
    shoot("graphics", [380.0, 260.0], |frame| {
        let blue = Color::from_srgb8(137, 180, 250);
        let pink = Color::from_srgb8(245, 194, 231);
        let green = Color::from_srgb8(166, 227, 161);
        let s = &mut frame.shapes;
        let card = Rect::from_min_size(point(30.0, 30.0), vec2(170.0, 110.0));
        let shadow = Shadow {
            offset: vec2(0.0, 10.0),
            blur: 24.0,
            spread: 0.0,
            color: Color::BLACK.with_alpha(0.6),
        };
        s.shadow(card, 14.0, shadow);
        let gradient = Gradient::linear(card.left_top(), card.right_bottom(), blue, pink);
        s.rect_gradient(card, 14.0, gradient, Stroke::NONE);
        s.cubic_bezier(
            [
                point(30.0, 220.0),
                point(110.0, 140.0),
                point(200.0, 260.0),
                point(350.0, 170.0),
            ],
            Stroke::new(3.0, green),
        );
        s.dashed_line(
            &[point(30.0, 240.0), point(350.0, 240.0)],
            Stroke::new(2.0, blue),
            9.0,
            6.0,
        );
        let center = point(285.0, 85.0);
        s.with_transform(Transform::rotate_around(center, 0.4), |s| {
            s.rect_stroke(
                Rect::from_center_size(center, vec2(90.0, 60.0)),
                8.0,
                Stroke::new(2.5, pink),
            );
        });
    });

    // A modal dialog and a toast.
    let mut first = true;
    shoot("dialog", [420.0, 280.0], |frame| {
        if first {
            frame
                .ctx()
                .toast(Toast::success("Exported drawing.pdf").duration(60.0));
            first = false;
        }
        frame.ui(|ui| {
            ui.heading("Drawing");
            ui.label("Bracket, sheet 1 of 3");
        });
        Modal::new("save").title("Save changes?").show(frame, |ui| {
            ui.label("Your changes are lost if you don't save them.");
            ui.horizontal(|ui| {
                ui.button("Don't save");
                ui.button("Cancel");
                ui.add(Button::new("Save"));
            });
        });
    });
}
