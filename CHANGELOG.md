# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/AndreCusimano/rustroke/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/AndreCusimano/rustroke/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/AndreCusimano/rustroke/releases/tag/v0.1.0
