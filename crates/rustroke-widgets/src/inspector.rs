//! A debugging window: what is under the pointer, frame timing, focus,
//! and the style edited live.

use rustroke_core::{Rect, Stroke, point};

use crate::context::Order;
use crate::{CollapsingHeader, Id, LayerId, Slider, Style, UiRoot, Window};

/// Shows the inspector window and outlines the widgets under the pointer,
/// while [`crate::Context::is_inspector_open`] is true (Cmd/Ctrl+Alt+I
/// toggles it). rustroke-winit calls this after the app's update; call it
/// yourself at the end of a frame when driving a `Context` directly.
pub fn show_inspector(root: &mut impl UiRoot) {
    let (ctx, fonts) = root.parts();
    if !ctx.is_inspector_open() {
        return;
    }
    let window_rect: Option<Rect> = ctx.data(Id::new("inspector rect"));
    let in_inspector = |r: Rect| window_rect.is_some_and(|w| w.contains(r.center()));
    let pointer = ctx.input().pointer.pos();
    // Last frame's widgets (the app's, not the inspector's own).
    let under: Vec<_> = ctx
        .widgets()
        .iter()
        .filter(|w| pointer.is_some_and(|p| w.rect.contains(p)) && !in_inspector(w.rect))
        .cloned()
        .collect();
    let focused = ctx
        .focused()
        .and_then(|id| ctx.widgets().iter().find(|w| w.id == id))
        .map(|w| format!("{:?} \"{}\"", w.info.role, w.info.label));
    let widget_count = ctx.widgets().len();
    let (dt, zoom, ppp) = (
        ctx.input().dt,
        ctx.zoom_factor(),
        ctx.input().pixels_per_point,
    );

    // Outlines, above everything; the innermost widget is highlighted.
    let style = std::sync::Arc::clone(ctx.style());
    let overlay = LayerId::new(Order::Tooltip, Id::new("inspector overlay"));
    let screen = ctx.input().screen_rect;
    ctx.ui_in_layer(overlay, Id::new("inspector overlay"), screen, fonts, |ui| {
        let innermost = under
            .iter()
            .min_by(|a, b| area(a.rect).total_cmp(&area(b.rect)));
        for w in &under {
            let color = if innermost.is_some_and(|i| i.id == w.id) {
                style.visuals.warning
            } else {
                style.visuals.info.with_alpha(0.6)
            };
            ui.painter()
                .rect_stroke(w.rect, 0.0, Stroke::new(1.0, color));
        }
    });

    let mut open = true;
    let shown = Window::new("Inspector")
        .id_salt("rustroke inspector")
        .open(&mut open)
        .default_pos(point(screen.max.x - 340.0, 40.0))
        .default_width(320.0)
        .show(root, |ui| {
            ui.label(format!(
                "{:.1} ms since the last frame · zoom {zoom:.2} · {ppp:.2} px/pt",
                dt * 1000.0
            ));
            ui.label(format!("{widget_count} widgets described"));
            ui.label(format!(
                "Focus: {}",
                focused.unwrap_or_else(|| "none".to_owned())
            ));
            if let Some(p) = pointer {
                ui.label(format!("Pointer: {:.1}, {:.1}", p.x, p.y));
            }
            CollapsingHeader::new("Under the pointer")
                .default_open(true)
                .show(ui, |ui| {
                    if under.is_empty() {
                        ui.label("Nothing");
                    }
                    for w in &under {
                        let r = w.rect;
                        ui.label(format!(
                            "{:?} \"{}\" at {:.0},{:.0} {:.0}×{:.0}{}",
                            w.info.role,
                            w.info.label,
                            r.min.x,
                            r.min.y,
                            r.width(),
                            r.height(),
                            if w.enabled { "" } else { " (disabled)" }
                        ));
                    }
                });
            CollapsingHeader::new("Style").show(ui, |ui| {
                let mut style: Style = (*ui.ctx().style().as_ref()).clone();
                let before = style.clone();
                let mut dark = style.visuals.dark_mode;
                if ui.checkbox(&mut dark, "Dark theme").changed() {
                    let spacing = style.spacing.clone();
                    style = if dark { Style::dark() } else { Style::light() };
                    style.spacing = spacing;
                }
                let s = &mut style.spacing;
                ui.add(Slider::new(&mut s.item_spacing.x, 0.0..=24.0).text("Item spacing x"));
                ui.add(Slider::new(&mut s.item_spacing.y, 0.0..=24.0).text("Item spacing y"));
                ui.add(Slider::new(&mut s.interact_height, 16.0..=48.0).text("Interact height"));
                ui.add(Slider::new(&mut s.window_padding, 0.0..=24.0).text("Window padding"));
                let v = &mut style.visuals;
                ui.add(Slider::new(&mut v.corner_radius, 0.0..=16.0).text("Corner radius"));
                ui.add(Slider::new(&mut style.animation_time, 0.0..=0.5).text("Animation time"));
                if style != before {
                    ui.ctx().set_style(style);
                }
            });
        });
    let (ctx, _) = root.parts();
    if let Some(r) = shown {
        ctx.insert_data(Id::new("inspector rect"), r.response.rect);
    }
    if !open {
        ctx.set_inspector_open(false);
    }
}

fn area(r: Rect) -> f32 {
    r.width() * r.height()
}
