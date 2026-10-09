//! Trees: nested rows that expand and collapse, laid out only while
//! visible.

use std::collections::HashSet;
use std::hash::{BuildHasher, Hash};

use rustroke_core::{Key, Modifiers, Rect, Stroke, point, vec2};

use crate::{Align, Id, Layout, Response, ScrollArea, Sense, Ui, WidgetInfo, WidgetRole};

/// What [`Tree::show`] returns.
#[derive(Clone, Debug)]
pub struct TreeResponse<N> {
    /// Covers the tree; has focus while the arrow keys move the
    /// selection.
    pub response: Response,
    /// The node clicked this frame.
    pub clicked: Option<N>,
    /// The node double-clicked this frame (e.g. to open or rename it).
    pub double_clicked: Option<N>,
    /// The selection changed this frame.
    pub selection_changed: bool,
    /// The visible rows laid out this frame, with their responses (for
    /// context menus or tooltips).
    pub rows: Vec<(N, Response)>,
}

/// A row of the flattened tree.
#[derive(Clone, Copy)]
struct Row<N> {
    node: N,
    depth: usize,
    has_children: bool,
    open: bool,
}

/// A tree of nodes that expand and collapse with the arrow in front of
/// them (or →/← and double-click), with one selected node moved by clicks
/// and the arrow keys. The nodes belong to the app: give the roots and a
/// function returning a node's children; nodes are small `Copy` ids
/// (indices, keys). Only the visible rows are laid out, so trees with
/// huge numbers of nodes stay fast while their branches are collapsed.
///
/// ```ignore
/// Tree::new("model").show(
///     ui,
///     &roots,
///     |n| model.children(n),
///     &mut selected,
///     |ui, n| { ui.label(model.name(n)); },
/// );
/// ```
#[derive(Clone, Debug)]
pub struct Tree {
    id_salt: Id,
    row_height: Option<f32>,
    max_height: f32,
    open_depth: usize,
    accessible_label: String,
}

impl Tree {
    /// A tree identified by `id_salt` (unique within its Ui).
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            row_height: None,
            max_height: f32::INFINITY,
            open_depth: 0,
            accessible_label: String::new(),
        }
    }

    /// Height of every row (default: the style's `interact_height`).
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// The tree scrolls when taller than this (it is also limited by the
    /// available height).
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// Nodes less than `depth` levels deep start open (default 0: all
    /// closed; 1: the roots open).
    pub fn default_open_depth(mut self, depth: usize) -> Self {
        self.open_depth = depth;
        self
    }

    /// The name screen readers announce for the tree.
    pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
        self.accessible_label = label.into();
        self
    }

    /// Shows the tree. `children(node)` lists a node's children (called
    /// for open nodes and to know whether a node has any); `add_row(ui,
    /// node)` draws a visible row after its indentation and arrow.
    pub fn show<N, C>(
        self,
        ui: &mut Ui<'_>,
        roots: &[N],
        children: impl Fn(N) -> C,
        selection: &mut Option<N>,
        mut add_row: impl FnMut(&mut Ui<'_>, N),
    ) -> TreeResponse<N>
    where
        N: Copy + Hash + Eq,
        C: IntoIterator<Item = N>,
    {
        let style = ui.style();
        let id = ui.id().with(self.id_salt);
        let toggled_key = id.with("toggled");
        let row_h = self.row_height.unwrap_or(style.spacing.interact_height);
        let indent = style.spacing.indent;
        let hasher =
            std::hash::BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default();
        let key = |n: N| hasher.hash_one(n);
        // Nodes whose open state differs from the default.
        let mut toggled: HashSet<u64> = ui.ctx().data_persisted(toggled_key).unwrap_or_default();
        let before_toggled = toggled.clone();
        let is_open = |n: N, depth: usize, toggled: &HashSet<u64>| {
            (depth < self.open_depth) != toggled.contains(&key(n))
        };

        // Flatten the open part of the tree.
        let flatten = |toggled: &HashSet<u64>| {
            let mut rows = Vec::new();
            let mut stack: Vec<(N, usize)> = roots.iter().rev().map(|&n| (n, 0)).collect();
            while let Some((node, depth)) = stack.pop() {
                let open = is_open(node, depth, toggled);
                let kids: Vec<N> = children(node).into_iter().collect();
                rows.push(Row {
                    node,
                    depth,
                    has_children: !kids.is_empty(),
                    open: open && !kids.is_empty(),
                });
                if open {
                    stack.extend(kids.into_iter().rev().map(|k| (k, depth + 1)));
                }
            }
            rows
        };
        let mut rows = flatten(&toggled);

        // The tree takes keyboard focus; rows win clicks.
        let prev_rect: Option<Rect> = ui.ctx().data(id.with("rect"));
        let start = ui.available_rect().min;
        let tree_rect = prev_rect.map_or(Rect::from_min_size(start, vec2(0.0, 0.0)), |r| {
            Rect::from_min_size(start, r.size())
        });
        let mut tree_response = ui.interact(id, tree_rect, Sense::CLICK);
        ui.describe(
            &tree_response,
            WidgetInfo::new(WidgetRole::List, self.accessible_label.clone()),
        );

        // Keys.
        let before = *selection;
        let mut reveal = None;
        let position =
            |rows: &[Row<N>], n: Option<N>| n.and_then(|n| rows.iter().position(|r| r.node == n));
        if tree_response.has_focus() && !rows.is_empty() {
            let current = position(&rows, *selection);
            let last = rows.len() - 1;
            let input = ui.ctx().input_mut();
            let pressed =
                |input: &mut rustroke_core::InputState, k| input.consume_key(k, Modifiers::NONE);
            let mut to = None;
            if pressed(input, Key::ArrowDown) {
                to = Some(current.map_or(0, |c| (c + 1).min(last)));
            } else if pressed(input, Key::ArrowUp) {
                to = Some(current.map_or(0, |c| c.saturating_sub(1)));
            } else if pressed(input, Key::Home) {
                to = Some(0);
            } else if pressed(input, Key::End) {
                to = Some(last);
            } else if let Some(c) = current {
                let row = rows[c];
                if pressed(input, Key::ArrowRight) {
                    if row.has_children && !row.open {
                        toggle(&mut toggled, key(row.node));
                    } else if row.open {
                        to = Some(c + 1);
                    }
                } else if pressed(input, Key::ArrowLeft) {
                    if row.open {
                        toggle(&mut toggled, key(row.node));
                    } else if let Some(parent) = rows[..c].iter().rposition(|r| r.depth < row.depth)
                    {
                        to = Some(parent);
                    }
                }
            }
            if let Some(to) = to {
                *selection = Some(rows[to].node);
                reveal = Some(to);
            }
            if toggled != before_toggled {
                rows = flatten(&toggled);
            }
        }

        // Rows, only the visible ones.
        let mut clicked = None;
        let mut double_clicked = None;
        let mut toggle_now = None;
        let mut row_responses = Vec::new();
        let width = ui.available_width();
        let saved_spacing = ui.style().spacing.item_spacing;
        ui.style_mut().spacing.item_spacing.y = 0.0;
        let area = ScrollArea::vertical()
            .max_height(self.max_height)
            .id_salt(id.with("scroll"))
            .show_rows(ui, row_h, rows.len(), |ui, range| {
                let top = ui.max_rect().min.y;
                if let Some(r) = reveal {
                    let y = top + r as f32 * row_h;
                    let rect = Rect::from_min_size(point(ui.max_rect().min.x, y), vec2(1.0, row_h));
                    ui.scroll_to_rect(rect, None);
                }
                for index in range {
                    let row = rows[index];
                    let y = top + index as f32 * row_h;
                    let rect =
                        Rect::from_min_size(point(ui.max_rect().min.x, y), vec2(width, row_h));
                    let row_id = id.with(("row", key(row.node)));
                    let sense = Sense {
                        focusable: false,
                        activate_with_keys: false,
                        ..Sense::CLICK
                    };
                    let response = ui.interact(row_id, rect, sense);
                    let selected = *selection == Some(row.node);
                    let mut info =
                        WidgetInfo::new(WidgetRole::SelectableItem, format!("Row {}", index + 1))
                            .selected(selected);
                    if row.has_children {
                        info = info.expanded(row.open);
                    }
                    ui.describe(&response, info);
                    let visuals = &ui.style().visuals;
                    let radius = visuals.small_corner_radius;
                    if selected {
                        let fill = visuals.selection;
                        ui.painter().rect_filled(rect, radius, fill);
                    } else if response.hovered() {
                        let fill = ui.widget_visuals(&response).bg_fill.with_alpha(0.5);
                        ui.painter().rect_filled(rect, radius, fill);
                    }
                    // Arrow.
                    let arrow_x = rect.min.x + row.depth as f32 * indent;
                    let arrow_rect = Rect::from_min_size(point(arrow_x, y), vec2(indent, row_h));
                    if row.has_children {
                        let arrow = ui.interact(row_id.with("arrow"), arrow_rect, sense);
                        if arrow.clicked() {
                            toggle_now = Some(row.node);
                        }
                        let c = arrow_rect.center();
                        let color = if arrow.hovered() {
                            ui.style().visuals.text
                        } else {
                            ui.style().visuals.weak_text
                        };
                        let points = if row.open {
                            vec![
                                c + vec2(-4.0, -2.0),
                                c + vec2(0.0, 2.0),
                                c + vec2(4.0, -2.0),
                            ]
                        } else {
                            vec![
                                c + vec2(-2.0, -4.0),
                                c + vec2(2.0, 0.0),
                                c + vec2(-2.0, 4.0),
                            ]
                        };
                        ui.painter().polyline(points, Stroke::new(1.5, color));
                    }
                    let content = Rect::from_min_max(
                        point(arrow_rect.max.x, y),
                        point(rect.max.x - 4.0, y + row_h),
                    );
                    ui.push_id(("tree row", key(row.node)), |ui| {
                        ui.scope_with_no_advance(
                            content,
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                add_row(ui, row.node);
                            },
                        );
                    });
                    if response.clicked() {
                        clicked = Some(row.node);
                    }
                    if response.double_clicked() {
                        double_clicked = Some(row.node);
                        if row.has_children {
                            toggle_now = Some(row.node);
                        }
                    }
                    row_responses.push((row.node, response));
                }
            });
        ui.style_mut().spacing.item_spacing = saved_spacing;
        if let Some(node) = toggle_now {
            toggle(&mut toggled, key(node));
        }
        if let Some(node) = clicked {
            *selection = Some(node);
            ui.ctx().request_focus(id);
            tree_response.has_focus = true;
        }
        if toggled != before_toggled {
            ui.ctx().insert_persisted(toggled_key, toggled);
            ui.ctx().request_repaint();
        }
        let rect = area.response.rect;
        if prev_rect != Some(rect) {
            ui.ctx().insert_data(id.with("rect"), rect);
        }
        if tree_response.focus_visible() {
            let style = ui.style();
            ui.painter().rect_stroke(
                rect.expand(2.0),
                style.visuals.small_corner_radius + 2.0,
                Stroke::new(2.0, style.visuals.focus),
            );
        }
        tree_response.rect = rect;
        let selection_changed = *selection != before;
        if selection_changed {
            tree_response.mark_changed();
        }
        TreeResponse {
            response: tree_response,
            clicked,
            double_clicked,
            selection_changed,
            rows: row_responses,
        }
    }
}

/// Adds `key` to the set, or removes it if present.
fn toggle(set: &mut HashSet<u64>, key: u64) {
    if !set.remove(&key) {
        set.insert(key);
    }
}
