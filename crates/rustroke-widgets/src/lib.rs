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
mod automation;
mod collapsing;
mod combo_box;
mod containers;
mod context;
mod dnd;
mod dock;
mod drag_value;
mod flex;
mod grid;
mod id;
mod inspector;
mod layout;
mod list;
#[cfg(feature = "markdown")]
mod markdown;
mod modal;
mod pickers;
mod popup;
mod property_grid;
mod response;
mod search_field;
mod style;
mod table;
mod tabs;
mod text_edit;
mod toast;
mod tool_button;
mod tree;
mod ui;
pub mod widgets;

pub use accessibility::{NumericInfo, WidgetDescription, WidgetInfo, WidgetRole};
pub use automation::Automation;
pub use collapsing::{CollapsingHeader, CollapsingResponse};
pub use combo_box::ComboBox;
pub use containers::{CentralPanel, Panel, PanelSide, ScrollArea, UiRoot, Window};
pub use context::{
    Context, CursorIcon, FrameOutput, LayerId, Order, Persist, RepaintHandle, Sense, TextureHandle,
};
pub use dock::{DockArea, DockNode, DockState, DockViewer, SplitAxis};
pub use drag_value::DragValue;
pub use flex::{
    Flex, FlexAlign, FlexDirection, FlexGrid, FlexItem, FlexJustify, FlexUi, GridCell, GridUi,
    Track,
};
pub use grid::Grid;
pub use id::Id;
pub use inspector::show_inspector;
pub use layout::{Align, Direction, Layout};
pub use list::{List, ListResponse};
#[cfg(feature = "markdown")]
pub use markdown::{ImageLoader, Markdown};
pub use modal::{Modal, ModalResponse};
pub use pickers::{ColorPicker, Date, DatePicker, TimePicker, days_in_month};
pub use property_grid::{PropertyGrid, PropertyGridUi, ReferenceField};
pub use response::{FocusLost, Response};
pub use search_field::SearchField;
pub use style::{Spacing, Style, Visuals, WidgetVisuals};
pub use table::{Column, SortOrder, Table, TableResponse};
pub use tabs::{TabBar, TabBarResponse, TabLabel};
pub use text_edit::TextEdit;
pub use toast::{Toast, ToastLevel, show_toasts};
pub use tool_button::{IconToggle, ToolButton, ToolButtonResponse};
pub use tree::{Tree, TreeResponse};
pub use ui::{InnerResponse, Ui};
pub use widgets::{
    AnimatedImage, AnimatedTexture, Button, Checkbox, Hyperlink, Icon, Image, Label, Numeric,
    ProgressBar, RadioButton, SelectableLabel, Separator, Slider, Spinner, Widget,
};

#[cfg(test)]
mod tests;
