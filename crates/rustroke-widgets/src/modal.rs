//! Modal dialogs: a card in the middle of the window over a veil that
//! blocks the rest of the UI.

use std::hash::Hash;

use rustroke_core::{Key, Modifiers, Rect, Vec2, point, vec2};

use crate::containers::paint_floating_frame;
use crate::context::Order;
use crate::{Align, Id, Label, LayerId, Layout, Response, Sense, Ui, UiRoot};

/// What [`Modal::show`] returns.
#[derive(Clone, Debug)]
pub struct ModalResponse<R> {
    /// What the content closure returned.
    pub inner: R,
    /// Covers the dialog's card.
    pub response: Response,
    /// The user asked to close the dialog: Escape (when no text field or
    /// popup took it) or, with [`Modal::close_on_click_outside`], a click
    /// on the veil. Stop showing the dialog to close it.
    pub should_close: bool,
}

impl<R> ModalResponse<R> {
    /// The user asked to close the dialog (see
    /// [`ModalResponse::should_close`]).
    pub fn should_close(&self) -> bool {
        self.should_close
    }
}

/// A modal dialog ("Save changes?", settings, parameters): a card in the
/// middle of the window, drawn over a veil that takes every click, so
/// nothing else reacts while it is open. Tab and Shift+Tab only move
/// between its widgets, and focus elsewhere is removed.
///
/// The dialog is open while the app calls [`Modal::show`] every frame;
/// close it by not calling it any more (e.g. when a button is clicked or
/// [`ModalResponse::should_close`] is true). While it is open,
/// [`crate::Context::is_modal_open`] tells the app to ignore shortcuts.
///
/// ```no_run
/// # fn demo(frame: &mut impl rustroke_widgets::UiRoot, open: &mut bool) {
/// use rustroke_widgets::Modal;
/// if *open {
///     let response = Modal::new("confirm").title("Delete the part?").show(frame, |ui| {
///         ui.label("This can't be undone.");
///         ui.horizontal(|ui| ui.button("Delete").clicked() || ui.button("Cancel").clicked())
///             .inner
///     });
///     if response.inner || response.should_close {
///         *open = false;
///     }
/// }
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Modal {
    id: Id,
    title: Option<String>,
    width: f32,
    close_on_click_outside: bool,
}

impl Modal {
    /// A dialog identified by `id_salt` (it keeps its measured size).
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id: Id::new("modal").with(id_salt),
            title: None,
            width: 360.0,
            close_on_click_outside: false,
        }
    }

    /// A bold title above the content.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Width of the card in points (default 360), limited by the window.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// A click on the veil asks to close the dialog (default `false`: the
    /// veil only blocks clicks).
    pub fn close_on_click_outside(mut self, close: bool) -> Self {
        self.close_on_click_outside = close;
        self
    }

    /// Shows the dialog with its content for this frame.
    pub fn show<R>(
        self,
        root: &mut impl UiRoot,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> ModalResponse<R> {
        let (ctx, fonts) = root.parts();
        let style = std::sync::Arc::clone(ctx.style());
        let visuals = &style.visuals;
        let pad = style.spacing.window_padding * 1.5;
        let screen = ctx.input().screen_rect;
        let layer = LayerId::new(Order::Modal, self.id);
        ctx.set_modal(layer);

        // The card is centered with last frame's content height; the first
        // frame is drawn invisibly to measure it.
        let measured: Option<Vec2> = ctx.data(self.id);
        let width = self.width.min(screen.width() - 2.0 * pad).max(0.0);
        let height = measured.map_or(0.0, |m| m.y + 2.0 * pad);
        let card = Rect::from_center_size(screen.center(), vec2(width, height));
        let card = Rect::from_min_size(
            point(card.min.x, card.min.y.max(screen.min.y + pad)),
            card.size(),
        );

        let mut should_close = false;
        let (inner, content) = ctx.ui_in_layer(layer, self.id, screen, fonts, |ui| {
            let start = ui.painter().len();
            let veil = ui.interact(self.id.with("veil"), screen, Sense::POINTER_DRAG);
            ui.painter()
                .rect_filled(screen, 0.0, visuals.modal_backdrop);
            let _ = ui.interact(self.id.with("card"), card, Sense::POINTER_DRAG);
            paint_floating_frame(ui.painter(), card, visuals);

            let content_max = Rect::from_min_max(
                card.min + Vec2::splat(pad),
                point(card.max.x - pad, screen.max.y.max(card.max.y)),
            );
            let content = ui.scope_with(content_max, Layout::top_down(Align::Min), |ui| {
                if let Some(title) = &self.title {
                    let title_style = ui.style().body.clone().bold();
                    ui.add(Label::new(title.as_str()).style(title_style));
                }
                add_contents(ui)
            });
            if measured.is_none() {
                ui.painter().multiply_alpha_from(start, 0.0);
            }
            if self.close_on_click_outside && veil.clicked() {
                should_close = true;
            }
            (content.inner, content.response)
        });

        // Escape the content didn't use (a text field cancels its edit
        // first) closes the dialog.
        if ctx.open_popup_id().is_none()
            && ctx.input_mut().consume_key(Key::Escape, Modifiers::NONE)
        {
            should_close = true;
        }

        let size = content.rect.size();
        if measured != Some(size) {
            ctx.insert_data(self.id, size);
            ctx.request_repaint();
        }
        let mut response = content;
        response.rect = card;
        ModalResponse {
            inner,
            response,
            should_close,
        }
    }
}
