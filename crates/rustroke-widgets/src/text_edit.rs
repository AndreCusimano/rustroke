//! Editable text fields.

use rustroke_core::{Event, Galley, ImeEvent, Key, Modifiers, Point, Rect, Stroke, point, vec2};

use crate::{CursorIcon, FocusLost, Id, Response, Sense, Ui, Widget, WidgetInfo, WidgetRole};

/// Seconds the text cursor stays visible, then hidden, while blinking.
const BLINK_HALF_PERIOD: f64 = 0.5;

/// Cursor, selection and scroll of a text field, kept between frames.
#[derive(Clone, Debug, Default)]
struct TextEditState {
    /// Byte index of the cursor.
    cursor: usize,
    /// Other end of the selection (equal to `cursor` when nothing is selected).
    anchor: usize,
    /// Horizontal scroll of single-line fields, in points.
    scroll_x: f32,
    /// Column to aim for when moving up/down through rows of different length.
    preferred_x: Option<f32>,
    /// Text being composed with an input method, shown at the cursor.
    preedit: String,
    /// When the cursor last moved (restarts the blink).
    last_change: f64,
    /// The text when the field got focus, restored by Escape. `None`
    /// while the field is not focused.
    original: Option<String>,
}

impl TextEditState {
    fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    /// Moves the cursor; with `extend` the selection grows, otherwise it collapses.
    fn move_to(&mut self, index: usize, extend: bool) {
        self.cursor = index;
        if !extend {
            self.anchor = index;
        }
    }
}

/// Most undo steps kept per field.
const UNDO_LIMIT: usize = 100;

/// Typing or deleting within this many seconds of the previous edit is
/// undone together.
const UNDO_GROUP_SECONDS: f64 = 1.0;

/// Text and selection at one point of the edit history.
#[derive(Clone, Debug)]
struct Snapshot {
    text: String,
    cursor: usize,
    anchor: usize,
}

impl Snapshot {
    fn of(text: &str, state: &TextEditState) -> Self {
        Self {
            text: text.to_owned(),
            cursor: state.cursor,
            anchor: state.anchor,
        }
    }
}

/// Kinds of edits; consecutive typing (or deleting) is undone as one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditKind {
    Typing,
    Deleting,
    Other,
}

/// Undo and redo history of a field, for one editing session (cleared
/// when the field gets focus). Kept apart from [`TextEditState`] and only
/// loaded while editing, since it holds copies of the text.
#[derive(Clone, Debug, Default)]
struct UndoHistory {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last: Option<(EditKind, f64)>,
}

impl UndoHistory {
    /// Records the state `before` an edit of `kind` made at `now`.
    fn record(&mut self, before: Snapshot, kind: EditKind, now: f64) {
        let grouped = kind != EditKind::Other
            && self
                .last
                .is_some_and(|(k, t)| k == kind && now - t < UNDO_GROUP_SECONDS);
        if !grouped {
            self.undo.push(before);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = Some((kind, now));
    }

    /// Undoes (or redoes) one step. Returns whether the text changed.
    fn step(&mut self, text: &mut String, state: &mut TextEditState, redo: bool) -> bool {
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let Some(snapshot) = from.pop() else {
            return false;
        };
        to.push(Snapshot::of(text, state));
        *text = snapshot.text;
        state.cursor = clamp_to_boundary(text, snapshot.cursor);
        state.anchor = clamp_to_boundary(text, snapshot.anchor);
        state.preferred_x = None;
        self.last = None;
        true
    }
}

/// `Some(false)` for the undo shortcut (Cmd/Ctrl+Z), `Some(true)` for
/// redo (Cmd/Ctrl+Shift+Z, or Ctrl+Y outside macOS).
fn undo_shortcut(key: Key, modifiers: Modifiers) -> Option<bool> {
    let shift_command = modifiers == Modifiers::COMMAND.plus(Modifiers::SHIFT);
    match key {
        Key::Z if modifiers.command_only() => Some(false),
        Key::Z if shift_command => Some(true),
        Key::Y if !cfg!(target_os = "macos") && modifiers.command_only() => Some(true),
        _ => None,
    }
}

/// What kind of edit `event` would make, if any.
fn edit_kind(event: &Event, multiline: bool) -> Option<EditKind> {
    match event {
        Event::Text(_) | Event::Ime(ImeEvent::Commit(_)) => Some(EditKind::Typing),
        Event::Paste(_) | Event::Cut => Some(EditKind::Other),
        Event::Key {
            key: Key::Backspace | Key::Delete,
            pressed: true,
            ..
        } => Some(EditKind::Deleting),
        Event::Key {
            key: Key::Enter,
            pressed: true,
            ..
        } if multiline => Some(EditKind::Other),
        _ => None,
    }
}

/// A field to edit a `String`: single-line (e.g. a name) or multi-line
/// (wraps and grows with its content).
///
/// Mouse: click to place the cursor, drag or Shift+click to select.
/// Keyboard: arrows, Home/End, Alt/Ctrl+arrows by word, Shift to select,
/// Backspace/Delete, Cmd/Ctrl+A/C/X/V, Cmd/Ctrl+Z to undo and
/// Cmd/Ctrl+Shift+Z (or Ctrl+Y) to redo. Enter in a single-line field ends
/// editing ([`FocusLost::Submit`]); Escape restores the text the field had
/// when it got focus and ends editing ([`FocusLost::Cancel`]). See
/// [`Response::lost_focus_reason`].
#[derive(Debug)]
pub struct TextEdit<'t> {
    text: &'t mut String,
    multiline: bool,
    hint: String,
    desired_width: Option<f32>,
    desired_rows: usize,
    accessible_label: Option<String>,
    id: Option<Id>,
    select_all_on_focus: bool,
}

impl<'t> TextEdit<'t> {
    /// A one-line field: newlines are not allowed, Enter ends editing.
    pub fn singleline(text: &'t mut String) -> Self {
        Self {
            text,
            multiline: false,
            hint: String::new(),
            desired_width: None,
            desired_rows: 1,
            accessible_label: None,
            id: None,
            select_all_on_focus: false,
        }
    }

    /// A multi-line field that wraps and grows with its content.
    pub fn multiline(text: &'t mut String) -> Self {
        Self {
            multiline: true,
            desired_rows: 4,
            ..Self::singleline(text)
        }
    }

    /// The name screen readers announce (and tests find the widget by),
    /// instead of the visible text. Useful when the text is empty.
    pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Shown in a weak color while the field is empty and not focused.
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Width in points (defaults: 240 for single-line, all available
    /// width for multi-line).
    pub fn desired_width(mut self, width: f32) -> Self {
        self.desired_width = Some(width);
        self
    }

    /// A fixed id instead of an automatic one, e.g. to give the field focus
    /// with `Context::request_focus`.
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Selects the whole text when the field gets focus (by click, Tab or
    /// `Context::request_focus`), so typing replaces it.
    pub fn select_all_on_focus(mut self, select_all: bool) -> Self {
        self.select_all_on_focus = select_all;
        self
    }

    /// Minimum height of a multi-line field, in rows of text.
    pub fn desired_rows(mut self, rows: usize) -> Self {
        self.desired_rows = rows.max(1);
        self
    }
}

impl Widget for TextEdit<'_> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let visuals = &style.visuals;
        let padding = vec2(8.0, 4.0);
        let available = ui.available_width();
        let width = self
            .desired_width
            .unwrap_or(if self.multiline {
                ui.fill_width(480.0)
            } else {
                240.0
            })
            .min(available)
            .max(40.0);
        let inner_width = width - 2.0 * padding.x;
        let row_height = style.body.size * style.body.line_height;
        let wrap = self.multiline.then_some(inner_width);

        let id = self.id.unwrap_or_else(|| ui.next_auto_id());
        let mut state: TextEditState = ui.ctx().data(id).unwrap_or_default();
        state.cursor = clamp_to_boundary(self.text, state.cursor);
        state.anchor = clamp_to_boundary(self.text, state.anchor);

        // Size from the current text (before this frame's edits).
        let galley = ui.layout_text(self.text, &style.body, wrap);
        let content_height = if self.multiline {
            galley.size.y.max(self.desired_rows as f32 * row_height)
        } else {
            row_height
        };
        let height = (content_height + 2.0 * padding.y).max(style.spacing.interact_height);
        let rect = ui.allocate_rect(vec2(width, height));
        let mut response = ui.interact(id, rect, Sense::TEXT);
        let role = if self.multiline {
            WidgetRole::MultilineTextInput
        } else {
            WidgetRole::TextInput
        };
        ui.describe(
            &response,
            WidgetInfo::new(
                role,
                self.accessible_label
                    .clone()
                    .unwrap_or_else(|| self.hint.clone()),
            )
            .value(self.text.clone()),
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor(CursorIcon::Text);
        }
        let text_origin = |scroll_x: f32| {
            let y = if self.multiline {
                rect.min.y + padding.y
            } else {
                rect.center().y - row_height / 2.0
            };
            point(rect.min.x + padding.x - scroll_x, y)
        };

        let now = ui.input().time;
        let mut changed = false;
        let old_cursor = state.cursor;

        let gained_focus = response.has_focus() && state.original.is_none();
        if !response.has_focus() && state.original.take().is_some() {
            // Focus moved away (Tab, click elsewhere): keep the edits.
            response.lost_focus = Some(FocusLost::Other);
        }
        let undo_id = id.with("undo");
        if gained_focus {
            state.original = Some(self.text.clone());
            ui.ctx().remove_data(undo_id);
        }

        // Mouse: place the cursor and select by dragging.
        if (response.drag_started() || response.dragged())
            && let Some(pos) = ui.input().pointer.pos()
        {
            let index = galley.index_at(Point::new(0.0, 0.0) + (pos - text_origin(state.scroll_x)));
            let extend =
                response.dragged() && !response.drag_started() || ui.input().modifiers.shift;
            state.move_to(index, extend);
            state.preferred_x = None;
        }

        if gained_focus && self.select_all_on_focus {
            state.anchor = 0;
            state.cursor = self.text.len();
        }

        // Keyboard and text input, in the order they happened.
        if response.has_focus() {
            ui.ctx().set_keyboard_owner(id);
            let events = ui.input().events.clone();
            let mut galley = std::sync::Arc::clone(&galley);
            let mut history: Option<UndoHistory> = None;
            for event in events {
                if let Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                    && let Some(redo) = undo_shortcut(key, modifiers)
                {
                    // The app's own undo must not run too.
                    ui.ctx().input_mut().consume_key(key, modifiers);
                    let history =
                        history.get_or_insert_with(|| ui.ctx().data(undo_id).unwrap_or_default());
                    if history.step(self.text, &mut state, redo) {
                        changed = true;
                        galley = ui.layout_text(self.text, &style.body, wrap);
                    }
                    continue;
                }
                let kind = edit_kind(&event, self.multiline);
                let before = kind.map(|_| Snapshot::of(self.text, &state));
                let edited = match event {
                    Event::Text(text) => insert(self.text, &mut state, &text, self.multiline),
                    Event::Paste(text) => insert(self.text, &mut state, &text, self.multiline),
                    Event::Copy | Event::Cut => {
                        if state.has_selection() {
                            let (a, b) = state.selection();
                            ui.ctx().copy_text(self.text[a..b].to_owned());
                        }
                        matches!(event, Event::Cut) && delete_selection(self.text, &mut state)
                    }
                    Event::Ime(ImeEvent::Preedit(text)) => {
                        delete_selection(self.text, &mut state);
                        state.preedit = text;
                        true
                    }
                    Event::Ime(ImeEvent::Commit(text)) => {
                        state.preedit.clear();
                        insert(self.text, &mut state, &text, self.multiline);
                        true
                    }
                    Event::Ime(ImeEvent::Disabled) => {
                        state.preedit.clear();
                        false
                    }
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => match on_key(
                        self.text,
                        &mut state,
                        &galley,
                        key,
                        modifiers,
                        self.multiline,
                    ) {
                        KeyResult::Edited => true,
                        KeyResult::Moved | KeyResult::Ignored => false,
                        KeyResult::Submit => {
                            ui.ctx().clear_focus();
                            state.original = None;
                            response.lost_focus = Some(FocusLost::Submit);
                            false
                        }
                        KeyResult::Cancel => {
                            ui.ctx().clear_focus();
                            response.lost_focus = Some(FocusLost::Cancel);
                            match state.original.take() {
                                Some(original) if original != *self.text => {
                                    *self.text = original;
                                    state.move_to(self.text.len(), false);
                                    true
                                }
                                _ => false,
                            }
                        }
                    },
                    _ => false,
                };
                if let (Some(kind), Some(before)) = (kind, before)
                    && before.text != *self.text
                    && response.lost_focus.is_none()
                {
                    let history =
                        history.get_or_insert_with(|| ui.ctx().data(undo_id).unwrap_or_default());
                    history.record(before, kind, now);
                }
                if edited {
                    changed = true;
                    // Later keys (e.g. arrows after typing) need the new layout.
                    galley = ui.layout_text(self.text, &style.body, wrap);
                }
            }
            if let Some(history) = history {
                ui.ctx().insert_data(undo_id, history);
            }
        }
        if changed || state.cursor != old_cursor {
            state.last_change = now;
        }
        if !response.has_focus() || response.lost_focus() {
            // A composition can't continue in a field that lost focus.
            state.preedit.clear();
        }

        // What to draw: the text, with the IME composition at the cursor.
        let mut display = self.text.clone();
        if !state.preedit.is_empty() {
            display.insert_str(state.cursor, &state.preedit);
        }
        let galley = ui.layout_text(&display, &style.body, wrap);

        // Single-line fields scroll sideways to keep the cursor visible.
        if !self.multiline {
            let cursor_x = galley.cursor_rect(state.cursor + state.preedit.len()).min.x;
            if cursor_x - state.scroll_x > inner_width {
                state.scroll_x = cursor_x - inner_width;
            } else if cursor_x < state.scroll_x {
                state.scroll_x = cursor_x;
            }
            state.scroll_x = state
                .scroll_x
                .clamp(0.0, (galley.size.x - inner_width).max(0.0));
        }
        let origin = text_origin(state.scroll_x);
        let has_focus = ui.ctx().focused() == Some(id);

        // Frame.
        let stroke = if has_focus {
            Stroke::new(1.5, visuals.accent)
        } else {
            ui.widget_visuals(&response).stroke
        };
        ui.painter()
            .rect(rect, visuals.corner_radius, visuals.text_field_fill, stroke);

        let saved_clip = ui.clip_rect();
        ui.set_clip_rect(rect.expand(-1.0));
        if has_focus {
            let selection = visuals.selection;
            let (a, b) = state.selection();
            for r in galley.selection_rects(a, b) {
                ui.painter().rect_filled(offset(r, origin), 0.0, selection);
            }
        }
        if self.text.is_empty() && state.preedit.is_empty() && !has_focus && !self.hint.is_empty() {
            let hint = ui.layout_text(&self.hint, &style.body, wrap);
            ui.painter().galley(origin, hint, visuals.weak_text);
        } else {
            ui.painter()
                .galley(origin, std::sync::Arc::clone(&galley), visuals.text);
        }
        if !state.preedit.is_empty() {
            // Underline the text being composed.
            let start = galley.cursor_rect(state.cursor);
            let end = galley.cursor_rect(state.cursor + state.preedit.len());
            let y = start.max.y - 2.0;
            ui.painter().line(
                point(origin.x + start.min.x, origin.y + y),
                point(origin.x + end.min.x, origin.y + y),
                Stroke::new(1.0, visuals.text),
            );
        }
        if has_focus {
            let caret = offset(
                galley.cursor_rect(state.cursor + state.preedit.len()),
                origin,
            );
            ui.ctx().set_ime_cursor(caret);
            // Blink, restarting whenever the cursor moves.
            let phase = (now - state.last_change) % (2.0 * BLINK_HALF_PERIOD);
            if phase < BLINK_HALF_PERIOD {
                let line = Rect::from_min_max(
                    caret.min - vec2(0.75, 0.0),
                    point(caret.min.x + 0.75, caret.max.y),
                );
                ui.painter().rect_filled(line, 0.0, visuals.text);
            }
            let next_toggle = BLINK_HALF_PERIOD - phase % BLINK_HALF_PERIOD;
            ui.ctx().request_repaint_after(next_toggle);
        }
        ui.clip_rect_restore(saved_clip);

        if changed {
            response.mark_changed();
            ui.ctx().request_repaint();
        }
        ui.ctx().insert_data(id, state);
        response
    }
}

fn offset(r: Rect, by: Point) -> Rect {
    Rect::from_min_max(by + r.min.to_vec2(), by + r.max.to_vec2())
}

/// The nearest character boundary at or before `index`.
fn clamp_to_boundary(text: &str, index: usize) -> usize {
    let mut i = index.min(text.len());
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Replaces the selection with `insert`. Returns whether the text changed.
fn insert(text: &mut String, state: &mut TextEditState, insert: &str, multiline: bool) -> bool {
    let filtered: String = if multiline {
        insert.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        insert
            .chars()
            .filter(|c| *c != '\n' && *c != '\r')
            .collect()
    };
    let filtered: String = filtered
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    if filtered.is_empty() {
        return false;
    }
    delete_selection(text, state);
    text.insert_str(state.cursor, &filtered);
    state.move_to(state.cursor + filtered.len(), false);
    state.preferred_x = None;
    true
}

/// Deletes the selected text. Returns whether anything was deleted.
fn delete_selection(text: &mut String, state: &mut TextEditState) -> bool {
    if !state.has_selection() {
        return false;
    }
    let (a, b) = state.selection();
    text.replace_range(a..b, "");
    state.move_to(a, false);
    true
}

enum KeyResult {
    Edited,
    Moved,
    Submit,
    Cancel,
    Ignored,
}

/// All cursor positions of the galley, in text order.
fn caret_indices(galley: &Galley) -> Vec<usize> {
    let mut all: Vec<usize> = galley
        .rows
        .iter()
        .flat_map(|r| r.carets.iter().map(|c| c.0))
        .collect();
    all.sort_unstable();
    all.dedup();
    all
}

fn next_caret(galley: &Galley, index: usize) -> Option<usize> {
    caret_indices(galley).into_iter().find(|&i| i > index)
}

fn prev_caret(galley: &Galley, index: usize) -> Option<usize> {
    caret_indices(galley).into_iter().rev().find(|&i| i < index)
}

/// Start of the next word after `index` (or the end of the text).
fn next_word(text: &str, index: usize) -> usize {
    let rest = &text[index..];
    let mut chars = rest.char_indices().peekable();
    // Skip the current word, then the spaces after it.
    while chars.peek().is_some_and(|(_, c)| c.is_alphanumeric()) {
        chars.next();
    }
    while chars.peek().is_some_and(|(_, c)| !c.is_alphanumeric()) {
        chars.next();
    }
    chars.peek().map_or(text.len(), |(i, _)| index + i)
}

/// Start of the word before `index` (or the start of the text).
fn prev_word(text: &str, index: usize) -> usize {
    let before = &text[..index];
    let mut chars = before.char_indices().rev().peekable();
    while chars.peek().is_some_and(|(_, c)| !c.is_alphanumeric()) {
        chars.next();
    }
    let mut start = chars.peek().map_or(0, |(i, _)| *i);
    for (i, c) in chars {
        if !c.is_alphanumeric() {
            break;
        }
        start = i;
    }
    start
}

fn on_key(
    text: &mut String,
    state: &mut TextEditState,
    galley: &Galley,
    key: Key,
    modifiers: Modifiers,
    multiline: bool,
) -> KeyResult {
    let shift = modifiers.shift;
    // Word-wise movement: Option on macOS, Ctrl elsewhere.
    let word = if cfg!(target_os = "macos") {
        modifiers.alt
    } else {
        modifiers.ctrl
    };
    let row = galley.row_of(state.cursor);
    let mut vertical = false;
    let result = match key {
        Key::A if modifiers.command_only() => {
            state.anchor = 0;
            state.cursor = text.len();
            KeyResult::Moved
        }
        Key::ArrowLeft => {
            let target = if state.has_selection() && !shift {
                state.selection().0
            } else if word {
                prev_word(text, state.cursor)
            } else if modifiers.command() {
                galley.rows[row].start()
            } else {
                prev_caret(galley, state.cursor).unwrap_or(0)
            };
            state.move_to(target, shift);
            KeyResult::Moved
        }
        Key::ArrowRight => {
            let target = if state.has_selection() && !shift {
                state.selection().1
            } else if word {
                next_word(text, state.cursor)
            } else if modifiers.command() {
                galley.rows[row].end()
            } else {
                next_caret(galley, state.cursor).unwrap_or(text.len())
            };
            state.move_to(target, shift);
            KeyResult::Moved
        }
        Key::Home => {
            state.move_to(galley.rows[row].start(), shift);
            KeyResult::Moved
        }
        Key::End => {
            state.move_to(galley.rows[row].end(), shift);
            KeyResult::Moved
        }
        Key::ArrowUp | Key::ArrowDown if multiline => {
            vertical = true;
            let x = *state
                .preferred_x
                .get_or_insert(galley.cursor_rect(state.cursor).min.x);
            let target_row = if key == Key::ArrowUp {
                row.checked_sub(1)
            } else {
                Some(row + 1)
            };
            let target = match target_row.and_then(|r| galley.rows.get(r)) {
                Some(r) => r.index_at_x(x),
                None if key == Key::ArrowUp => 0,
                None => text.len(),
            };
            state.move_to(target, shift);
            KeyResult::Moved
        }
        Key::Backspace => {
            if !delete_selection(text, state) {
                let start = if word {
                    prev_word(text, state.cursor)
                } else {
                    prev_caret(galley, state.cursor).unwrap_or(0)
                };
                if start == state.cursor {
                    return KeyResult::Ignored;
                }
                text.replace_range(start..state.cursor, "");
                state.move_to(start, false);
            }
            KeyResult::Edited
        }
        Key::Delete => {
            if !delete_selection(text, state) {
                let end = if word {
                    next_word(text, state.cursor)
                } else {
                    next_caret(galley, state.cursor).unwrap_or(text.len())
                };
                if end == state.cursor {
                    return KeyResult::Ignored;
                }
                text.replace_range(state.cursor..end, "");
            }
            KeyResult::Edited
        }
        Key::Enter if multiline => {
            insert(text, state, "\n", true);
            KeyResult::Edited
        }
        Key::Enter => KeyResult::Submit,
        Key::Escape => KeyResult::Cancel,
        _ => KeyResult::Ignored,
    };
    if !vertical {
        state.preferred_x = None;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_boundaries() {
        let t = "ciao, mondo bello";
        assert_eq!(next_word(t, 0), 6);
        assert_eq!(next_word(t, 6), 12);
        assert_eq!(next_word(t, 12), t.len());
        assert_eq!(prev_word(t, t.len()), 12);
        assert_eq!(prev_word(t, 12), 6);
        assert_eq!(prev_word(t, 6), 0);
        assert_eq!(prev_word("è già", 7), 3);
    }

    #[test]
    fn insert_filters_newlines_in_single_line_fields() {
        let mut text = String::from("ab");
        let mut state = TextEditState {
            cursor: 1,
            anchor: 1,
            ..Default::default()
        };
        assert!(insert(&mut text, &mut state, "x\ny", false));
        assert_eq!(text, "axyb");
        assert_eq!(state.cursor, 3);
        assert!(!insert(&mut text, &mut state, "\n", false));
        assert!(insert(&mut text, &mut state, "1\r\n2", true));
        assert_eq!(text, "axy1\n2b");
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut text = String::from("hello world");
        let mut state = TextEditState {
            cursor: 0,
            anchor: 5,
            ..Default::default()
        };
        insert(&mut text, &mut state, "ciao", false);
        assert_eq!(text, "ciao world");
        assert_eq!((state.cursor, state.anchor), (4, 4));
    }

    #[test]
    fn boundaries_are_clamped_to_chars() {
        assert_eq!(clamp_to_boundary("è", 1), 0);
        assert_eq!(clamp_to_boundary("abc", 99), 3);
    }
}
