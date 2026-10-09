//! Dockable panels: tab groups in resizable splits, rearranged by dragging
//! tabs.

use std::hash::Hash;

use rustroke_core::{Point, Rect, Stroke, point, vec2};

use crate::tabs::{TabLabel, insertion_index, tab_strip};
use crate::{Align, CursorIcon, Id, Layout, Sense, Ui};

/// Direction of a split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SplitAxis {
    /// Side by side: first on the left.
    Horizontal,
    /// One above the other: first on top.
    Vertical,
}

/// A node of the dock layout: a group of tabs, or a split in two.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum DockNode<T> {
    /// Tabs shown one at a time.
    Tabs {
        /// Stable identity of the group (kept when the tree changes).
        id: u64,
        /// The tabs, in order.
        tabs: Vec<T>,
        /// Index of the visible tab.
        active: usize,
    },
    /// Two nodes sharing an area.
    Split {
        /// How the area is divided.
        axis: SplitAxis,
        /// Share of the first node (0..1).
        fraction: f32,
        /// Left or top.
        first: Box<DockNode<T>>,
        /// Right or bottom.
        second: Box<DockNode<T>>,
    },
}

/// The layout of a [`DockArea`]: owned by the app, so it can be saved and
/// changed in code. Build it with [`DockState::new`] and the split
/// methods, or from [`DockNode`]s.
/// With the `persistence` feature it can be saved with serde (when the
/// tab type can), e.g. next to the app's settings.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DockState<T> {
    root: DockNode<T>,
    next_id: u64,
}

/// Where a dragged tab will go.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DropTarget {
    /// Into the group's tab bar at this index.
    Insert { group: u64, index: usize },
    /// Next to the group, splitting its area (`first`: before it).
    Split {
        group: u64,
        axis: SplitAxis,
        first: bool,
    },
}

impl<T> DockState<T> {
    /// One group with `tabs`.
    pub fn new(tabs: Vec<T>) -> Self {
        Self {
            root: DockNode::Tabs {
                id: 0,
                tabs,
                active: 0,
            },
            next_id: 1,
        }
    }

    /// The layout tree.
    pub fn root(&self) -> &DockNode<T> {
        &self.root
    }

    /// The id of the group that holds the first tab matching `find`.
    pub fn find_group(&self, find: impl Fn(&T) -> bool) -> Option<u64> {
        fn walk<T>(node: &DockNode<T>, find: &dyn Fn(&T) -> bool) -> Option<u64> {
            match node {
                DockNode::Tabs { id, tabs, .. } => tabs.iter().any(find).then_some(*id),
                DockNode::Split { first, second, .. } => {
                    walk(first, find).or_else(|| walk(second, find))
                }
            }
        }
        walk(&self.root, &find)
    }

    /// The ids of all groups, left to right / top to bottom.
    pub fn groups(&self) -> Vec<u64> {
        let mut out = Vec::new();
        visit(&self.root, &mut |node| {
            if let DockNode::Tabs { id, .. } = node {
                out.push(*id);
            }
        });
        out
    }

    /// Splits group `group` in two: `tabs` go in a new group on the
    /// chosen side (`first` = left/top), taking `fraction` of the area.
    /// Returns the new group's id.
    pub fn split(
        &mut self,
        group: u64,
        axis: SplitAxis,
        first: bool,
        fraction: f32,
        tabs: Vec<T>,
    ) -> Option<u64> {
        let new_id = self.next_id;
        let node = find_group_mut(&mut self.root, group)?;
        self.next_id += 1;
        let old = std::mem::replace(
            node,
            DockNode::Tabs {
                id: u64::MAX,
                tabs: Vec::new(),
                active: 0,
            },
        );
        let new = DockNode::Tabs {
            id: new_id,
            tabs,
            active: 0,
        };
        let (a, b, fraction) = if first {
            (new, old, fraction)
        } else {
            (old, new, 1.0 - fraction)
        };
        *node = DockNode::Split {
            axis,
            fraction: fraction.clamp(0.05, 0.95),
            first: Box::new(a),
            second: Box::new(b),
        };
        Some(new_id)
    }

    /// Adds `tab` to `group` (or the first group if there is no such
    /// group) and makes it the active tab.
    pub fn push_tab(&mut self, group: u64, tab: T) {
        let first = self.groups()[0];
        let id = if find_group_mut(&mut self.root, group).is_some() {
            group
        } else {
            first
        };
        if let Some(DockNode::Tabs { tabs, active, .. }) = find_group_mut(&mut self.root, id) {
            tabs.push(tab);
            *active = tabs.len() - 1;
        }
    }

    /// Every tab, in layout order.
    pub fn tabs(&self) -> Vec<&T> {
        let mut out = Vec::new();
        visit(&self.root, &mut |node| {
            if let DockNode::Tabs { tabs, .. } = node {
                out.extend(tabs.iter());
            }
        });
        out
    }

    /// Removes the first tab matching `find` and returns it (empty groups
    /// disappear).
    pub fn remove_tab(&mut self, find: impl Fn(&T) -> bool) -> Option<T> {
        let group = self.find_group(&find)?;
        let DockNode::Tabs { tabs, .. } = find_group_mut(&mut self.root, group)? else {
            return None;
        };
        let index = tabs.iter().position(&find)?;
        let tab = self.take(group, index);
        self.collapse();
        tab
    }

    /// Takes tab `index` out of `group`, keeping the active index valid.
    fn take(&mut self, group: u64, index: usize) -> Option<T> {
        let DockNode::Tabs { tabs, active, .. } = find_group_mut(&mut self.root, group)? else {
            return None;
        };
        if index >= tabs.len() {
            return None;
        }
        let tab = tabs.remove(index);
        if *active > index || *active >= tabs.len() {
            *active = active.saturating_sub(1);
        }
        Some(tab)
    }

    /// Removes empty groups (the root group may stay empty).
    fn collapse(&mut self) {
        fn walk<T>(node: &mut DockNode<T>) {
            if let DockNode::Split { first, second, .. } = node {
                walk(first);
                walk(second);
                let empty =
                    |n: &DockNode<T>| matches!(n, DockNode::Tabs { tabs, .. } if tabs.is_empty());
                let keep = if empty(first) {
                    Some(std::mem::replace(second.as_mut(), placeholder()))
                } else if empty(second) {
                    Some(std::mem::replace(first.as_mut(), placeholder()))
                } else {
                    None
                };
                if let Some(keep) = keep {
                    *node = keep;
                }
            }
        }
        walk(&mut self.root);
    }

    /// Moves tab `index` of group `from` to `target`.
    fn move_tab(&mut self, from: u64, index: usize, target: DropTarget) {
        let source_len = match find_group_mut(&mut self.root, from) {
            Some(DockNode::Tabs { tabs, .. }) => tabs.len(),
            _ => return,
        };
        match target {
            DropTarget::Insert { group, index: to } if group == from => {
                // Reorder within the bar.
                let to = if to > index { to - 1 } else { to };
                if let Some(DockNode::Tabs { tabs, active, .. }) =
                    find_group_mut(&mut self.root, from)
                    && to != index
                    && index < tabs.len()
                {
                    let tab = tabs.remove(index);
                    tabs.insert(to.min(tabs.len()), tab);
                    *active = to.min(tabs.len() - 1);
                }
            }
            DropTarget::Insert { group, index: to } => {
                let Some(tab) = self.take(from, index) else {
                    return;
                };
                if let Some(DockNode::Tabs { tabs, active, .. }) =
                    find_group_mut(&mut self.root, group)
                {
                    let to = to.min(tabs.len());
                    tabs.insert(to, tab);
                    *active = to;
                }
                self.collapse();
            }
            DropTarget::Split { group, .. } if group == from && source_len == 1 => {
                // Splitting a group with its only tab changes nothing.
            }
            DropTarget::Split { group, axis, first } => {
                let Some(tab) = self.take(from, index) else {
                    return;
                };
                self.split(group, axis, first, 0.5, vec![tab]);
                self.collapse();
            }
        }
    }
}

fn placeholder<T>() -> DockNode<T> {
    DockNode::Tabs {
        id: u64::MAX,
        tabs: Vec::new(),
        active: 0,
    }
}

fn visit<'a, T>(node: &'a DockNode<T>, f: &mut impl FnMut(&'a DockNode<T>)) {
    f(node);
    if let DockNode::Split { first, second, .. } = node {
        visit(first, f);
        visit(second, f);
    }
}

fn find_group_mut<T>(node: &mut DockNode<T>, group: u64) -> Option<&mut DockNode<T>> {
    match node {
        DockNode::Tabs { id, .. } if *id == group => Some(node),
        DockNode::Tabs { .. } => None,
        DockNode::Split { first, second, .. } => {
            if let Some(found) = find_group_mut(first, group) {
                return Some(found);
            }
            find_group_mut(second, group)
        }
    }
}

/// What the app shows in a [`DockArea`]: the tabs' names and contents.
pub trait DockViewer {
    /// The app's tab type (e.g. an enum of panels or a document id).
    type Tab;

    /// The tab's label.
    fn label(&mut self, tab: &Self::Tab) -> TabLabel;

    /// Shows the tab's content.
    fn ui(&mut self, ui: &mut Ui<'_>, tab: &mut Self::Tab);

    /// Whether the tab has a close button (default: yes).
    fn closable(&mut self, _tab: &Self::Tab) -> bool {
        true
    }

    /// The tab's close button was clicked. Return `false` to keep it open
    /// (e.g. to ask about unsaved changes first). Default: close.
    fn on_close(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    /// The items of the tab's context menu (right-click on the tab).
    fn context_menu(&mut self, _ui: &mut Ui<'_>, _tab: &mut Self::Tab) {}

    /// A new tab for the group's "+" button, if [`DockArea::add_button`]
    /// is on.
    fn add_tab(&mut self) -> Option<Self::Tab> {
        None
    }
}

/// Panels arranged as tab groups in resizable splits. Drag a tab to
/// reorder it, into another group's tab bar, or onto the side of a group
/// to split it; drag the line between groups to resize them.
///
/// ```ignore
/// DockArea::new("dock").show(ui, &mut self.dock, &mut MyViewer { .. });
/// ```
#[derive(Clone, Debug)]
pub struct DockArea {
    id_salt: Id,
    add_button: bool,
}

impl DockArea {
    /// A dock area identified by `id_salt` (unique within its Ui).
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            add_button: false,
        }
    }

    /// Show a "+" button in every tab bar (see [`DockViewer::add_tab`]).
    pub fn add_button(mut self, add: bool) -> Self {
        self.add_button = add;
        self
    }

    /// Shows the layout in all the space left in `ui`.
    pub fn show<V: DockViewer>(
        self,
        ui: &mut Ui<'_>,
        state: &mut DockState<V::Tab>,
        viewer: &mut V,
    ) {
        let id = ui.id().with(self.id_salt);
        let rect = ui.available_rect();
        let rect = Rect::from_min_size(rect.min, vec2(rect.width(), rect.height().max(0.0)));
        ui.allocate_rect(rect.size());

        let mut pass = Pass {
            id,
            add_button: self.add_button,
            groups: Vec::new(),
            drag: None,
            drag_title: String::new(),
            dropped: None,
            closes: Vec::new(),
            adds: Vec::new(),
        };
        let mut root = std::mem::replace(&mut state.root, placeholder());
        pass.node(ui, &mut root, rect, viewer, &mut Vec::new());
        state.root = root;

        // A tab is being dragged: show where it would go.
        if let Some((_, _, pos)) = pass.drag {
            if let Some((target, preview)) = pass.target(pos) {
                let accent = ui.style().visuals.accent;
                match target {
                    DropTarget::Insert { .. } => {
                        ui.painter().rect_filled(preview, 1.0, accent);
                    }
                    DropTarget::Split { .. } => {
                        ui.painter().rect(
                            preview,
                            4.0,
                            accent.with_alpha(0.25),
                            Stroke::new(2.0, accent),
                        );
                    }
                }
            }
            // The dragged tab follows the pointer.
            let style = ui.style();
            let galley = ui.layout_text(&pass.drag_title, &style.body, None);
            let ghost = Rect::from_min_size(pos + vec2(14.0, 10.0), galley.size + vec2(20.0, 10.0));
            let painter = ui.painter();
            painter.rect(
                ghost,
                style.visuals.small_corner_radius,
                style.visuals.window_fill.with_alpha(0.9),
                Stroke::new(1.0, style.visuals.accent),
            );
            painter.galley(ghost.min + vec2(10.0, 5.0), galley, style.visuals.text);
        }
        if let Some((group, index, pos)) = pass.dropped
            && let Some((target, _)) = pass.target(pos)
        {
            state.move_tab(group, index, target);
            ui.ctx().request_repaint();
        }
        for (group, index) in pass.closes.into_iter().rev() {
            if let Some(DockNode::Tabs { tabs, .. }) = find_group_mut(&mut state.root, group)
                && index < tabs.len()
                && viewer.on_close(&mut tabs[index])
            {
                state.take(group, index);
                state.collapse();
            }
        }
        for group in pass.adds {
            if let Some(tab) = viewer.add_tab() {
                state.push_tab(group, tab);
            }
        }
    }
}

/// Collected while laying out the tree.
struct Pass {
    id: Id,
    add_button: bool,
    /// Every group's tab bar and body, with its tab responses.
    groups: Vec<GroupLayout>,
    /// A tab dragged past the threshold: (group, index, pointer).
    drag: Option<(u64, usize, Point)>,
    /// Its title, shown next to the pointer.
    drag_title: String,
    dropped: Option<(u64, usize, Point)>,
    closes: Vec<(u64, usize)>,
    adds: Vec<u64>,
}

struct GroupLayout {
    group: u64,
    bar: Rect,
    body: Rect,
    tabs: Vec<crate::Response>,
}

impl Pass {
    fn node<V: DockViewer>(
        &mut self,
        ui: &mut Ui<'_>,
        node: &mut DockNode<V::Tab>,
        rect: Rect,
        viewer: &mut V,
        path: &mut Vec<bool>,
    ) {
        match node {
            DockNode::Split {
                axis,
                fraction,
                first,
                second,
            } => {
                let gap = 4.0;
                let (a, b, handle) = split_rects(rect, *axis, *fraction, gap);
                path.push(false);
                self.node(ui, first, a, viewer, path);
                path.pop();
                path.push(true);
                self.node(ui, second, b, viewer, path);
                path.pop();

                // The line between the two: drag to resize.
                let handle_id = self.id.with(("split", path.clone()));
                let r = ui.interact(handle_id, handle, Sense::POINTER_DRAG);
                let style = ui.style();
                let (cursor, along) = match axis {
                    SplitAxis::Horizontal => (CursorIcon::ResizeHorizontal, rect.width()),
                    SplitAxis::Vertical => (CursorIcon::ResizeVertical, rect.height()),
                };
                if r.hovered() || r.dragged() {
                    ui.ctx().set_cursor(cursor);
                }
                let color = if r.hovered() || r.dragged() {
                    style.visuals.accent
                } else {
                    style.visuals.window_stroke.color
                };
                let (p, q) = match axis {
                    SplitAxis::Horizontal => (
                        point(handle.center().x, handle.min.y),
                        point(handle.center().x, handle.max.y),
                    ),
                    SplitAxis::Vertical => (
                        point(handle.min.x, handle.center().y),
                        point(handle.max.x, handle.center().y),
                    ),
                };
                ui.painter().line(p, q, Stroke::new(1.0, color));
                if r.dragged() && along > 0.0 {
                    let delta = match axis {
                        SplitAxis::Horizontal => r.drag_delta().x,
                        SplitAxis::Vertical => r.drag_delta().y,
                    };
                    let min = (60.0 / along).min(0.45);
                    *fraction = (*fraction + delta / along).clamp(min, 1.0 - min);
                    ui.ctx().request_repaint();
                }
            }
            DockNode::Tabs { id, tabs, active } => {
                let group = *id;
                let style = ui.style();
                let bar_height = style.spacing.interact_height + 4.0;
                let bar = Rect::from_min_size(rect.min, vec2(rect.width(), bar_height));
                let body = Rect::from_min_max(point(rect.min.x, bar.max.y), rect.max);
                ui.painter().rect_filled(bar, 0.0, style.visuals.panel_fill);
                ui.painter()
                    .rect_filled(body, 0.0, style.visuals.window_fill);

                let labels: Vec<TabLabel> = tabs.iter().map(|t| viewer.label(t)).collect();
                let closable = tabs.iter().any(|t| viewer.closable(t));
                if !tabs.is_empty() {
                    *active = (*active).min(tabs.len() - 1);
                }
                let saved_clip = ui.clip_rect();
                ui.set_clip_rect(bar);
                let out = tab_strip(
                    ui,
                    self.id.with(("group", group)),
                    bar.expand(-2.0),
                    &labels,
                    (!tabs.is_empty()).then_some(*active),
                    closable,
                    self.add_button,
                );
                ui.clip_rect_restore(saved_clip);
                if let Some(i) = out.clicked {
                    *active = i;
                }
                if let Some(i) = out.close
                    && viewer.closable(&tabs[i])
                {
                    self.closes.push((group, i));
                }
                if out.add_clicked {
                    self.adds.push(group);
                }
                if let Some((i, pos)) = out.dragging {
                    self.drag = Some((group, i, pos));
                    self.drag_title = labels[i].title.clone();
                }
                if let Some((i, pos)) = out.dropped {
                    self.dropped = Some((group, i, pos));
                }
                for (i, tab_response) in out.tabs.iter().enumerate() {
                    let tab = &mut tabs[i];
                    tab_response.context_menu(ui, |ui| viewer.context_menu(ui, tab));
                }

                // The active tab's content.
                if let Some(tab) = tabs.get_mut(*active) {
                    let pad = style.spacing.window_padding;
                    let saved_clip = ui.clip_rect();
                    ui.set_clip_rect(body);
                    ui.push_id(("dock group", group), |ui| {
                        ui.scope_with_no_advance(
                            body.expand(-pad),
                            Layout::top_down(Align::Min),
                            |ui| viewer.ui(ui, tab),
                        );
                    });
                    ui.clip_rect_restore(saved_clip);
                }
                self.groups.push(GroupLayout {
                    group,
                    bar,
                    body,
                    tabs: out.tabs,
                });
            }
        }
    }

    /// The drop target under `pos` and the area to highlight.
    fn target(&self, pos: Point) -> Option<(DropTarget, Rect)> {
        for g in &self.groups {
            if g.bar.contains(pos) {
                let index = insertion_index(&g.tabs, pos.x);
                let x = if index < g.tabs.len() {
                    g.tabs[index].rect.min.x - 1.5
                } else {
                    g.tabs
                        .last()
                        .map_or(g.bar.min.x + 2.0, |t| t.rect.max.x + 1.5)
                };
                let line = Rect::from_min_max(
                    point(x - 1.0, g.bar.min.y + 4.0),
                    point(x + 1.0, g.bar.max.y - 4.0),
                );
                return Some((
                    DropTarget::Insert {
                        group: g.group,
                        index,
                    },
                    line,
                ));
            }
            if g.body.contains(pos) {
                let b = g.body;
                let rel = vec2(
                    (pos.x - b.min.x) / b.width().max(1.0),
                    (pos.y - b.min.y) / b.height().max(1.0),
                );
                // The middle adds the tab to the group; the sides split it.
                let edge = [
                    (rel.x, SplitAxis::Horizontal, true),
                    (1.0 - rel.x, SplitAxis::Horizontal, false),
                    (rel.y, SplitAxis::Vertical, true),
                    (1.0 - rel.y, SplitAxis::Vertical, false),
                ]
                .into_iter()
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .expect("four edges");
                if edge.0 > 0.25 {
                    return Some((
                        DropTarget::Insert {
                            group: g.group,
                            index: g.tabs.len(),
                        },
                        b.expand(-4.0),
                    ));
                }
                let (_, axis, first) = edge;
                let (a, c, _) = split_rects(b, axis, 0.5, 0.0);
                let preview = if first { a } else { c };
                return Some((
                    DropTarget::Split {
                        group: g.group,
                        axis,
                        first,
                    },
                    preview.expand(-4.0),
                ));
            }
        }
        None
    }
}

/// The two parts of `rect` and the handle between them.
fn split_rects(rect: Rect, axis: SplitAxis, fraction: f32, gap: f32) -> (Rect, Rect, Rect) {
    match axis {
        SplitAxis::Horizontal => {
            let x = rect.min.x + rect.width() * fraction;
            (
                Rect::from_min_max(rect.min, point(x - gap / 2.0, rect.max.y)),
                Rect::from_min_max(point(x + gap / 2.0, rect.min.y), rect.max),
                Rect::from_min_max(point(x - 3.0, rect.min.y), point(x + 3.0, rect.max.y)),
            )
        }
        SplitAxis::Vertical => {
            let y = rect.min.y + rect.height() * fraction;
            (
                Rect::from_min_max(rect.min, point(rect.max.x, y - gap / 2.0)),
                Rect::from_min_max(point(rect.min.x, y + gap / 2.0), rect.max),
                Rect::from_min_max(point(rect.min.x, y - 3.0), point(rect.max.x, y + 3.0)),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> DockState<&'static str> {
        let mut state = DockState::new(vec!["Scene", "Assembly"]);
        let root = state.groups()[0];
        let right = state
            .split(root, SplitAxis::Horizontal, false, 0.3, vec!["Properties"])
            .unwrap();
        state.split(right, SplitAxis::Vertical, false, 0.4, vec!["Log"]);
        state
    }

    #[test]
    fn building_and_moving_tabs() {
        let mut state = layout();
        assert_eq!(state.tabs(), [&"Scene", &"Assembly", &"Properties", &"Log"]);
        assert_eq!(state.groups().len(), 3);
        let log = state.find_group(|t| *t == "Log").unwrap();
        let scene = state.find_group(|t| *t == "Scene").unwrap();

        // Log into the first group: its group disappears.
        state.move_tab(
            log,
            0,
            DropTarget::Insert {
                group: scene,
                index: 1,
            },
        );
        assert_eq!(state.tabs(), [&"Scene", &"Log", &"Assembly", &"Properties"]);
        assert_eq!(state.groups().len(), 2);

        // Reorder within the group.
        state.move_tab(
            scene,
            0,
            DropTarget::Insert {
                group: scene,
                index: 3,
            },
        );
        assert_eq!(state.tabs(), [&"Log", &"Assembly", &"Scene", &"Properties"]);

        // Split the group with one of its tabs, below it.
        state.move_tab(
            scene,
            2,
            DropTarget::Split {
                group: scene,
                axis: SplitAxis::Vertical,
                first: false,
            },
        );
        assert_eq!(state.groups().len(), 3);
        assert_ne!(state.find_group(|t| *t == "Scene"), Some(scene));

        assert_eq!(state.remove_tab(|t| *t == "Properties"), Some("Properties"));
        assert_eq!(state.groups().len(), 2);
    }

    #[test]
    fn splitting_a_lone_tab_onto_itself_is_a_no_op() {
        let mut state = layout();
        let props = state.find_group(|t| *t == "Properties").unwrap();
        let before = state.clone();
        state.move_tab(
            props,
            0,
            DropTarget::Split {
                group: props,
                axis: SplitAxis::Horizontal,
                first: true,
            },
        );
        assert_eq!(state, before);
    }
}
