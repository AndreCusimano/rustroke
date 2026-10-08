//! The immediate-mode API: [`Context`] (state kept between frames),
//! [`Ui`] (places and draws widgets), [`Response`] (what the user did with a
//! widget) and the built-in widgets.
//!
//! ```ignore
//! frame.ui(|ui| {
//!     ui.heading("Settings");
//!     if ui.button("Save").clicked() { /* ... */ }
//!     ui.checkbox(&mut enabled, "Enabled");
//!     ui.slider(&mut volume, 0.0..=1.0);
//! });
//! ```
//!
//! This crate has no platform or GPU dependency, so interactions can be
//! tested by feeding [`rustroke_core::RawInput`] to a [`Context`].

mod accessibility;
mod containers;
mod context;
mod grid;
mod id;
mod layout;
mod popup;
mod response;
mod style;
mod text_edit;
mod ui;
pub mod widgets;

pub use accessibility::{NumericInfo, WidgetDescription, WidgetInfo, WidgetRole};
pub use containers::{CentralPanel, Panel, PanelSide, ScrollArea, UiRoot, Window};
pub use context::{
    Context, CursorIcon, FrameOutput, LayerId, Order, RepaintHandle, Sense, TextureHandle,
};
pub use grid::Grid;
pub use id::Id;
pub use layout::{Align, Direction, Layout};
pub use response::Response;
pub use style::{Spacing, Style, Visuals, WidgetVisuals};
pub use text_edit::TextEdit;
pub use ui::{InnerResponse, Ui};
pub use widgets::{
    Button, Checkbox, Image, Label, Numeric, RadioButton, Separator, Slider, Widget,
};

#[cfg(test)]
mod tests;
