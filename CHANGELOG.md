# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.7.0] — 2026-10-09

### Added

- **More native windows**: `Frame::show_window(id, WindowOptions)`, called
  from the main window's `update` every frame the window should exist,
  keeps an extra native window open (e.g. an assembly view on a second
  monitor); it closes when a frame stops asking for it. Its content comes
  from the new `App::update_window(id, frame)`, and
  `App::on_window_close_requested(id)` handles its close button. Each
  window has its own context and renderer; fonts and icons are shared, and
  input in one window redraws the others (shared app state).
  `Frame::window_id()` tells which window a frame is for.
- `testing::Harness::requested_windows()` and `run_window(app, id)`.
- `TextureAtlas::revision()` and `dirty_base()`: several renderers can
  share one atlas (a renderer that missed changes uploads it whole).
- New example: `windows`.

## [0.6.0] — 2026-10-09

### Added

- **Tab bars**: `TabBar::new(id).add_button(..).show(ui, &mut items,
  &mut active, |item| TabLabel::new(name).icon(..).modified(..))`: click
  to switch, drag to reorder, × or middle click to close (the app decides,
  through `TabBarResponse::close_requested`), optional "+", per-tab
  responses for context menus. Modified tabs show a dot instead of ×.
- **Docking**: `DockArea::new(id).show(ui, &mut dock_state, &mut viewer)`
  arranges panels as tab groups in resizable splits. Drag a tab to
  reorder it, into another group's tab bar, or onto a side of a group to
  split it (with a preview of where it goes); drag the line between groups
  to resize them. The layout is a `DockState` owned by the app (built with
  `DockState::new` and `split`, changed with `push_tab`, `remove_tab`,
  inspected with `root`, `groups`, `tabs`, `find_group`); the app shows
  the tabs through the `DockViewer` trait (`label`, `ui`, `closable`,
  `on_close`, `context_menu`, `add_tab`).
- `WidgetRole::Tab`, exposed to screen readers as tabs.
- New example: `docking` (document tabs and dockable panels).

## [0.5.0] — 2026-10-09

### Added

- **Paint callbacks**: `CallbackFn::new(|info, render_pass| ..)` (with an
  optional `.prepare(|device, queue, encoder, info| ..)`) draws with the
  application's own wgpu pipelines inside a rectangle of the UI, in the
  UI's render pass and in order with it: `ui.painter().add(callback
  .into_shape(rect))`. The viewport covers the rectangle and the scissor
  keeps drawing inside it. New `Shape::Callback(PaintCallback)` in the core
  (renderer-independent), `ClippedMesh::callback`, `CallbackInfo`,
  `Frame::wgpu_target_format()`, `Renderer::target_format()` and
  `OffscreenRenderer::target_format()`.
- **Integration in an existing event loop**: `Integration` embeds the UI in
  an application that owns its winit window and wgpu device:
  `on_window_event` (says whether the UI consumed the event, so clicks on a
  panel don't reach the 3D view), `run(window, |frame| ..)` and
  `paint(device, queue, encoder, view, size)`, which draws over the
  application's content. `run_frame` runs without a window.
- `Painter::paint_over` draws without clearing the target first.
- `Context::is_pointer_over_ui(pos)` and `Context::wants_pointer_input()`.
- `rustroke::winit` re-exports the winit version rustroke uses;
  `PhysicalSize` is re-exported by `rustroke`.
- New examples: `custom_wgpu` (a shader inside the UI) and `integration`
  (the UI inside your own winit loop).

## [0.4.0] — 2026-10-09

### Added

- **SVG icons**: `Fonts::add_svg_icon(svg)` (through `Frame::fonts()`)
  loads an icon that is rasterized on demand at the size and screen density
  it is drawn at, cached in the glyph atlas. Two-tone: black parts take the
  text color and `#1E6FFF` parts (`ICON_ACCENT_SOURCE_COLOR`) the theme's
  accent, so one file fits light and dark themes and every widget state.
  `add_svg_icon_themed(light, dark)` for icons that need a dark drawing,
  `add_svg_icon_colored` for icons with their own colors. Shown with
  `Icon` / `Ui::icon`, `Button::icon`, `Button::icon_only`,
  `CollapsingHeader::icon`; `Ui::rasterize_icon` and `Ui::paint_icon` for
  custom widgets. SVGs are rendered with resvg (Apache-2.0 OR MIT).
- **Lists**: `List::new(id).multi_select(..).reorderable(..).show(ui,
  &mut items, &mut selection, |ui, index, item| ..)`: rows that can be
  selected (click, Cmd/Ctrl+click, Shift+click, arrow keys, Home/End) and
  reordered by dragging, with a `ListResponse` (row responses for context
  menus and tooltips, clicked row, selection changes, moves).
- **Scrolling sideways**: `ScrollArea::horizontal()` and `both()`, with
  `max_width`; the mouse wheel scrolls sideways in horizontal-only areas
  and with Shift.
- `Panel::auto_width()`: a side panel exactly as wide as its content.
- **Undo and redo in text fields**: Cmd/Ctrl+Z, Cmd/Ctrl+Shift+Z (and
  Ctrl+Y outside macOS); typing within a second is undone in one step.
- **Bundled symbol font** (subset of Noto Sans Math, Symbols and Symbols 2,
  SIL Open Font License): arrows, math and technical symbols, shapes and
  dingbats (↶ ⚓ ∥ ⊥ ⌀ ✓ ★ ⚙) render even without system fonts.
- **More keys**: `,` `.` `/` `\` `;` `'` `` ` `` `[` `]`, the numeric
  keypad (`Numpad0`–`Numpad9`, operators, decimal point; its Enter is
  `Key::Enter`) and Insert. Letters and digits are recognized with
  Cmd/Ctrl/Alt held too (by position when the typed character is unknown,
  e.g. Option+E on macOS).
- `testing::Harness::render()` draws the last frame on the GPU without a
  window (with every texture still alive) and returns a `Screenshot`
  (`pixel`, `save_png`).
- `Button::selected(..)` highlights the current choice (active tool).
- `Context::any_popup_open()` and `Context::open_popup_id()`, to leave
  Escape, arrows and Enter to an open menu.
- `Ui::fill_width(natural)` for widgets that fill the available width.
- New examples: `lists` (icons, reorderable list, horizontal scrolling) and
  `files` (open and save with native dialogs through `rfd`).

### Fixed

- `DragValue`: dragging over several frames now adds up the whole
  movement (it only counted the last frame's movement).

### Changed

- Inside a horizontally scrolling area the available width is unlimited:
  labels don't wrap and widgets that fill the width use their natural size.
- Scroll bars only take space when the content overflows.
- Escape is only consumed when it closes a popup or takes focus away from a
  widget; otherwise the app sees it (e.g. to leave a tool).

## [0.3.0] — 2026-10-08

### Added

- **New widgets**: `ComboBox` (drop-down list), `DragValue` (a number
  changed by dragging, arrow keys or typing, with range, speed, prefix and
  suffix), `SelectableLabel` with `Ui::selectable_label` /
  `Ui::selectable_value`, `ProgressBar` (text, percentage, animated) and
  `Spinner`.
- **Collapsible sections and trees**: `CollapsingHeader` (default open,
  forced open/closed, keyboard arrows, animated), tree-node style with
  `.selected(..)` (the label selects, the triangle toggles),
  `Ui::collapsing` and `Ui::indent`; `Spacing::indent`.
- **Context menus**: `Response::context_menu(ui, ..)` opens a menu at the
  pointer on right-click.
- **All mouse buttons**: `Response::secondary_clicked`, `middle_clicked`,
  `clicked_by` and `dragged_by(button)` (e.g. middle-drag to pan a view).
- **Keyboard shortcuts**: `KeyboardShortcut` with `format()` ("⇧⌘S" on
  macOS, "Ctrl+Shift+S" elsewhere), `InputState::consume_shortcut`,
  `Modifiers::COMMAND`, `Button::shortcut_text` for menu items; keys A–Z,
  0–9, F1–F12, `-` and `=`.
- **Text fields**: Escape restores the text the field had when it got
  focus; `Response::lost_focus_reason()` returns `FocusLost::Submit`
  (Enter), `Cancel` (Escape) or `Other` (Tab, click elsewhere), and
  `Response::submitted()`. `TextEdit::select_all_on_focus(true)` and
  `TextEdit::id(..)`.
- `Context::request_focus(id)`, `Context::wants_keyboard_input()` and
  `Context::remove_data(id)`.
- More mouse cursors: `ResizeVertical`, `Crosshair`, `Move`, `NotAllowed`.
- Accessibility roles for the new widgets (combo box, spin button,
  progress indicator, list item) with expanded and selected states.
- `testing::Harness::right_click` and `click_with(button)`.
- New example: `cargo run -p rustroke --example properties`.

### Changed

- Radio buttons inside a menu or combo box close it when clicked.
- `Response::lost_focus()` is also true when a text field loses focus by
  Tab or a click elsewhere, not only by Enter.
- `Spacing` has a new field (`indent`).

## [0.2.0] — 2026-10-08

### Added

- **Native GPU textures**: `Frame::wgpu()` gives rustroke's `Device` and
  `Queue`; `Frame::register_native_texture(view, size)` shows an
  application-owned wgpu texture as an image without copying it
  (`Frame::update_native_texture` after recreating it). `rustroke::wgpu`
  re-exports the matching wgpu version.
- **Wake-ups from other threads**: `Context::repaint_handle()` /
  `Frame::repaint_handle()` return a `RepaintHandle` (`Clone + Send + Sync`)
  whose `request_repaint()` schedules a frame.
- `TextureHandle::set(image)` replaces a texture's image in place, keeping
  its id.
- **Testing kit**: `rustroke::testing::Harness` runs an `App` without a
  window: `click(app, label)`, `type_text`, `key`, `widgets()`, `find`,
  `title()`, `shapes()`.
- `Context::widgets()` / `find_widget(label)`: role, label, state and
  rectangle of every widget of the last frame, without enabling
  accessibility.
- `accessible_label(..)` on `Button`, `Checkbox`, `RadioButton`, `Slider`
  and `TextEdit`, for widgets without visible text.
- `Response::hover_pos()` and `Response::interact_pointer_pos()`.
- `InputState::consume_scroll()` and `Ui::input_mut()`, so a widget can keep
  the mouse wheel from scrolling its parent.
- `POINTS_PER_SCROLL_LINE`: points per mouse wheel notch.
- `Frame::set_title(..)` and `App::on_close_requested()`.
- Semantic theme colors: `Visuals::success`, `warning`, `error`, `info`.
- `Color::from_srgb8` / `from_srgba8` are `const fn`, for `const` colors.
- `RawInput`, `Event`, `ImeEvent`, `FrameOutput`, `TexturesDelta` and
  `WidgetDescription` are re-exported by `rustroke`.

## [0.1.1] — 2026-10-08

### Fixed

- Widgets under the resize grip of a side panel or in a window's
  bottom-right corner are clickable again: the grips now take only clicks
  where there is no widget (CAD3D LAY-01).
- A grid column grows to fit a widget's desired width (e.g. a `TextEdit`
  with `desired_width`), instead of staying at last frame's width when it
  only had empty cells before (CAD3D LAY-03).
- On the frame a drag starts, `Response::drag_delta` only counts the
  movement after the press.

## [0.1.0] — 2026-10-08

First release of **Rustroke**.

### Added

- **Rendering**: wgpu renderer with anti-aliased rectangles (rounded),
  circles, lines, polylines, convex polygons, clipping, glyph atlas and
  user textures; offscreen rendering for tests and screenshots.
- **Text**: layout, shaping, bidirectional text and wrapping with
  cosmic-text; bundled Inter font with system font fallback (emoji, CJK,
  Arabic, ...); text measurement.
- **Immediate-mode API**: `Context`, `Ui`, `Response`, automatic widget
  ids, hover/press/focus handling, keyboard navigation (Tab, Enter, Space,
  arrows, Escape).
- **Widgets**: label, heading, button, checkbox, radio button, slider (any
  numeric type), separator, image, single- and multi-line text field with
  selection, clipboard, word navigation and IME.
- **Layout**: rows, columns, alignment, justification, wrapping,
  right-to-left rows, fixed-size widgets, grids with aligned columns.
- **Containers**: top/bottom/left/right panels (side panels resizable),
  central panel, movable and resizable windows, vertical scroll areas,
  popup menus, tooltips; layers with correct z-order and hit testing.
- **Style**: dark and light themes, accent color, runtime switching,
  animated hover/press transitions.
- **Disabled widgets** (`Ui::add_enabled`, `Ui::add_enabled_ui`).
- **Closures as apps**: `rustroke::run(options, |frame: &mut Frame| ...)` for
  small programs, no struct or trait needed.
- **Accessibility**: AccessKit tree for screen readers (VoiceOver,
  Narrator, Orca) with click and focus actions.
- Examples for every feature, GPU snapshot tests, headless interaction tests.

### Known limitations

- Text fields have no undo, double-click word selection or password mode.
- Scroll areas are vertical only; windows resize in width only; no submenus.
- Desktop only (macOS, Windows, Linux); only tested on macOS so far.

[Unreleased]: https://github.com/AndreCusimano/rustroke/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/AndreCusimano/rustroke/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/AndreCusimano/rustroke/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/AndreCusimano/rustroke/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/AndreCusimano/rustroke/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/AndreCusimano/rustroke/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/AndreCusimano/rustroke/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/AndreCusimano/rustroke/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/AndreCusimano/rustroke/releases/tag/v0.1.0
