//! Accessibility: a description of the widgets for screen readers
//! (VoiceOver, Narrator, Orca), built with AccessKit.
//!
//! Widgets describe themselves with [`crate::Ui::describe`]. When the
//! platform reports that assistive technology is active
//! ([`crate::Context::set_accessibility_active`]), the context collects
//! these descriptions each frame and turns them into an AccessKit tree
//! update ([`crate::FrameOutput::accesskit_update`]).

use accesskit::{Action, Node, NodeId, Role, Toggled, TreeId, TreeInfo, TreeUpdate};
use rustroke_core::Rect;

use crate::Id;

/// What kind of widget a [`WidgetInfo`] describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetRole {
    /// Static text.
    Label,
    /// A push button.
    Button,
    /// A checkbox.
    Checkbox,
    /// A radio button.
    RadioButton,
    /// A slider.
    Slider,
    /// A single-line text field.
    TextInput,
    /// A multi-line text field.
    MultilineTextInput,
    /// An image.
    Image,
    /// An item that can be selected (e.g. in a list or a combo box).
    SelectableItem,
    /// A drop-down list (combo box).
    ComboBox,
    /// A number edited by dragging or typing.
    DragValue,
    /// A progress bar or activity indicator.
    Progress,
    /// The header of a collapsible section.
    CollapsingHeader,
    /// A list of selectable rows.
    List,
    /// A tab of a tab bar.
    Tab,
}

/// A widget as seen by a screen reader.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetInfo {
    /// The kind of widget.
    pub role: WidgetRole,
    /// What the widget is called (the button text, the checkbox label...).
    pub label: String,
    /// Current text of a text field.
    pub value: Option<String>,
    /// Checked state of checkboxes and radio buttons.
    pub toggled: Option<bool>,
    /// Value and range of sliders.
    pub numeric: Option<NumericInfo>,
    /// Expanded state of collapsible sections and open combo boxes.
    pub expanded: Option<bool>,
    /// Selected state of selectable items.
    pub selected: Option<bool>,
}

/// The value and range of a slider, for screen readers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericInfo {
    /// Current value.
    pub value: f64,
    /// Smallest value.
    pub min: f64,
    /// Largest value.
    pub max: f64,
    /// Increment, if values snap to steps.
    pub step: Option<f64>,
}

impl WidgetInfo {
    /// A description with a role and label, and no state.
    pub fn new(role: WidgetRole, label: impl Into<String>) -> Self {
        Self {
            role,
            label: label.into(),
            value: None,
            toggled: None,
            numeric: None,
            expanded: None,
            selected: None,
        }
    }

    /// Adds the expanded state.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Adds the selected state.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Adds the checked state.
    pub fn toggled(mut self, on: bool) -> Self {
        self.toggled = Some(on);
        self
    }

    /// Adds a text value (the content of a text field).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Adds a numeric value and range.
    pub fn numeric(mut self, numeric: NumericInfo) -> Self {
        self.numeric = Some(numeric);
        self
    }
}

/// A widget placed in a frame, with what it is and where: see
/// `Context::widgets`. Useful to find widgets in tests.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetDescription {
    /// The widget's id.
    pub id: Id,
    /// Role, label and state.
    pub info: WidgetInfo,
    /// Where it is, in points.
    pub rect: Rect,
    /// False if it was disabled.
    pub enabled: bool,
    /// Whether it can take keyboard focus.
    pub focusable: bool,
}

/// Node id of the root (the whole window).
const ROOT: NodeId = NodeId(0);

pub(crate) fn node_id(id: Id) -> NodeId {
    // 0 is the root; widget ids are hashes, so a collision is negligible.
    NodeId(id.value().max(1))
}

/// Builds a full AccessKit tree: a window containing every described
/// widget in the order they were added. Coordinates are scaled from
/// points to physical pixels by the root's transform.
pub(crate) fn build_tree(
    widgets: &[WidgetDescription],
    focused: Option<Id>,
    screen: Rect,
    pixels_per_point: f32,
) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_transform(accesskit::Affine::scale(f64::from(pixels_per_point)));
    root.set_bounds(to_ak_rect(screen));
    root.set_children(widgets.iter().map(|w| node_id(w.id)).collect::<Vec<_>>());

    let mut nodes = Vec::with_capacity(widgets.len() + 1);
    nodes.push((ROOT, root));
    for w in widgets {
        nodes.push((node_id(w.id), widget_node(w)));
    }
    let focus = focused
        .filter(|f| widgets.iter().any(|w| w.id == *f))
        .map_or(ROOT, node_id);
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus,
    }
}

fn widget_node(w: &WidgetDescription) -> Node {
    let info = &w.info;
    let role = match info.role {
        WidgetRole::Label => Role::Label,
        WidgetRole::Button => Role::Button,
        WidgetRole::Checkbox => Role::CheckBox,
        WidgetRole::RadioButton => Role::RadioButton,
        WidgetRole::Slider => Role::Slider,
        WidgetRole::TextInput => Role::TextInput,
        WidgetRole::MultilineTextInput => Role::MultilineTextInput,
        WidgetRole::Image => Role::Image,
        WidgetRole::SelectableItem => Role::ListBoxOption,
        WidgetRole::ComboBox => Role::ComboBox,
        WidgetRole::DragValue => Role::SpinButton,
        WidgetRole::Progress => Role::ProgressIndicator,
        WidgetRole::CollapsingHeader => Role::Button,
        WidgetRole::List => Role::ListBox,
        WidgetRole::Tab => Role::Tab,
    };
    let mut node = Node::new(role);
    node.set_bounds(to_ak_rect(w.rect));
    if !info.label.is_empty() {
        node.set_label(info.label.clone());
    }
    if let Some(value) = &info.value {
        node.set_value(value.clone());
    }
    if let Some(on) = info.toggled {
        node.set_toggled(Toggled::from(on));
    }
    if let Some(expanded) = info.expanded {
        node.set_expanded(expanded);
    }
    if let Some(selected) = info.selected {
        node.set_selected(selected);
    }
    if let Some(n) = info.numeric {
        node.set_numeric_value(n.value);
        node.set_min_numeric_value(n.min);
        node.set_max_numeric_value(n.max);
        if let Some(step) = n.step {
            node.set_numeric_value_step(step);
        }
    }
    if !w.enabled {
        node.set_disabled();
    } else {
        if w.focusable {
            node.add_action(Action::Focus);
        }
        if matches!(
            info.role,
            WidgetRole::Button
                | WidgetRole::Checkbox
                | WidgetRole::RadioButton
                | WidgetRole::SelectableItem
                | WidgetRole::ComboBox
                | WidgetRole::CollapsingHeader
                | WidgetRole::Tab
        ) {
            node.add_action(Action::Click);
        }
    }
    node
}

fn to_ak_rect(r: Rect) -> accesskit::Rect {
    accesskit::Rect::new(
        f64::from(r.min.x),
        f64::from(r.min.y),
        f64::from(r.max.x),
        f64::from(r.max.y),
    )
}

/// What an assistive technology asked a widget to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingAction {
    Click(Id),
    Focus(Id),
}

/// Converts an AccessKit request into an action on one of `widgets`.
pub(crate) fn pending_action(
    request: &accesskit::ActionRequest,
    widgets: &[WidgetDescription],
) -> Option<PendingAction> {
    let target = widgets
        .iter()
        .find(|w| node_id(w.id) == request.target_node)?;
    match request.action {
        Action::Click => Some(PendingAction::Click(target.id)),
        Action::Focus => Some(PendingAction::Focus(target.id)),
        _ => None,
    }
}
