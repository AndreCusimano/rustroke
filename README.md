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
rustroke = { git = "https://github.com/AndreCusimano/rustroke" }
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

## Features

- **Widgets**: labels, buttons, checkboxes, radio buttons, selectable labels,
  sliders, drag values (drag or type a number), combo boxes, progress bars,
  spinners, lists that can be selected and reordered by dragging, single and
  multi-line text fields (selection, clipboard, undo, word navigation, input
  methods for accented and Asian text), images, SVG icons, separators. Any
  widget can be disabled.
- **Layout**: rows, columns, alignment, wrapping, fixed-size widgets, grids
  with aligned columns.
- **Containers**: top/bottom/side panels (resizable or as wide as their
  content), movable and resizable windows, scroll areas (vertical,
  horizontal, both), popup and context menus, collapsible sections and
  trees, tab bars, dockable panels (drag tabs between groups or to a side to
  split), tooltips.
- **Text**: shaping, bidirectional text and wrapping via cosmic-text. The Inter
  font and a symbol font (arrows, math, technical symbols) are bundled; system
  fonts are used for emoji and other scripts.
- **Icons**: SVG icons rasterized at any size and screen density, recolored
  from the theme (line and accent colors), so one icon set fits light and
  dark themes.
- **Drawing**: anti-aliased rectangles (rounded), circles, lines, polygons,
  text and images, pixel-aligned on HiDPI screens.
- **Style**: dark and light themes, accent colors, animated transitions,
  changeable at runtime.
- **Accessibility**: screen readers (VoiceOver, Narrator, Orca) through
  AccessKit.
- **Mouse and keyboard**: clicks and drags with any button, Tab navigation,
  Enter/Space activation, arrow keys, keyboard shortcuts shown in menus.
- **Your own GPU rendering**: render a 3D viewport (or anything) with wgpu
  into your own texture and show it in the UI without copies, or draw with
  your own shaders inside the UI's render pass (paint callbacks).
- **More windows**: extra native windows (e.g. a view on a second monitor),
  sharing the app's state.
- **Your own event loop**: embed the UI in an app that already owns its
  winit window and wgpu device (`rustroke::Integration`).
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

Version 0.7: the core is complete and tested (interaction tests without a
window, GPU snapshot tests, CI on macOS, Windows and Linux). Known limitations
are listed in the [changelog](CHANGELOG.md). Coming next: extending and
customizing shapes, and a visual screen designer built with Rustroke itself.

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
