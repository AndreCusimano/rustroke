//! Toasts: short messages in a corner of the window that go away by
//! themselves.

use rustroke_core::{Rect, Stroke, point, vec2};

use crate::containers::paint_floating_frame;
use crate::context::Order;
use crate::{Id, LayerId, Sense, UiRoot};

/// How important a [`Toast`] is (sets its color).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToastLevel {
    /// Neutral information.
    Info,
    /// Something went well.
    Success,
    /// Needs attention.
    Warning,
    /// Something failed.
    Error,
}

/// A short message shown in the bottom-right corner for a few seconds
/// (see [`crate::Context::toast`]). Toasts stack upwards; each has a ×
/// to dismiss it early.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    /// The message (wraps).
    pub text: String,
    /// Its color.
    pub level: ToastLevel,
    /// Seconds it stays.
    pub duration: f64,
}

impl Toast {
    fn new(level: ToastLevel, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level,
            duration: 4.0,
        }
    }

    /// Neutral information.
    pub fn info(text: impl Into<String>) -> Self {
        Self::new(ToastLevel::Info, text)
    }

    /// Something went well.
    pub fn success(text: impl Into<String>) -> Self {
        Self::new(ToastLevel::Success, text)
    }

    /// Something needs attention.
    pub fn warning(text: impl Into<String>) -> Self {
        Self::new(ToastLevel::Warning, text)
    }

    /// Something failed (stays longer).
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            duration: 8.0,
            ..Self::new(ToastLevel::Error, text)
        }
    }

    /// Seconds it stays (default 4, errors 8).
    pub fn duration(mut self, seconds: f64) -> Self {
        self.duration = seconds;
        self
    }
}

/// A toast on screen: an id, the toast and when it appeared.
pub(crate) type ShownToast = (Id, Toast, f64);

/// Draws the toasts of [`crate::Context::toast`]; rustroke-winit calls it
/// after the app's update (call it yourself at the end of a frame when
/// driving a `Context` directly).
pub fn show_toasts(root: &mut impl UiRoot) {
    let (ctx, fonts) = root.parts();
    let now = ctx.input().time;
    let mut toasts = ctx.take_toasts();
    // Toasts added this frame start now.
    for t in &mut toasts {
        if t.2.is_nan() {
            t.2 = now;
        }
    }
    toasts.retain(|(_, t, start)| now - start < t.duration);
    if toasts.is_empty() {
        return;
    }
    let style = std::sync::Arc::clone(ctx.style());
    let visuals = &style.visuals;
    let screen = ctx.input().screen_rect;
    let width = 300.0_f32.min(screen.width() - 32.0);
    let pad = 12.0;
    let mut bottom = screen.max.y - 16.0;
    let mut dismissed = Vec::new();
    let mut next_change = f64::INFINITY;
    for (id, toast, start) in toasts.iter().rev() {
        let age = now - start;
        let left = toast.duration - age;
        next_change = next_change.min(left);
        // Fade in and out over a quarter second.
        let alpha = ((age / 0.25).min(left / 0.25)).clamp(0.0, 1.0) as f32;
        if alpha < 1.0 {
            ctx.request_repaint();
        }
        let galley = fonts.layout(
            &toast.text,
            &style.body,
            Some(width - 2.0 * pad - 24.0),
            ctx.input().pixels_per_point,
        );
        let height = galley.size.y + 2.0 * pad;
        let rect = Rect::from_min_max(
            point(screen.max.x - 16.0 - width, bottom - height),
            point(screen.max.x - 16.0, bottom),
        );
        bottom = rect.min.y - 8.0;
        let color = match toast.level {
            ToastLevel::Info => visuals.info,
            ToastLevel::Success => visuals.success,
            ToastLevel::Warning => visuals.warning,
            ToastLevel::Error => visuals.error,
        };
        let layer = LayerId::new(Order::Foreground, id.with("toast"));
        ctx.ui_in_layer(layer, *id, rect, fonts, |ui| {
            let start = ui.painter().len();
            let _ = ui.interact(id.with("background"), rect, Sense::HOVER);
            paint_floating_frame(ui.painter(), rect, visuals);
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min + vec2(0.0, 8.0), vec2(4.0, rect.height() - 16.0)),
                2.0,
                color,
            );
            ui.painter()
                .galley(rect.min + vec2(pad, pad), galley, visuals.text);
            let close = Rect::from_center_size(
                point(rect.max.x - 16.0, rect.min.y + 16.0),
                vec2(18.0, 18.0),
            );
            let r = ui.interact(id.with("close"), close, Sense::CLICK);
            ui.describe(
                &r,
                crate::WidgetInfo::new(crate::WidgetRole::Button, "Dismiss"),
            );
            let w = ui.widget_visuals(&r);
            let c = close.center();
            let fg = if r.hovered() { w.fg } else { visuals.weak_text };
            for d in [vec2(-4.0, -4.0), vec2(-4.0, 4.0)] {
                ui.painter().line(c + d, c - d, Stroke::new(1.5, fg));
            }
            if r.clicked() {
                dismissed.push(*id);
            }
            ui.painter().multiply_alpha_from(start, alpha);
        });
    }
    toasts.retain(|(id, _, _)| !dismissed.contains(id));
    if next_change.is_finite() {
        ctx.request_repaint_after(next_change.max(0.0) + 0.01);
    }
    ctx.put_toasts(toasts);
}
