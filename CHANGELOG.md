# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.17.0] — 2026-10-09

### Added

- **Apps in the browser**: the same `rustroke::run` app compiles to
  `wasm32-unknown-unknown` and draws into a canvas added to the page,
  with WebGPU or, where it is missing, WebGL 2. `tools/web.sh <example>`
  builds an example for the web (needs `wasm-bindgen-cli`). In the
  browser the GPU is set up asynchronously, `run` returns right away,
  copy and paste stay inside the app, panics are printed to the console,
  and screen readers, native menus, the tray icon, notifications and
  saving state are not available yet. CI builds the `widgets` example for
  wasm.
- **Software renderer**: new crate `rustroke-soft` (re-exported as
  `rustroke::SoftwareRenderer`) draws the same meshes on the CPU, with the
  GPU's conventions (linear blending, sRGB, bilinear sampling, scissors),
  for machines without a GPU; `testing::Harness::render_software()`.

### Changed

- `Ui::dnd_drag_source` also starts dragging when the press is on a widget
  inside that doesn't drag by itself (e.g. a card made of a button), after
  the pointer moves 4 points.
- In the browser the canvas fills the page and follows its size.
- `testing::Harness` draws toasts, like a real window.
- The README has a gallery: code next to the image it draws (rendered by
  `cargo test -p rustroke --test gallery --features markdown -- --ignored`).
- The painter offsets indices on the CPU instead of drawing with a base
  vertex, which WebGL doesn't support (pages with several clipped areas
  crashed there).
- `rustroke-render` creates its wgpu instance with WebGPU detection, so
  browsers without WebGPU fall back to WebGL 2.
- `rustroke-winit` uses `web-time` for timing, and its desktop-only
  dependencies (arboard, accesskit_winit, pollster) are not built for
  wasm.

## [0.16.0] — 2026-10-09

### Added

- **Flexbox layout** (computed with taffy): `Flex::row(id)` /
  `Flex::column(id)` with `wrap`, `gap`, `justify` (start, end, center,
  space between / around / evenly) and `align` (start, end, center,
  stretch); items added with `flex.add(FlexItem::new().grow(1.0)
  .basis(..).shrink(..).align(..), |ui| ..)`.
- **CSS-style grid**: `FlexGrid::new(id, vec![Track::Points(100.0),
  Track::Fraction(1.0), Track::Auto])` with cells placed by
  `GridCell::at(column, row).span(columns, rows)`.
- Both settle in two frames, using the sizes their items had in the
  previous frame (like `Grid`). New `flex` example.

## [0.15.0] — 2026-10-09

### Added

- **Color picker**: `ColorPicker::new(&mut color).alpha(true)`, a swatch
  that opens a saturation/value square, a hue bar, an opacity bar and a
  hex field (`#RGB`, `#RRGGBB`, `#RRGGBBAA`). `Color::to_hsva` /
  `from_hsva`.
- **Date and time pickers**: `DatePicker::new(&mut date)` with a calendar
  popup (month navigation, weeks from Monday or `sunday_first`);
  `TimePicker::new(&mut hour, &mut minute)`. `Date` (days since the
  epoch, weekday, `add_months`, `today_utc`, ISO display) and
  `days_in_month`, without dependencies.
- **Toasts**: `ctx.toast(Toast::success("Saved"))` (info, success,
  warning, error; `duration`) shows messages stacked in the bottom-right
  corner, fading in and out, each with a × to dismiss it;
  `show_toasts` for apps driving a `Context` directly.
- New `pickers` example.

## [0.14.0] — 2026-10-09

### Added

- **Drag and drop between widgets**: `ui.dnd_drag_source(id, payload,
  |ui| ..)` makes content draggable with a payload of any type (it
  follows the pointer, its place stays empty); `ui.dnd_drop_zone::<T>(|ui|
  ..)` outlines itself while a `T` is dragged and returns it when dropped
  there. `Context::is_dnd_active`, `dnd_payload`, `take_dnd_payload`.
- **Files from the system**: `Event::FileHovered` / `FileHoverCancelled` /
  `FileDropped`; `InputState::hovered_files` and `dropped_files`.
- **Gestures and touch**: `Event::Zoom` (pinch) and `Event::Rotate`
  (`InputState::zoom_delta`, `rotation_delta`); `Event::Touch` with
  `TouchPhase`, the first finger also acting as the mouse.
- **Native menu bar** (macOS, feature `native-menu`):
  `Frame::set_native_menu(vec![NativeMenu::new("File", items)])` after an
  application menu with About, Hide and Quit; choices arrive in
  `Frame::native_menu_events`.
- **Tray icon** (macOS, Windows, feature `tray`): `Frame::set_tray(Some(
  TrayOptions { icon, tooltip, menu }))`, `Frame::tray_clicked`.
- The `native-menu` and `tray` features need Rust 1.90 (their
  dependencies do); everything else still builds with Rust 1.89.
- **Notifications**: `rustroke::notify(title, body)` (macOS, Windows 10+,
  Linux with `notify-send`).
- New `platform` example.

## [0.13.0] — 2026-10-09

### Added

- **Persistence** (feature `persistence`): `WindowOptions::persistence_id`
  saves the UI state (window positions and sizes, panel sizes, open
  sections, table column widths and sort, open tree nodes), the native
  window's position and size, and app values (`Frame::set_value` /
  `Frame::value`) when the app exits, and restores them at start, in the
  platform's configuration folder. `Context::insert_persisted` /
  `data_persisted` for custom widgets, `Context::save_state` /
  `load_state` for apps driving a `Context` directly; `DockState` can be
  serialized with serde.
- **Global zoom**: Cmd/Ctrl + = / - / 0 zoom the whole UI
  (`Context::zoom_factor`, `set_zoom_factor`, `set_zoom_shortcuts`).
- **Inspector**: Cmd/Ctrl+Alt+I opens a debugging window (frame time,
  zoom, focus, the widgets under the pointer, outlined on screen, and the
  style edited live); `show_inspector` for apps driving a `Context`.
- **Automation**: `Context::automation()` gives a `Send` handle to drive
  the app from another thread (read the widgets, click by label, type,
  press keys, wait for frames), e.g. for end-to-end tests or an agent.
- **Accessibility in tests**: `Harness::enable_accessibility`,
  `accesskit_tree` and `find_by_role`; `accesskit` is re-exported.

### Changed

- `WindowOptions` has a new field, `persistence_id`.

## [0.12.0] — 2026-10-09

### Added

- **Gradients**: `Gradient::linear` / `Gradient::radial` with any number
  of stops (`with_stop`), drawn with `DisplayList::rect_gradient`,
  `circle_gradient`, `polygon_gradient` or `gradient_fill` (new
  `Shape::Gradient`). Linear gradients are exact (triangles are cut at
  every stop); radial ones are subdivided finely.
- **Soft shadows**: `Shadow { offset, blur, spread, color }` and
  `DisplayList::shadow(rect, radius, shadow)` (new `Shape::Shadow`), a
  Gaussian-like blur built from rings of vertices. Windows, menus,
  tooltips and dialogs use it (`Visuals::shadow`).
- **Lines and curves**: `DisplayList::dashed_line`, `dotted_line`,
  `quadratic_bezier`, `cubic_bezier`; `dashes`, `quadratic_bezier_points`,
  `cubic_bezier_points`.
- **Transforms**: `Transform` (translate, rotate, scale, `from_axes`,
  `then`) and `DisplayList::with_transform(t, |list| ..)` rotates, scales
  or shears everything drawn inside, text and images included.
- **Transformed text** (CAD3D TXT-06): `DisplayList::galley_transformed(galley,
  transform, color)` (new `Shape::TransformedText`), e.g. labels on the
  faces of a view cube.
- **App meshes**: `Shape::Mesh` / `DisplayList::mesh` draw triangles built
  by the app, with any texture.
- **Animated images**: `rustroke::load_animated_image` decodes GIF, APNG
  and animated WebP; `AnimatedTexture` holds the frames on the GPU and
  `AnimatedImage` plays them, drawing frames only when one is due.
- New `graphics` example.

### Changed

- `Visuals::shadow` is a `Shadow` (offset, blur, spread, color) instead of
  a color; floating content casts a softer, wider shadow.
- `Shape` has new variants (`Gradient`, `Shadow`, `Mesh`,
  `TransformedText`); the `image` feature also decodes GIF and WebP.

## [0.11.0] — 2026-10-09

### Added

- **Rich text**: `LayoutJob` (sections of text, each with a `TextFormat`:
  style, italic, color, background, underline, strike-through, link),
  laid out with `Fonts::layout_job` / `Ui::layout_job` and shown with
  `Label::rich(job)`. Links take the accent color, are underlined under
  the pointer and open their URL when clicked. Sections can have
  different sizes on one line.
- **Markdown** (feature `markdown`, pulldown-cmark): `ui.markdown(text)` /
  `Markdown::new(text).image_loader(..).show(ui)` with headings, bold,
  italic, strike-through, inline code, links, nested bulleted and
  numbered lists, task lists, block quotes, code blocks, tables, rules
  and images.
- `Galley::decorations`, `Galley::sections` and `Galley::section_at`;
  `GlyphQuad::color` (per-glyph color, still faded with the shape's
  alpha). New `rich_text` example (`--features markdown`).

### Changed

- **Right-to-left text**: paragraphs in Arabic, Hebrew... are aligned to
  the right of the wrap width (the galley spans it), and selections in
  mixed-direction text cover the right characters (several rectangles
  per row when needed).
- `GlyphQuad` has a new field, `color`, and is no longer `Eq`; `Galley`
  has new fields (`decorations`, `sections`) and implements `Default`.
- CI runs clippy, tests and docs with `--all-features`; docs.rs builds
  with all features.

## [0.10.0] — 2026-10-09

### Added

- **Virtual scrolling**: `ScrollArea::show_rows(ui, row_height, total,
  |ui, range| ..)` only adds the visible rows (a million rows scroll like
  ten); `ScrollArea::show_viewport(ui, |ui, visible| ..)` for rows of
  different heights, with `Ui::set_cursor_y` and
  `Ui::extend_min_rect_to_y`.
- **Scrolling to something**: `Ui::scroll_to_rect(rect, align)` /
  `Context::scroll_to_rect` scroll the areas around `rect` so it is
  visible (as little as needed, or aligned to the top, center or bottom);
  nested areas all follow.
- **Autoscroll**: dragging something (a text selection, a list row) past
  the edge of a scroll area scrolls it, faster the farther out.
- **Tables**: `Table::new(id).column(Column::new("Name").width(..)
  .sortable(true).align(..)).show(ui, rows, &mut selection, |ui, row,
  col| ..)`: a header that stays at the top, columns resized by dragging
  the header's edge (double-click fits the content), sorting by clicking
  a header (`TableResponse::sort`, the app sorts its rows), the first
  columns fixed while the others scroll sideways (`sticky_columns`),
  striped rows, selection with clicks and arrow / Page / Home / End keys
  (kept visible), double-clicked rows, and only the visible rows laid
  out. `show_with_heights` takes a height per row, e.g. for expanded
  rows. New `table` example with 100 000 parts.
- **Trees**: `Tree::new(id).show(ui, &roots, |node| children, &mut
  selection, |ui, node| ..)` for nodes that are small `Copy` ids: open
  and close with the arrow, a double-click or →/←, select with clicks and
  the arrow keys (← goes to the parent, → to the first child), only the
  visible rows laid out. `default_open_depth`, `max_height`, `row_height`.
  The `table` example has a tree of 100 000 nodes.
- **Floating scroll bars** (`Spacing::floating_scrollbars`): over the
  content, thin until hovered, fading out a second after scrolling.

## [0.9.0] — 2026-10-09

### Added

- **Closing from the app**: `Frame::close()` closes the window without
  asking `App::on_close_requested` (e.g. after a "save changes?" dialog);
  `Frame::close_requested()`, `RunOutput::close` for `Integration` and
  `Harness::close_requested()` for tests.
- **Unified title bar** (macOS): `WindowOptions::unified_titlebar` makes
  the title bar transparent with the content under it, so the app draws
  its menu bar next to the window buttons; `Frame::titlebar_height()`
  tells how much space it takes (28 points, 0 elsewhere). The
  `containers` example uses it.
- **Modal dialogs**: `Modal::new(id).title(..).width(..).show(frame, |ui|
  ..)` draws a centered card over a veil that blocks every click below;
  Tab only moves between the dialog's widgets, focus behind it is
  removed, and Escape (or, with `close_on_click_outside`, a click on the
  veil) sets `ModalResponse::should_close`. `Context::is_modal_open()`
  tells the app to ignore shortcuts. New layer order `Order::Modal`
  (between windows and popups) and `Visuals::modal_backdrop`.
- **Per-widget style**: `.fill(..)`, `.stroke(..)`, `.corner_radius(..)`
  and `.min_size(..)` on `Button`, `TextEdit`, `ComboBox` and
  `ToolButton`, without changing the global `Style`. `min_size` replaces
  the style's `interact_height` as the minimum height, so it can also
  make a widget smaller (e.g. 22-point fields); `TextEdit::margin(..)`
  sets the space around the text.
- **Search fields**: `SearchField::new(&mut text)` with a magnifying
  glass, a hint ("Search…") and a × that clears the text (`changed()` is
  true then, and the field keeps focus).
- **Two-state icon buttons**: `IconToggle::new(&mut on, on_icon,
  off_icon, label)`, frameless, with a tooltip and announced as a
  checkbox (e.g. eye open / closed in a tree).
- **Submenus**: `ui.menu_button(..)` inside a menu is an item with a ▸
  that opens a submenu to its right on hover or click; hovering another
  item closes it, Escape closes only the innermost submenu, and choosing
  an item closes the whole menu.
- **Text fields**: `TextEdit::password(true)` shows • for every
  character (copying is disabled, screen readers get the masked text);
  double-click selects a word and triple-click a line, and dragging
  afterwards extends the selection by whole words or lines.
- **Multiple clicks**: `PointerState::click_count()` and
  `Response::double_clicked()` / `triple_clicked()` (reported on the
  press).
- **Links**: `Hyperlink::new(url)`, `Hyperlink::from_label_and_url(..)`,
  `Hyperlink::action(text)` and `ui.hyperlink(..)` / `ui.hyperlink_to(..)`:
  accent-colored text, underlined on hover, with a pointing-hand cursor.
  Clicking calls `Context::open_url`, which sets `FrameOutput::open_url`;
  rustroke-winit opens it in the browser. New role `WidgetRole::Link`.
- **Window height**: dragging a `Window`'s corner now changes its
  height too; from then on (or with `Window::default_height`) content
  that doesn't fit is clipped and a `ScrollArea` inside fills the height.
- `Context::pointer_move_needs_frame(pos)` and
  `Context::request_pointer_moves()` (see Changed).
- The `lists` example filters its features with a search field, uses eye
  toggles, and shows a feature timeline with a draggable rollback marker
  built from `ui.interact`, the painter and tooltips on free areas.

### Changed

- **Fewer redraws**: moving the pointer over empty space or plain text
  no longer draws a frame; moves still do over interactive widgets and
  widgets with tooltips, when the hovered widget changes and while
  dragging. Apps that draw something following the pointer elsewhere
  call `ctx.request_pointer_moves()` every frame. Consecutive pointer
  moves between frames are merged into one event.
- `WindowOptions` has a new field, `unified_titlebar`: struct literals
  need `..Default::default()`.
- `Order` has a new variant, `Modal`, and `Visuals` a new field,
  `modal_backdrop`. `FrameOutput` has a new field, `open_url`, and
  `WidgetRole` a new variant, `Link`.

## [0.8.0] — 2026-10-09

### Added

- **Font weights**: `TextStyle::weight` (100–900; 500 medium, 600
  semibold...) and `TextStyle::weight(..)`; `bold` still means at least
  700. The bundled Inter is now the variable font, so every weight is
  exact.
- **The platform's UI font**: `FontFamily::System` / `TextStyle::system(size)`
  (SF Pro on macOS, Segoe UI on Windows, a common desktop font on Linux;
  Inter when unavailable). `Fonts::system_family()` tells which one.
- **Popups below any rectangle**: `Ui::popup_below(id, anchor, ..)` and
  `popup_below_with_width`, with `Context::toggle_popup(id)`, for menus
  opened by buttons the app draws itself.
- **Tool buttons**: `ToolButton::new(icon, label).shortcut_text(..)
  .selected(..)` with `show(ui)` or `show_with_menu(ui, ..)` (a ▾ opening
  the tool's variants); the tooltip shows the name and shortcut.
- **Property grids**: `PropertyGrid::new(id).header(icon, name).show(ui,
  |grid| ..)` with `grid.section(title, default_open, ..)` and
  `grid.row(name, |ui| value)`: names aligned in one column across all
  sections. `ReferenceField` shows a reference with a × to clear it.
- **Rounded images**: `Image::corner_radius(..)`,
  `DisplayList::image_rounded(..)` and `Shape::Image::corner_radius`
  (anti-aliased, e.g. a 3D viewport in a card).
- `CollapsingHeader::text_style(..)`.

### Changed

- The bundled font file is `InterVariable.ttf` (family "Inter Variable")
  instead of Inter Regular and Bold; text looks the same.
- `Shape::Image` has a new field, `corner_radius`.

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

[Unreleased]: https://github.com/AndreCusimano/rustroke/compare/v0.17.0...HEAD
[0.17.0]: https://github.com/AndreCusimano/rustroke/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/AndreCusimano/rustroke/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/AndreCusimano/rustroke/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/AndreCusimano/rustroke/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/AndreCusimano/rustroke/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/AndreCusimano/rustroke/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/AndreCusimano/rustroke/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/AndreCusimano/rustroke/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/AndreCusimano/rustroke/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/AndreCusimano/rustroke/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/AndreCusimano/rustroke/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/AndreCusimano/rustroke/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/AndreCusimano/rustroke/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/AndreCusimano/rustroke/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/AndreCusimano/rustroke/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/AndreCusimano/rustroke/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/AndreCusimano/rustroke/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/AndreCusimano/rustroke/releases/tag/v0.1.0
