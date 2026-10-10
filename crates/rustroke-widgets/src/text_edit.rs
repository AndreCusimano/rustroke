//! Editable text fields.

use rustroke_core::{
    Color, Event, Galley, ImeEvent, Key, Modifiers, Point, Rect, Stroke, Vec2, point, vec2,
};
use rustroke_text::{LayoutJob, TextStyle};

use crate::widgets::{FrameOverride, frame_setters};
use crate::{
    Context, CursorIcon, FocusLost, Id, Response, Sense, Ui, Widget, WidgetInfo, WidgetRole,
};

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
    /// After a double (words) or triple (lines) click: the unit and the
    /// range first selected, which dragging extends by whole units.
    unit_selection: Option<(SelectUnit, usize, usize)>,
    /// The cursor was placed by code ([`TextEdit::set_selection`]): bring
    /// it into view next time the field is shown.
    scroll_to_cursor: bool,
}

/// The character shown for every character of a password.
const PASSWORD_CHAR: char = '•';

/// `text` with every character replaced by [`PASSWORD_CHAR`].
fn mask(text: &str) -> String {
    text.chars().map(|_| PASSWORD_CHAR).collect()
}

/// Lays out the field's text. For passwords the galley shows • for every
/// character, with its cursor positions mapped back to byte indices of
/// the real text, so editing works on the real text unchanged.
fn layout_field(
    ui: &mut Ui<'_>,
    text: &str,
    style: &TextStyle,
    wrap: Option<f32>,
    password: bool,
    layouter: Option<&Layouter<'_>>,
) -> std::sync::Arc<Galley> {
    if !password {
        if let Some(layouter) = layouter {
            let job = (layouter.0)(text);
            // A job for some other text would break the cursor positions.
            if job.text == text {
                return ui.layout_job(&job, style, wrap);
            }
        }
        return ui.layout_text(text, style, wrap);
    }
    let masked = ui.layout_text(&mask(text), style, wrap);
    let starts: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect();
    let mut galley = (*masked).clone();
    for row in &mut galley.rows {
        for caret in &mut row.carets {
            let n = caret.0 / PASSWORD_CHAR.len_utf8();
            caret.0 = starts.get(n).copied().unwrap_or(text.len());
        }
    }
    std::sync::Arc::new(galley)
}

/// Turns the text of a field into rich text (see [`TextEdit::layouter`]).
struct Layouter<'t>(Box<dyn Fn(&str) -> LayoutJob + 't>);

impl std::fmt::Debug for Layouter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Layouter")
    }
}

/// How [`TextEdit::highlight_line`] marks a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineHighlight {
    /// A background across the whole width of the field.
    Background,
    /// A wavy line under the text of the line (e.g. an error).
    Underline,
}

/// Spaces inserted by Tab in a code editor.
const INDENT: &str = "    ";

/// What a multiple click selects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SelectUnit {
    #[default]
    Word,
    Line,
}

impl SelectUnit {
    /// The word or line around byte `index` of `text`.
    fn range(self, text: &str, index: usize) -> (usize, usize) {
        match self {
            Self::Word => word_range(text, index),
            Self::Line => {
                let start = text[..index].rfind('\n').map_or(0, |i| i + 1);
                let end = text[index..].find('\n').map_or(text.len(), |i| index + i);
                (start, end)
            }
        }
    }
}

/// The run of characters of the same kind (word characters, spaces or
/// punctuation) around byte `index`.
fn word_range(text: &str, index: usize) -> (usize, usize) {
    let kind = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        }
    };
    // The character after the index, or the one before it at the end.
    let Some(here) = text[index..]
        .chars()
        .next()
        .or_else(|| text[..index].chars().next_back())
        .map(kind)
    else {
        return (index, index);
    };
    let start = text[..index]
        .char_indices()
        .rev()
        .take_while(|(_, c)| kind(*c) == here)
        .last()
        .map_or(index, |(i, _)| i);
    let end = text[index..]
        .char_indices()
        .find(|(_, c)| kind(*c) != here)
        .map_or(text.len(), |(i, _)| index + i);
    (start, end)
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
fn edit_kind(event: &Event, multiline: bool, code_editor: bool) -> Option<EditKind> {
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
        Event::Key {
            key: Key::Tab,
            pressed: true,
            ..
        } if code_editor => Some(EditKind::Other),
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
    margin: Vec2,
    pub(crate) frame_style: FrameOverride,
    /// Extra space before and after the text, for icons drawn over the
    /// field (e.g. by `SearchField`).
    pub(crate) inset: [f32; 2],
    password: bool,
    font: Option<TextStyle>,
    code_editor: bool,
    wrap: bool,
    layouter: Option<Layouter<'t>>,
    line_numbers: bool,
    line_highlights: Vec<(usize, Color, LineHighlight)>,
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
            margin: vec2(8.0, 4.0),
            frame_style: FrameOverride::default(),
            inset: [0.0, 0.0],
            password: false,
            font: None,
            code_editor: false,
            wrap: true,
            layouter: None,
            line_numbers: false,
            line_highlights: Vec::new(),
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

    frame_setters!();

    /// Shows every character as • (passwords, keys). Copying and cutting
    /// are disabled; screen readers get the masked text too. For
    /// single-line fields.
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    /// Space between the border and the text (default 8 × 4 points).
    pub fn margin(mut self, margin: Vec2) -> Self {
        self.margin = margin;
        self
    }

    /// Minimum height of a multi-line field, in rows of text.
    pub fn desired_rows(mut self, rows: usize) -> Self {
        self.desired_rows = rows.max(1);
        self
    }

    /// The text style of the field (e.g. `TextStyle::monospace(13.0)` for
    /// code) instead of the style's `body`.
    pub fn font(mut self, style: TextStyle) -> Self {
        self.font = Some(style);
        self
    }

    /// Editing code: while the field has focus, Tab inserts spaces up to
    /// the next multiple of four columns, or indents the selected lines,
    /// and Shift+Tab removes one level of indentation, instead of moving
    /// the focus. Escape still ends editing, so the keyboard can leave the
    /// field.
    pub fn code_editor(mut self) -> Self {
        self.code_editor = true;
        self
    }

    /// Whether a multi-line field wraps long lines at its width (default
    /// `true`). Without wrapping, lines stay whole and the field grows to
    /// the longest one: put it in a [`crate::ScrollArea::both`] to scroll
    /// sideways. Where the width is limited the field scrolls its text
    /// sideways to keep the cursor visible.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Lays the text out as rich text: `layouter` gets the text and
    /// returns it split into formatted sections (colors, styles), e.g. to
    /// highlight syntax. The cursor, selection and input methods work as
    /// usual. The job must hold exactly the text it got, otherwise the
    /// field falls back to plain text; sections without a style use the
    /// field's font. Called whenever the field is laid out, so cache the
    /// result if building it is expensive.
    pub fn layouter(mut self, layouter: impl Fn(&str) -> LayoutJob + 't) -> Self {
        self.layouter = Some(Layouter(Box::new(layouter)));
        self
    }

    /// Shows the line numbers in a margin on the left of the text (lines
    /// end at newlines; wrapped rows of a line share its number).
    pub fn line_numbers(mut self, show: bool) -> Self {
        self.line_numbers = show;
        self
    }

    /// Marks line `line` (counted from 0) with `color`, e.g. the line of
    /// an error. Call it more than once to mark several lines.
    pub fn highlight_line(mut self, line: usize, color: Color, kind: LineHighlight) -> Self {
        self.line_highlights.push((line, color, kind));
        self
    }

    /// Selects `anchor..cursor` (byte indices into the text, in any order;
    /// equal for a plain cursor) in the field with id `id` (see
    /// [`TextEdit::id`]), and scrolls it into view the next time the field
    /// is shown. Indices are clamped to the text. Use
    /// `Context::request_focus` as well to start editing there; see
    /// [`TextEdit::line_column_to_index`] for positions given as line and column.
    pub fn set_selection(ctx: &mut Context, id: Id, anchor: usize, cursor: usize) {
        let mut state: TextEditState = ctx.data(id).unwrap_or_default();
        state.anchor = anchor;
        state.cursor = cursor;
        state.preferred_x = None;
        state.unit_selection = None;
        state.scroll_to_cursor = true;
        ctx.insert_data(id, state);
        ctx.request_repaint();
    }

    /// The selection of the field with id `id`, as `(anchor, cursor)`
    /// byte indices (equal when nothing is selected), or `None` if it has
    /// not been shown yet.
    pub fn selection(ctx: &Context, id: Id) -> Option<(usize, usize)> {
        ctx.data::<TextEditState>(id)
            .map(|state| (state.anchor, state.cursor))
    }
    /// The byte index of `column` (in characters, from 0) on line `line`
    /// (from 0) of `text`. Positions past the end of a line give its end;
    /// lines past the end of the text give the end of the text.
    pub fn line_column_to_index(text: &str, line: usize, column: usize) -> usize {
        let mut start = 0;
        for _ in 0..line {
            match text[start..].find('\n') {
                Some(i) => start += i + 1,
                None => return text.len(),
            }
        }
        let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
        text[start..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(i, _)| start + i)
    }

    /// The line and column (both from 0, the column in characters) of byte
    /// `index` of `text`; the reverse of [`TextEdit::line_column_to_index`].
    pub fn index_to_line_column(text: &str, index: usize) -> (usize, usize) {
        let index = clamp_to_boundary(text, index);
        let before = &text[..index];
        let line = before.matches('\n').count();
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        (line, text[line_start..index].chars().count())
    }
}

impl Widget for TextEdit<'_> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let visuals = &style.visuals;
        let font = self.font.clone().unwrap_or_else(|| style.body.clone());
        let padding = self.margin;
        let available = ui.available_width();
        let min_width = self.frame_style.min_size.map_or(40.0, |m| m.x);
        let mut width = self
            .desired_width
            .unwrap_or(if self.multiline {
                ui.fill_width(480.0)
            } else {
                240.0
            })
            .min(available)
            .max(min_width);
        // Line numbers take a margin on the left, wide enough for the
        // number of the last line.
        let gutter = if self.line_numbers {
            let lines = self.text.matches('\n').count() + 1;
            let digits = lines.to_string().len().max(2);
            let digit = ui.layout_text("0", &font, None).size.x;
            digits as f32 * digit + padding.x
        } else {
            0.0
        };
        let inset_left = self.inset[0] + gutter;
        let inset_right = self.inset[1];
        let row_height = font.size * font.line_height;
        let wraps = self.multiline && self.wrap;
        // Single-line fields and fields that don't wrap scroll sideways.
        let scrolls_x = !wraps;
        let wrap = wraps.then_some(width - 2.0 * padding.x - inset_left - inset_right);

        let id = self.id.unwrap_or_else(|| ui.next_auto_id());
        let mut state: TextEditState = ui.ctx().data(id).unwrap_or_default();
        state.cursor = clamp_to_boundary(self.text, state.cursor);
        state.anchor = clamp_to_boundary(self.text, state.anchor);

        // Size from the current text (before this frame's edits).
        let password = self.password;
        let layouter = self.layouter.as_ref();
        let layout =
            |ui: &mut Ui<'_>, text: &str| layout_field(ui, text, &font, wrap, password, layouter);
        let galley = layout(ui, self.text);
        if self.multiline && !self.wrap {
            // As wide as the longest line, where there is room.
            let content = galley.size.x + 2.0 * padding.x + inset_left + inset_right + 2.0;
            width = width.max(content.min(available));
        }
        let inner_width = width - 2.0 * padding.x - inset_left - inset_right;
        let content_height = if self.multiline {
            galley.size.y.max(self.desired_rows as f32 * row_height)
        } else {
            row_height
        };
        let height = self
            .frame_style
            .size(
                vec2(width, content_height + 2.0 * padding.y),
                style.spacing.interact_height,
            )
            .y;
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
            .value(if self.password {
                mask(self.text)
            } else {
                self.text.clone()
            }),
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
            point(rect.min.x + padding.x + inset_left - scroll_x, y)
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
        let by_mouse = response.drag_started() || response.dragged();
        if by_mouse && let Some(pos) = ui.input().pointer.pos() {
            let index = galley.index_at(Point::new(0.0, 0.0) + (pos - text_origin(state.scroll_x)));
            let extend =
                response.dragged() && !response.drag_started() || ui.input().modifiers.shift;
            if response.drag_started() {
                state.unit_selection = match response.press_count {
                    2 => Some(SelectUnit::Word),
                    n if n >= 3 => Some(SelectUnit::Line),
                    _ => None,
                }
                .map(|unit| {
                    let (a, b) = unit.range(self.text, index);
                    (unit, a, b)
                });
            }
            match state.unit_selection {
                // Whole words or lines, from the first one to the pointer.
                Some((unit, a, b)) => {
                    let (c, d) = unit.range(self.text, index);
                    if c < a {
                        state.anchor = b;
                        state.cursor = c;
                    } else {
                        state.anchor = a;
                        state.cursor = d.max(b);
                    }
                }
                None => state.move_to(index, extend),
            }
            state.preferred_x = None;
        }

        if gained_focus && self.select_all_on_focus {
            state.anchor = 0;
            state.cursor = self.text.len();
        }

        // Keyboard and text input, in the order they happened.
        if response.has_focus() {
            ui.ctx().set_keyboard_owner(id);
            if self.code_editor {
                // Tab indents instead of moving the focus (next frame).
                ui.ctx().set_tab_owner(id);
            }
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
                        galley = layout(ui, self.text);
                    }
                    continue;
                }
                let kind = edit_kind(&event, self.multiline, self.code_editor);
                let before = kind.map(|_| Snapshot::of(self.text, &state));
                let edited = match event {
                    Event::Text(text) => insert(self.text, &mut state, &text, self.multiline),
                    Event::Paste(text) => insert(self.text, &mut state, &text, self.multiline),
                    Event::Copy | Event::Cut if self.password => false,
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
                        key: Key::Tab,
                        pressed: true,
                        modifiers,
                        ..
                    } if self.code_editor
                        && (modifiers == Modifiers::NONE || modifiers == Modifiers::SHIFT) =>
                    {
                        ui.ctx().input_mut().consume_key(Key::Tab, modifiers);
                        if modifiers.shift {
                            outdent(self.text, &mut state)
                        } else {
                            indent(self.text, &mut state)
                        }
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
                    galley = layout(ui, self.text);
                }
            }
            if let Some(history) = history {
                ui.ctx().insert_data(undo_id, history);
            }
        }
        let cursor_moved = changed || state.cursor != old_cursor;
        if cursor_moved {
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
        let galley = layout(ui, &display);

        // Fields that don't wrap scroll sideways to keep the cursor visible.
        if scrolls_x {
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
        let caret = offset(
            galley.cursor_rect(state.cursor + state.preedit.len()),
            origin,
        );

        // Scroll areas around the field follow the cursor when it moves by
        // keys or is placed by code.
        if state.scroll_to_cursor {
            state.scroll_to_cursor = false;
            ui.ctx().scroll_to_rect(
                Rect::from_min_max(caret.min - vec2(2.0, 0.0), caret.max + vec2(2.0, 0.0)),
                Some(crate::Align::Center),
            );
        } else if has_focus && cursor_moved && !by_mouse {
            ui.ctx().scroll_to_rect(
                Rect::from_min_max(caret.min - vec2(2.0, 0.0), caret.max + vec2(2.0, 0.0)),
                None,
            );
        }

        // Frame.
        let custom = self.frame_style;
        let stroke = if has_focus {
            Stroke::new(1.5, visuals.accent)
        } else {
            custom.stroke(ui.widget_visuals(&response).stroke)
        };
        let fill = custom.fill.unwrap_or(visuals.text_field_fill);
        let radius = custom.corner_radius(visuals.corner_radius);
        ui.painter().rect(rect, radius, fill, stroke);

        // The line each row of the text belongs to.
        let lines = if self.line_numbers || !self.line_highlights.is_empty() {
            line_of_rows(&galley, &display)
        } else {
            Vec::new()
        };

        let saved_clip = ui.clip_rect();
        let gutter_right = rect.min.x + inset_left;
        if gutter > 0.0 {
            ui.set_clip_rect(rect);
            let visible = ui.clip_rect();
            let current = galley.row_of(state.cursor);
            let x = gutter_right + padding.x / 2.0;
            ui.painter().line(
                point(x, rect.min.y),
                point(x, rect.max.y),
                visuals.window_stroke,
            );
            for (i, row) in galley.rows.iter().enumerate() {
                let top = origin.y + row.top;
                let first_row = i == 0 || lines[i] != lines[i - 1];
                if !first_row || top > visible.max.y || top + row.height < visible.min.y {
                    continue;
                }
                let number = ui.layout_text(&(lines[i] + 1).to_string(), &font, None);
                let color = if has_focus && lines.get(current) == Some(&lines[i]) {
                    visuals.text
                } else {
                    visuals.weak_text
                };
                let pos = point(gutter_right - number.size.x, top);
                ui.painter().galley(pos, number, color);
            }
            ui.clip_rect_restore(saved_clip);
        }

        let text_left = if gutter > 0.0 {
            gutter_right + padding.x / 2.0
        } else {
            rect.min.x + inset_left
        };
        let text_clip = Rect::from_min_max(
            point(text_left, rect.min.y),
            point(rect.max.x - inset_right, rect.max.y),
        );
        ui.set_clip_rect(text_clip.expand(-1.0));
        for &(line, color, kind) in &self.line_highlights {
            let rows = galley.rows.iter().zip(&lines).filter(|(_, l)| **l == line);
            for (row, _) in rows {
                let top = origin.y + row.top;
                match kind {
                    LineHighlight::Background => {
                        let band = Rect::from_min_max(
                            point(text_clip.min.x, top),
                            point(text_clip.max.x, top + row.height),
                        );
                        ui.painter().rect_filled(band, 0.0, color);
                    }
                    LineHighlight::Underline => {
                        // Under the text, without the indentation.
                        let (start, end) = (row.start(), row.end().min(display.len()));
                        let indent = display
                            .get(start..end)
                            .map_or(0, |t| t.len() - t.trim_start().len());
                        let first = row.x_of(start + indent);
                        let last = row.x_of(end);
                        let x0 = origin.x + first.min(last);
                        let x1 = (origin.x + first.max(last)).max(x0 + 8.0);
                        let y = top + row.height - 1.5;
                        ui.painter()
                            .polyline(wave(x0, x1, y), Stroke::new(1.0, color));
                    }
                }
            }
        }
        if has_focus {
            let selection = visuals.selection;
            let (a, b) = state.selection();
            for r in galley.selection_rects(a, b) {
                ui.painter().rect_filled(offset(r, origin), 0.0, selection);
            }
        }
        if self.text.is_empty() && state.preedit.is_empty() && !has_focus && !self.hint.is_empty() {
            let hint = ui.layout_text(&self.hint, &font, wrap);
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

/// For every row of `galley`, the line of `text` (counted from 0) it
/// belongs to: lines end at newlines, wrapped rows share their line.
fn line_of_rows(galley: &Galley, text: &str) -> Vec<usize> {
    let mut line = 0;
    galley
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let start = clamp_to_boundary(text, row.start());
            if i > 0 && text[..start].ends_with('\n') {
                line += 1;
            }
            line
        })
        .collect()
}

/// A wavy line from `x0` to `x1` around `y` (error underline).
fn wave(x0: f32, x1: f32, y: f32) -> Vec<Point> {
    const STEP: f32 = 2.0;
    let mut points = Vec::new();
    let mut x = x0;
    let mut up = false;
    while x < x1 {
        points.push(point(x, if up { y - 1.0 } else { y + 1.0 }));
        up = !up;
        x += STEP;
    }
    points.push(point(x1, if up { y - 1.0 } else { y + 1.0 }));
    points
}

/// The starts of the lines the selection touches (just the cursor's line
/// without a selection). A selection ending at the start of a line
/// doesn't include that line.
fn selected_line_starts(text: &str, state: &TextEditState) -> Vec<usize> {
    let (a, b) = state.selection();
    let first = text[..a].rfind('\n').map_or(0, |i| i + 1);
    let mut starts = vec![first];
    for (i, _) in text[first..b].match_indices('\n') {
        let start = first + i + 1;
        if start < b {
            starts.push(start);
        }
    }
    starts
}

/// Tab in a code editor: indents the selected lines, or inserts spaces up
/// to the next tab stop. Returns whether the text changed.
fn indent(text: &mut String, state: &mut TextEditState) -> bool {
    let (a, b) = state.selection();
    let multiple_lines = text[a..b].contains('\n');
    if !multiple_lines {
        let line_start = text[..a].rfind('\n').map_or(0, |i| i + 1);
        let column = text[line_start..a].chars().count();
        let spaces = INDENT.len() - column % INDENT.len();
        return insert(text, state, &INDENT[..spaces], true);
    }
    for start in selected_line_starts(text, state).into_iter().rev() {
        text.insert_str(start, INDENT);
        for index in [&mut state.cursor, &mut state.anchor] {
            // A selection starting at the start of a line keeps it.
            if *index > start || (*index == start && *index != a) {
                *index += INDENT.len();
            }
        }
    }
    state.preferred_x = None;
    true
}

/// Shift+Tab in a code editor: removes one level of indentation (up to
/// four spaces, or a tab) from the selected lines. Returns whether the
/// text changed.
fn outdent(text: &mut String, state: &mut TextEditState) -> bool {
    let mut changed = false;
    for start in selected_line_starts(text, state).into_iter().rev() {
        let rest = &text[start..];
        let n = if rest.starts_with('\t') {
            1
        } else {
            rest.bytes()
                .take(INDENT.len())
                .take_while(|b| *b == b' ')
                .count()
        };
        if n == 0 {
            continue;
        }
        text.replace_range(start..start + n, "");
        for index in [&mut state.cursor, &mut state.anchor] {
            if *index > start + n {
                *index -= n;
            } else if *index > start {
                *index = start;
            }
        }
        changed = true;
    }
    if changed {
        state.preferred_x = None;
    }
    changed
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
