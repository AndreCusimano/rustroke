# Rustroke

I got fed up with graphics libraries that are slow, overcomplicated and packed
with stuff nobody needs, where drawing a simple rectangle means reading half
the docs first.

So I built my own: **simple, fast, and up and running in a couple of lines.**

And yes, you can use it in your commercial projects too: it's dual-licensed
under **MIT** and **Apache 2.0**, so no worries.

---

Rustroke is an **immediate-mode GUI library for Rust** with its own GPU
renderer (wgpu). Your app keeps its own state; every frame you describe the
UI, and widgets tell you what the user did. No callbacks, no widget trees to
keep in sync.

## Quick start

```toml
[dependencies]
rustroke = "0.17"
```

A window with a label, in a couple of lines:

```rust,no_run
fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(Default::default(), |frame: &mut rustroke::Frame| {
        frame.ui(|ui| ui.label("Hello, Rustroke!"));
    })
}
```

Drawing a rectangle is one line:

```rust,no_run
use rustroke::{Color, Rect, point, vec2};

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(Default::default(), |frame: &mut rustroke::Frame| {
        let rect = Rect::from_min_size(point(40.0, 40.0), vec2(200.0, 120.0));
        frame.shapes.rect_filled(rect, 12.0, Color::from_srgb8(137, 180, 250));
    })
}
```

A small app with state:

```rust,no_run
use rustroke::{App, Frame, Slider, WindowOptions};

struct Counter {
    count: i32,
    step: i32,
}

impl App for Counter {
    fn update(&mut self, frame: &mut Frame) {
        frame.ui(|ui| {
            ui.heading("Counter");
            ui.horizontal(|ui| {
                if ui.button("−").clicked() {
                    self.count -= self.step;
                }
                ui.label(self.count.to_string());
                if ui.button("+").clicked() {
                    self.count += self.step;
                }
            });
            ui.add(Slider::new(&mut self.step, 1..=10).text("Step"));
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(WindowOptions::default(), Counter { count: 0, step: 1 })
}
```

## Gallery

A few things Rustroke can do: the code on the left draws the image on the
right (the images are rendered from this code by
`cargo test -p rustroke --test gallery --features markdown -- --ignored`).

<table>
<tr>
<td width="50%">

**Widgets**

```rust,ignore
ui.heading("Settings");
ui.checkbox(&mut dark, "Dark theme");
ui.add(Slider::new(&mut volume, 0..=100)
    .text("Volume"));
ComboBox::from_label("Quality")
    .selected_text(quality)
    .show_ui(ui, |ui| {
        for q in ["Low", "Medium", "High"] {
            ui.selectable_value(&mut quality, q, q);
        }
    });
ui.add(TextEdit::singleline(&mut name)
    .hint_text("Your name"));
ui.horizontal(|ui| {
    ui.button("Cancel");
    ui.button("Save");
});
```

</td>
<td width="50%"><img src="https://raw.githubusercontent.com/AndreCusimano/rustroke/main/docs/images/widgets.png" width="100%" alt="A settings form with a checkbox, a slider, a combo box, a text field and buttons"></td>
</tr>
<tr>
<td width="50%">

**Panels, menus, trees and grids**

```rust,ignore
Panel::top("menu").show(frame, |ui| {
    ui.horizontal(|ui| {
        ui.menu_button("File", |ui| ui.button("Open"));
        ui.menu_button("Edit", |ui| ui.button("Undo"));
        ui.menu_button("View", |ui| ui.button("Zoom"));
    });
});
Panel::bottom("status").show(frame, |ui| ui.label("Ready"));
Panel::left("tree").show(frame, |ui| {
    let parts = CollapsingHeader::new("Parts");
    parts.default_open(true).show(ui, |ui| {
        ui.label("Housing");
        ui.label("Cover");
        ui.label("Screws");
    });
});
CentralPanel.show(frame, |ui| {
    ui.heading("Housing");
    let grid = Grid::new("props").striped(true);
    grid.show(ui, |ui| {
        ui.label("Material");
        ui.label("Aluminium");
        ui.end_row();
        // ...
    });
});
```

</td>
<td width="50%"><img src="https://raw.githubusercontent.com/AndreCusimano/rustroke/main/docs/images/layout.png" width="100%" alt="A menu bar, a side panel with a tree, a status bar and a property grid"></td>
</tr>
<tr>
<td width="50%">

**Tables (sortable, resizable, millions of rows)**

```rust,ignore
Table::new("parts")
    .column(Column::new("Part").sortable(true))
    .column(Column::new("Material"))
    .column(Column::new("Qty").align(Align::Max))
    .show(ui, rows, &mut selected, |ui, row, col| {
        let (part, material, qty) = parts[row];
        match col {
            0 => ui.label(part),
            1 => ui.label(material),
            _ => ui.label(qty.to_string()),
        };
    });
```

</td>
<td width="50%"><img src="https://raw.githubusercontent.com/AndreCusimano/rustroke/main/docs/images/table.png" width="100%" alt="A table of parts with a selected row"></td>
</tr>
<tr>
<td width="50%">

**Rich text and Markdown**

```rust,ignore
let mut job = LayoutJob::default();
job.append("Rich text: ", TextFormat::new());
let bold = TextStyle::proportional(14.0).bold();
job.append("bold", TextFormat::new().style(bold));
job.append(", ", TextFormat::new());
job.append("colored", TextFormat::new().color(pink));
job.append(" and a ", TextFormat::new());
let docs = "https://docs.rs/rustroke";
job.append("link", TextFormat::new().link(docs));
ui.add(Label::rich(job));

ui.markdown(
    "## Markdown\n\
     - **bold**, *italic*, `code`\n\
     - [x] task lists\n\n\
     > quotes, tables, code blocks…",
);
```

</td>
<td width="50%"><img src="https://raw.githubusercontent.com/AndreCusimano/rustroke/main/docs/images/text.png" width="100%" alt="Rich text with bold, colored and linked words, and rendered Markdown"></td>
</tr>
<tr>
<td width="50%">

**Drawing**

```rust,ignore
let s = &mut frame.shapes;
let card = Rect::from_min_size(
    point(30.0, 30.0), vec2(170.0, 110.0));
s.shadow(card, 14.0, Shadow {
    offset: vec2(0.0, 10.0),
    blur: 24.0,
    spread: 0.0,
    color: Color::BLACK.with_alpha(0.6),
});
let sky = Gradient::linear(
    card.left_top(), card.right_bottom(), blue, pink);
s.rect_gradient(card, 14.0, sky, Stroke::NONE);
s.cubic_bezier([a, b, c, d], Stroke::new(3.0, green));
s.dashed_line(&[e, f], Stroke::new(2.0, blue), 9.0, 6.0);
let turn = Transform::rotate_around(center, 0.4);
s.with_transform(turn, |s| {
    s.rect_stroke(r, 8.0, Stroke::new(2.5, pink));
});
```

</td>
<td width="50%"><img src="https://raw.githubusercontent.com/AndreCusimano/rustroke/main/docs/images/graphics.png" width="100%" alt="A gradient card with a soft shadow, a Bézier curve, a dashed line and a rotated rectangle"></td>
</tr>
<tr>
<td width="50%">

**Dialogs and toasts**

```rust,ignore
if exported {
    let done = Toast::success("Exported drawing.pdf");
    frame.ctx().toast(done);
}
let dialog = Modal::new("save").title("Save changes?");
dialog.show(frame, |ui| {
    ui.label("Your changes are lost if you don't save them.");
    ui.horizontal(|ui| {
        ui.button("Don't save");
        ui.button("Cancel");
        ui.button("Save");
    });
});
```

</td>
<td width="50%"><img src="https://raw.githubusercontent.com/AndreCusimano/rustroke/main/docs/images/dialog.png" width="100%" alt="A modal dialog over the dimmed window, and a toast in the corner"></td>
</tr>
</table>

## Features

- **Widgets**: labels, buttons, checkboxes, radio buttons, selectable labels,
  sliders, drag values (drag or type a number), combo boxes, progress bars,
  spinners, lists that can be selected and reordered by dragging, tables
  (sortable and resizable columns, sticky columns, millions of rows) and
  trees that only lay out their visible rows, single and
  multi-line text fields (selection, double/triple click, clipboard, undo,
  word navigation, input methods for accented and Asian text, passwords),
  search fields, links, images (also with rounded corners), SVG icons,
  two-state icon buttons, color, date and time pickers, toasts, toolbar
  buttons with menus of variants, property
  grids, separators. Any widget can be disabled, and buttons and fields
  can have their own colors, borders, radius and size.
- **Layout**: rows, columns, alignment, wrapping, fixed-size widgets, grids
  with aligned columns, flexbox (grow, wrap, justify) and CSS-style grids
  (fractional columns, spans) computed with taffy.
- **Containers**: top/bottom/side panels (resizable or as wide as their
  content), movable and resizable windows, modal dialogs, scroll areas
  (vertical, horizontal, both), popup and context menus with submenus,
  collapsible sections and
  trees, tab bars, dockable panels (drag tabs between groups or to a side to
  split), tooltips.
- **Rich text**: labels mixing styles, colors, highlights, underline and
  links; Markdown documents (optional `markdown` feature: headings, lists,
  quotes, code blocks, tables, images).
- **Text**: shaping, bidirectional text (right-to-left paragraphs aligned
  and selected correctly) and wrapping via cosmic-text. The Inter
  font (variable: every weight from thin to black), the platform's UI font
  (SF Pro, Segoe UI) on request, and a symbol font (arrows, math, technical symbols) are bundled; system
  fonts are used for emoji and other scripts.
- **Icons**: SVG icons rasterized at any size and screen density, recolored
  from the theme (line and accent colors), so one icon set fits light and
  dark themes.
- **Drawing**: anti-aliased rectangles (rounded), circles, lines, polygons,
  text and images, pixel-aligned on HiDPI screens; linear and radial
  gradients, soft shadows, dashed and dotted lines, Bézier curves, 2D
  transforms (rotated and sheared shapes, text and images), custom meshes
  and animated images (GIF, APNG, WebP).
- **Style**: dark and light themes, accent colors, animated transitions,
  changeable at runtime.
- **Accessibility**: screen readers (VoiceOver, Narrator, Orca) through
  AccessKit.
- **Mouse and keyboard**: clicks and drags with any button, Tab navigation,
  Enter/Space activation, arrow keys, keyboard shortcuts shown in menus.
- **Your own GPU rendering**: render a 3D viewport (or anything) with wgpu
  into your own texture and show it in the UI without copies, or draw with
  your own shaders inside the UI's render pass (paint callbacks).
- **Drag and drop and platform**: drag payloads between widgets, files
  dropped from the system, pinch/rotate gestures and touch, a macOS menu
  bar and a tray icon (optional `native-menu` and `tray` features), system
  notifications.
- **More windows**: extra native windows (e.g. a view on a second monitor),
  sharing the app's state. On macOS the app can draw its own top bar next
  to the window buttons (unified title bar).
- **In the browser**: the same app compiles to WebAssembly and runs in a
  canvas with WebGPU or WebGL 2 (`tools/web.sh <example>` builds one).
- **No GPU needed for tests**: a software renderer draws frames on the CPU
  (`Harness::render_software`).
- **Your own event loop**: embed the UI in an app that already owns its
  winit window and wgpu device (`rustroke::Integration`).
- **State and tools**: UI state, window geometry and app values saved
  between runs (optional `persistence` feature), global zoom (Cmd/Ctrl +
  = / - / 0), a debugging inspector (Cmd/Ctrl+Alt+I), automation from
  another thread, and accessibility trees in headless tests.
- **Testing**: run your app without a window, click widgets by label, type
  text, check the result and render the frame to a PNG
  (`rustroke::testing::Harness`).

Desktop: macOS, Windows, Linux. Minimum Rust version: 1.89.

## Performance

- **Idle apps use no CPU**: the window only redraws on input, animations or
  timers.
- A frame with **1000 widgets** (labels, buttons, checkboxes, sliders) takes
  about **1.8 ms of CPU** for layout, interaction and tessellation (release
  build, Apple M1 Max, one core) — a 60 FPS frame has 16.7 ms.
- Text is shaped and rasterized once, then cached; everything is drawn with a
  handful of GPU draw calls.

## Examples

```sh
cargo run -p rustroke --example widgets     # buttons, checkboxes, radios, sliders, keyboard focus
cargo run -p rustroke --example properties  # tree, property panel, combo boxes, drag values, shortcuts
cargo run -p rustroke --example lists       # SVG icons, reorderable list, horizontal scrolling
cargo run -p rustroke --example docking     # document tabs and dockable panels
cargo run -p rustroke --example windows     # a second native window
cargo run -p rustroke --example files       # open and save files with native dialogs (rfd)
cargo run -p rustroke --example custom_wgpu # your own shader inside the UI (paint callback)
cargo run -p rustroke --example integration # the UI inside your own winit loop and wgpu device
cargo run -p rustroke --example layout      # rows, alignment, wrapping, grids
cargo run -p rustroke --example containers  # menus, panels, windows, scroll areas, tooltips
cargo run -p rustroke --example table       # 100 000-row table and a large tree
cargo run -p rustroke --example rich_text --features markdown   # rich text, Markdown, right-to-left
cargo run -p rustroke --example graphics    # gradients, shadows, curves, transforms, animation
cargo run -p rustroke --example platform --features native-menu,tray   # drag and drop, files, menu bar, tray
cargo run -p rustroke --example pickers     # color, date and time pickers, toasts
cargo run -p rustroke --example flex        # flexbox and grid layouts
cargo run -p rustroke --example text_input  # text fields: selection, clipboard, IME
cargo run -p rustroke --example themes      # live theme editor
cargo run -p rustroke --example extras      # images, disabled widgets, accessibility
cargo run -p rustroke --example text        # fonts, scripts, emoji
cargo run -p rustroke --example shapes      # the 2D renderer
```

The full guide is in the API docs: `cargo doc -p rustroke --open`.

## Project structure

| Crate | Role |
|---|---|
| `rustroke` | The library to depend on: re-exports everything an app needs |
| `rustroke-widgets` | Immediate-mode API: `Context`, `Ui`, layout, widgets, containers, style, accessibility |
| `rustroke-text` | Text layout and glyph rasterization (cosmic-text) |
| `rustroke-render` | wgpu renderer, on screen or offscreen |
| `rustroke-winit` | Window, event loop and input (winit) |
| `rustroke-core` | Geometry, colors, shapes, tessellation, input types — no I/O |

## Originality and third-party code

- **Written from scratch.** Every line of Rustroke was written for this
  project. No code was copied from other GUI libraries.
- **Inspiration, credited.** The immediate-mode approach and some API concepts
  and names (`Context`, `Ui`, `Response`, `CentralPanel`, `ScrollArea`, layer
  ordering, texture deltas) are inspired by [egui](https://github.com/emilk/egui),
  which is itself licensed under MIT OR Apache-2.0.
- **Nothing from non-permissive or commercial projects.** No code, assets or
  designs come from Slint (GPL/commercial) or any other project whose license
  would restrict your use of Rustroke.
- **Checked.** Before the first release the sources were compared
  automatically with egui, iced and Slint (shared lines and shared token
  sequences). The only overlaps are the unavoidable ones: identical calls to
  the same wgpu/winit APIs and trivial type definitions (e.g. a color with
  `r, g, b, a`), at the same level found between egui and iced themselves.
- **Permissive dependencies only.** Every dependency is under a permissive
  license (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, BSL-1.0, Unlicense),
  enforced in CI by [cargo-deny](deny.toml).
- **Fonts and colors.** The bundled [Inter](https://github.com/rsms/inter) font
  is under the SIL Open Font License 1.1 (included in
  `crates/rustroke-text/fonts/`), which allows embedding in commercial
  software. The bundled symbol font is a subset of Noto Sans Math, Noto Sans
  Symbols and Noto Sans Symbols 2 (SIL Open Font License 1.1, also in
  `crates/rustroke-text/fonts/`). The default themes use colors from the
  [Catppuccin](https://github.com/catppuccin/catppuccin) palette (MIT).

## Status

Version 0.17: widgets, layouts (including flexbox and grids), tables and
trees, rich text and Markdown, gradients and shadows, drag and drop, saved
state, accessibility, desktop and browser builds. Everything is tested
(interaction tests without a window, GPU and software snapshot tests, CI on
macOS, Windows, Linux and WebAssembly). Known limitations are listed in the
[changelog](CHANGELOG.md). Coming next: a visual screen designer built with
Rustroke itself.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

GPU snapshot tests render scenes offscreen and compare them with the PNGs in
`crates/rustroke-render/tests/snapshots/`. After an intended visual change:
`UPDATE_SNAPSHOTS=1 cargo test -p rustroke-render --test snapshots`.

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option. Unless you explicitly state otherwise, any contribution you
submit for inclusion in this project shall be dual licensed as above, without
any additional terms or conditions.
