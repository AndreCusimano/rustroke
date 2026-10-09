//! Interaction tests: feed synthetic input to a [`Context`] and check the
//! widgets' responses, without any window or GPU.

use rustroke_core::{
    DisplayList, Event, Key, Modifiers, Point, PointerButton, RawInput, Rect, point, vec2,
};
use rustroke_text::Fonts;

use crate::{Align, Context, FrameOutput, Layout, Response, Ui, UiRoot};

struct Harness {
    ctx: Context,
    fonts: Fonts,
    shapes: DisplayList,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        Self {
            ctx: Context::new(),
            fonts: Fonts::bundled_only(),
            shapes: DisplayList::new(),
            time: 0.0,
        }
    }

    /// Runs one frame with `events`, laying out widgets with `add`.
    fn frame(&mut self, events: Vec<Event>, add: impl FnOnce(&mut Ui<'_>)) -> FrameOutput {
        let screen = SCREEN;
        self.frame_with(events, |h| {
            h.ctx.ui(screen, &mut h.fonts, add);
        })
    }

    /// Runs one frame with `events`, building it with `build` (which can
    /// use containers through [`UiRoot`]).
    fn frame_with(&mut self, events: Vec<Event>, build: impl FnOnce(&mut Self)) -> FrameOutput {
        self.time += 1.0 / 60.0;
        self.ctx.begin_frame(RawInput {
            time: self.time,
            screen_rect: SCREEN,
            pixels_per_point: 1.0,
            events,
        });
        build(self);
        let mut output = self.ctx.end_frame();
        self.shapes = std::mem::take(&mut output.shapes);
        output
    }
}

const SCREEN: Rect = Rect::from_min_max(Point::ZERO, Point::new(400.0, 300.0));

impl UiRoot for Harness {
    fn parts(&mut self) -> (&mut Context, &mut Fonts) {
        (&mut self.ctx, &mut self.fonts)
    }
}

fn move_to(pos: Point) -> Event {
    Event::PointerMoved(pos)
}

fn button(pos: Point, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Runs a frame with a single button and returns its response.
fn button_frame(h: &mut Harness, events: Vec<Event>) -> Response {
    let mut response = None;
    h.frame(events, |ui| response = Some(ui.button("Button")));
    response.unwrap()
}

#[test]
fn button_click() {
    let mut h = Harness::new();
    let rect = button_frame(&mut h, vec![]).rect;
    let inside = rect.center();

    let r = button_frame(&mut h, vec![move_to(inside)]);
    assert!(r.hovered() && !r.clicked());

    let r = button_frame(&mut h, vec![button(inside, true)]);
    assert!(r.is_pressed() && !r.clicked());

    let r = button_frame(&mut h, vec![button(inside, false)]);
    assert!(r.clicked() && !r.is_pressed());

    let r = button_frame(&mut h, vec![]);
    assert!(!r.clicked(), "a click is reported only once");
}

#[test]
fn click_within_a_single_frame() {
    let mut h = Harness::new();
    let inside = button_frame(&mut h, vec![]).rect.center();
    let r = button_frame(&mut h, vec![button(inside, true), button(inside, false)]);
    assert!(r.clicked());
}

#[test]
fn releasing_outside_cancels_the_click() {
    let mut h = Harness::new();
    let rect = button_frame(&mut h, vec![]).rect;
    let _ = button_frame(&mut h, vec![button(rect.center(), true)]);
    let outside = point(rect.max.x + 50.0, rect.center().y);
    let r = button_frame(&mut h, vec![move_to(outside)]);
    assert!(r.is_pressed(), "stays pressed while the button is held");
    let r = button_frame(&mut h, vec![button(outside, false)]);
    assert!(!r.clicked());
}

#[test]
fn pressing_elsewhere_and_releasing_on_button_is_not_a_click() {
    let mut h = Harness::new();
    let rect = button_frame(&mut h, vec![]).rect;
    let outside = point(rect.max.x + 50.0, rect.center().y);
    let _ = button_frame(&mut h, vec![button(outside, true)]);
    let r = button_frame(&mut h, vec![move_to(rect.center())]);
    assert!(!r.hovered(), "no hover while dragging from elsewhere");
    let r = button_frame(&mut h, vec![button(rect.center(), false)]);
    assert!(!r.clicked());
}

#[test]
fn checkbox_toggles() {
    let mut h = Harness::new();
    let mut checked = false;
    let mut response = None;
    h.frame(vec![], |ui| {
        response = Some(ui.checkbox(&mut checked, "Check"))
    });
    // Clicking the label part also toggles.
    let rect = response.as_ref().unwrap().rect;
    let label = point(rect.max.x - 2.0, rect.center().y);

    h.frame(vec![button(label, true), button(label, false)], |ui| {
        response = Some(ui.checkbox(&mut checked, "Check"));
    });
    assert!(checked && response.as_ref().unwrap().changed());

    h.frame(vec![], |ui| {
        response = Some(ui.checkbox(&mut checked, "Check"))
    });
    assert!(checked && !response.unwrap().changed());
}

#[test]
fn radio_value_selects() {
    #[derive(Debug, PartialEq, Clone, Copy)]
    enum Choice {
        A,
        B,
    }
    let mut h = Harness::new();
    let mut choice = Choice::A;
    let mut rect_b = None;
    h.frame(vec![], |ui| {
        let _ = ui.radio_value(&mut choice, Choice::A, "A");
        rect_b = Some(ui.radio_value(&mut choice, Choice::B, "B").rect);
    });
    let b = rect_b.unwrap().center();
    h.frame(vec![button(b, true), button(b, false)], |ui| {
        let _ = ui.radio_value(&mut choice, Choice::A, "A");
        let _ = ui.radio_value(&mut choice, Choice::B, "B");
    });
    assert_eq!(choice, Choice::B);
}

#[test]
fn slider_follows_drag_and_snaps() {
    let mut h = Harness::new();
    let mut value = 0.0_f32;
    let mut rect = None;
    h.frame(vec![], |ui| {
        rect = Some(ui.slider(&mut value, 0.0..=10.0).rect)
    });
    let rect = rect.unwrap();
    let y = rect.center().y;
    let start = point(rect.min.x + 10.0, y);

    h.frame(vec![button(start, true)], |ui| {
        let _ = ui.slider(&mut value, 0.0..=10.0);
    });
    // Drag far to the right: clamps to the maximum.
    h.frame(vec![move_to(point(rect.max.x + 300.0, y))], |ui| {
        let _ = ui.slider(&mut value, 0.0..=10.0);
    });
    assert_eq!(value, 10.0);

    // Integer slider: values are whole numbers.
    let mut n = 0_i32;
    let mut response = None;
    h.frame(vec![button(start, false)], |ui| {
        response = Some(ui.slider(&mut n, 0..=100));
    });
    let r = response.unwrap().rect;
    let third = point(r.min.x + 200.0 / 3.0, r.center().y);
    h.frame(vec![button(third, true), button(third, false)], |ui| {
        let _ = ui.slider(&mut n, 0..=100);
    });
    assert!((30..=36).contains(&n), "{n}");
}

#[test]
fn tab_moves_focus_and_enter_clicks() {
    let mut h = Harness::new();
    let mut clicks = [false; 2];
    let run = |h: &mut Harness, events| {
        let mut responses = Vec::new();
        h.frame(events, |ui| {
            let _ = ui.label("not focusable");
            responses.push(ui.button("first"));
            responses.push(ui.button("second"));
        });
        responses
    };
    run(&mut h, vec![]);

    let r = run(&mut h, vec![key(Key::Tab, Modifiers::NONE)]);
    assert!(r[0].has_focus() && r[0].focus_visible() && !r[1].has_focus());

    let r = run(&mut h, vec![key(Key::Tab, Modifiers::NONE)]);
    assert!(r[1].has_focus());

    // Wraps around, and Shift+Tab goes back.
    let r = run(&mut h, vec![key(Key::Tab, Modifiers::NONE)]);
    assert!(r[0].has_focus());
    let r = run(&mut h, vec![key(Key::Tab, Modifiers::SHIFT)]);
    assert!(r[1].has_focus());

    let r = run(&mut h, vec![key(Key::Enter, Modifiers::NONE)]);
    clicks[1] = r[1].clicked();
    clicks[0] = r[0].clicked();
    assert_eq!(clicks, [false, true]);

    let r = run(&mut h, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!r[0].has_focus() && !r[1].has_focus());
}

#[test]
fn clicking_focuses_without_ring_and_empty_space_unfocuses() {
    let mut h = Harness::new();
    let p = button_frame(&mut h, vec![]).rect.center();
    let r = button_frame(&mut h, vec![button(p, true), button(p, false)]);
    assert!(r.has_focus() && !r.focus_visible());

    let empty = point(300.0, 250.0);
    let r = button_frame(&mut h, vec![button(empty, true), button(empty, false)]);
    assert!(!r.has_focus());
}

#[test]
fn slider_arrow_keys_when_focused() {
    let mut h = Harness::new();
    let mut value = 5_i32;
    let tab = key(Key::Tab, Modifiers::NONE);
    let right = key(Key::ArrowRight, Modifiers::NONE);
    h.frame(vec![], |ui| {
        let _ = ui.slider(&mut value, 0..=10);
    });
    h.frame(vec![tab, right.clone(), right], |ui| {
        let _ = ui.slider(&mut value, 0..=10);
    });
    assert_eq!(value, 7);
    h.frame(vec![key(Key::End, Modifiers::NONE)], |ui| {
        let _ = ui.slider(&mut value, 0..=10);
    });
    assert_eq!(value, 10);
}

#[test]
fn only_the_topmost_overlapping_widget_is_hovered() {
    let mut h = Harness::new();
    let add = |ui: &mut Ui<'_>, out: &mut Vec<Response>| {
        let a = ui.button("bottom");
        ui.add_space(-(a.rect.height() + ui.style().spacing.item_spacing.y));
        out.push(a);
        out.push(ui.button("top"));
    };
    let mut r = Vec::new();
    h.frame(vec![], |ui| add(ui, &mut r));
    let p = r[0].rect.min + vec2(5.0, 5.0);

    let mut r = Vec::new();
    h.frame(vec![move_to(p)], |ui| add(ui, &mut r));
    assert!(!r[0].hovered() && r[1].hovered());
}

#[test]
fn hover_change_requests_a_repaint() {
    let mut h = Harness::new();
    let mut rect = None;
    h.frame(vec![], |ui| rect = Some(ui.button("b").rect));
    let rect = rect.unwrap();
    // First frame after the button appears under a still pointer: the hit
    // test used the old (empty) layout, so another frame is needed.
    let mut h2 = Harness::new();
    let out = h2.frame(vec![move_to(rect.center())], |ui| {
        let _ = ui.button("b");
    });
    assert!(out.repaint);
    let out = h2.frame(vec![], |ui| {
        let _ = ui.button("b");
    });
    assert!(!out.repaint);
}

#[test]
fn widgets_stack_vertically() {
    let mut h = Harness::new();
    let mut rects = Vec::new();
    h.frame(vec![], |ui| {
        rects.push(ui.label("one").rect);
        rects.push(ui.button("two").rect);
        rects.push(ui.label("three").rect);
    });
    let spacing = crate::Style::default().spacing.item_spacing.y;
    for pair in rects.windows(2) {
        assert_eq!(pair[1].min.y, pair[0].max.y + spacing);
        assert_eq!(pair[1].min.x, 0.0);
    }
    assert!(!h.shapes.is_empty());
}

// ---- Layout ----

const SPACING: f32 = 8.0;

/// Runs one frame (no input) and returns what `add` collected.
fn layout_frame<T>(h: &mut Harness, add: impl FnOnce(&mut Ui<'_>) -> T) -> T {
    let mut out = None;
    h.frame(vec![], |ui| out = Some(add(ui)));
    out.unwrap()
}

#[test]
fn horizontal_places_side_by_side_and_parent_continues_below() {
    let mut h = Harness::new();
    let (row, below) = layout_frame(&mut h, |ui| {
        let row = ui.horizontal(|ui| {
            [
                ui.button("a").rect,
                ui.button("bb").rect,
                ui.label("text").rect,
            ]
        });
        (row, ui.label("below").rect)
    });
    let [a, b, text] = row.inner;
    assert_eq!(a.min, point(0.0, 0.0));
    assert_eq!(b.min.x, a.max.x + SPACING);
    assert_eq!(text.min.x, b.max.x + SPACING);
    // Shorter widgets are vertically centered in the row.
    assert!((text.center().y - a.center().y).abs() < 0.01);
    assert_eq!(below.min.y, row.response.rect.max.y + SPACING);
    assert_eq!(row.response.rect.width(), text.max.x);
}

#[test]
fn right_to_left_starts_at_the_right_edge() {
    let mut h = Harness::new();
    let [ok, cancel] = layout_frame(&mut h, |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            [ui.button("OK").rect, ui.button("Cancel").rect]
        })
        .inner
    });
    assert_eq!(ok.max.x, 400.0);
    assert_eq!(cancel.max.x, ok.min.x - SPACING);
}

#[test]
fn vertical_centered_and_justified() {
    let mut h = Harness::new();
    let (centered, justified) = layout_frame(&mut h, |ui| {
        let c = ui.vertical_centered(|ui| ui.button("mid").rect).inner;
        let j = ui
            .with_layout(
                Layout::top_down(Align::Min).with_cross_justify(true),
                |ui| ui.button("wide").rect,
            )
            .inner;
        (c, j)
    });
    assert!((centered.center().x - 200.0).abs() < 0.01);
    assert_eq!((justified.min.x, justified.max.x), (0.0, 400.0));
}

#[test]
fn wrapped_rows_continue_on_the_next_line() {
    let mut h = Harness::new();
    let rects = layout_frame(&mut h, |ui| {
        ui.horizontal_wrapped(|ui| {
            (0..12)
                .map(|i| ui.button(format!("button {i}")).rect)
                .collect::<Vec<_>>()
        })
        .inner
    });
    assert!(rects.iter().all(|r| r.max.x <= 400.0));
    let lines = rects.iter().filter(|r| r.min.x == 0.0).count();
    assert!(lines >= 3, "{lines} lines");
    assert!(rects.last().unwrap().min.y > rects[0].max.y);
}

#[test]
fn add_sized_stretches_the_widget() {
    let mut h = Harness::new();
    let r = layout_frame(&mut h, |ui| {
        ui.add_sized(vec2(150.0, 40.0), crate::Button::new("x"))
            .rect
    });
    assert_eq!(r.size(), vec2(150.0, 40.0));
}

#[test]
fn add_space_moves_along_the_main_axis() {
    let mut h = Harness::new();
    let (a, b, c) = layout_frame(&mut h, |ui| {
        let a = ui.label("a").rect;
        ui.add_space(20.0);
        let b = ui.label("b").rect;
        let c = ui
            .horizontal(|ui| {
                ui.add_space(30.0);
                ui.label("c").rect
            })
            .inner;
        (a, b, c)
    });
    assert_eq!(b.min.y, a.max.y + SPACING + 20.0);
    assert_eq!(c.min.x, 30.0);
}

#[test]
fn grid_aligns_columns_after_one_frame() {
    let mut h = Harness::new();
    let run = |h: &mut Harness| {
        let mut out = None;
        let output = h.frame(vec![], |ui| {
            out = Some(
                crate::Grid::new("g")
                    .show(ui, |ui| {
                        let a = [ui.label("x").rect, ui.button("first row").rect];
                        ui.end_row();
                        let b = [
                            ui.label("a much longer label").rect,
                            ui.button("second").rect,
                        ];
                        ui.end_row();
                        [a, b]
                    })
                    .inner,
            );
        });
        (out.unwrap(), output.repaint)
    };
    let (_, repaint) = run(&mut h);
    assert!(
        repaint,
        "first frame measures the columns and asks for another"
    );
    let ([row1, row2], repaint) = run(&mut h);
    assert!(!repaint, "sizes are stable");
    // Second column starts at the same x in both rows, after the widest cell.
    assert_eq!(row1[1].min.x, row2[1].min.x);
    assert!(row1[1].min.x > row2[0].max.x);
    // Rows are stacked.
    assert!(row2[0].min.y > row1[0].max.y);
}

#[test]
fn separator_is_vertical_in_rows() {
    let mut h = Harness::new();
    let (horizontal, vertical) = layout_frame(&mut h, |ui| {
        let h = ui.separator().rect;
        let v = ui.horizontal(|ui| ui.separator().rect).inner;
        (h, v)
    });
    assert_eq!(horizontal.width(), 400.0);
    assert!(vertical.height() > vertical.width());
}

#[test]
fn labels_wrap_only_in_vertical_layouts() {
    let mut h = Harness::new();
    let long = "word ".repeat(40);
    let (wrapped, single) = layout_frame(&mut h, |ui| {
        let w = ui.label(long.clone()).rect;
        let s = ui.horizontal(|ui| ui.label(long.clone()).rect).inner;
        (w, s)
    });
    assert!(wrapped.width() <= 400.0 && wrapped.height() > 30.0);
    assert!(single.width() > 400.0);
}

#[test]
fn style_changes_are_local_to_a_ui() {
    let mut h = Harness::new();
    let (tight, normal) = layout_frame(&mut h, |ui| {
        let tight = ui
            .horizontal(|ui| {
                ui.style_mut().spacing.item_spacing.x = 0.0;
                [ui.button("a").rect, ui.button("b").rect]
            })
            .inner;
        let normal = ui
            .horizontal(|ui| [ui.button("a").rect, ui.button("b").rect])
            .inner;
        (tight, normal)
    });
    assert_eq!(tight[1].min.x, tight[0].max.x);
    assert_eq!(normal[1].min.x, normal[0].max.x + SPACING);
}

#[test]
fn rows_inside_centered_columns_are_centered_after_one_frame() {
    let mut h = Harness::new();
    let run = |h: &mut Harness| {
        let mut out = None;
        let output = h.frame(vec![], |ui| {
            out = Some(
                ui.vertical_centered(|ui| ui.horizontal(|ui| ui.button("centered row").rect).inner)
                    .inner,
            );
        });
        (out.unwrap(), output.repaint)
    };
    let (first, repaint) = run(&mut h);
    assert!(
        repaint && first.min.x == 0.0,
        "first frame: size unknown yet"
    );
    let (second, repaint) = run(&mut h);
    assert!(!repaint);
    assert!((second.center().x - 200.0).abs() < 0.01, "{second:?}");
}

// ---- Containers ----

use crate::{CentralPanel, Panel, ScrollArea, Window};

fn scroll(dy: f32) -> Event {
    Event::Scroll(vec2(0.0, dy))
}

#[test]
fn panels_take_space_from_the_central_area() {
    let mut h = Harness::new();
    let run = |h: &mut Harness| {
        let mut out = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);
        h.frame_with(vec![], |h| {
            out.0 = Panel::top("top")
                .show(h, |ui| ui.label("toolbar"))
                .response
                .rect;
            out.1 = Panel::left("left")
                .default_size(120.0)
                .show(h, |ui| ui.label("side"))
                .response
                .rect;
            out.2 = CentralPanel.show(h, |ui| ui.label("main").rect);
        });
        out
    };
    run(&mut h);
    let (top, left, main) = run(&mut h);
    assert_eq!(top.min, point(0.0, 0.0));
    assert_eq!(top.width(), 400.0);
    assert_eq!(left.min.y, top.max.y);
    assert_eq!(left.width(), 120.0);
    assert!(main.min.x >= left.max.x && main.min.y >= top.max.y);
}

#[test]
fn side_panel_resizes_by_dragging_its_edge() {
    let mut h = Harness::new();
    let mut width = 0.0;
    let mut run = |h: &mut Harness, events| {
        h.frame_with(events, |h| {
            width = Panel::left("left")
                .default_size(100.0)
                .show(h, |_| {})
                .response
                .rect
                .width();
        });
        width
    };
    run(&mut h, vec![]);
    let edge = point(100.0, 150.0);
    run(&mut h, vec![button(edge, true)]);
    run(&mut h, vec![move_to(point(150.0, 150.0))]);
    run(&mut h, vec![button(point(150.0, 150.0), false)]);
    assert_eq!(run(&mut h, vec![]), 150.0);
}

/// A button in the central panel with a window above part of it.
fn window_over_button(
    h: &mut Harness,
    events: Vec<Event>,
    open: &mut bool,
) -> (Response, Option<Rect>) {
    let mut out = None;
    let mut window = None;
    h.frame_with(events, |h| {
        out = Some(CentralPanel.show(h, |ui| {
            ui.add_sized(vec2(300.0, 200.0), crate::Button::new("big"))
        }));
        window = Window::new("Tools")
            .open(open)
            .default_pos(point(100.0, 100.0))
            .show(h, |ui| ui.label("content"))
            .map(|r| r.response.rect);
    });
    (out.unwrap(), window)
}

#[test]
fn windows_cover_widgets_below_them() {
    let mut h = Harness::new();
    let mut open = true;
    window_over_button(&mut h, vec![], &mut open);
    let (_, window) = window_over_button(&mut h, vec![], &mut open);
    let inside_window = window.unwrap().center();
    let (big, _) = window_over_button(&mut h, vec![move_to(inside_window)], &mut open);
    assert!(big.rect.contains(inside_window));
    assert!(!big.hovered(), "the window is on top");
    let (big, _) = window_over_button(
        &mut h,
        vec![button(inside_window, true), button(inside_window, false)],
        &mut open,
    );
    assert!(!big.clicked());
    let outside = point(30.0, 30.0);
    let (big, _) = window_over_button(&mut h, vec![move_to(outside)], &mut open);
    assert!(big.hovered());
}

#[test]
fn window_moves_by_dragging_its_title_and_closes() {
    let mut h = Harness::new();
    let mut open = true;
    window_over_button(&mut h, vec![], &mut open);
    let (_, window) = window_over_button(&mut h, vec![], &mut open);
    let start = window.unwrap().min;
    let title = start + vec2(30.0, 10.0);
    window_over_button(&mut h, vec![button(title, true)], &mut open);
    window_over_button(&mut h, vec![move_to(title + vec2(40.0, 25.0))], &mut open);
    window_over_button(
        &mut h,
        vec![button(title + vec2(40.0, 25.0), false)],
        &mut open,
    );
    let (_, window) = window_over_button(&mut h, vec![], &mut open);
    assert_eq!(window.unwrap().min, start + vec2(40.0, 25.0));

    // The close button sits at the right end of the title bar.
    let w = window.unwrap();
    let close = point(w.max.x - 10.0 - 10.0, w.min.y + 16.0);
    window_over_button(
        &mut h,
        vec![button(close, true), button(close, false)],
        &mut open,
    );
    assert!(!open);
    let (_, window) = window_over_button(&mut h, vec![], &mut open);
    assert!(window.is_none());
}

#[test]
fn clicking_a_window_brings_it_to_the_front() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut buttons = Vec::new();
        h.frame_with(events, |h| {
            for (name, x) in [("A", 40.0), ("B", 120.0)] {
                Window::new(name).default_pos(point(x, 60.0)).show(h, |ui| {
                    buttons.push(ui.add_sized(vec2(250.0, 60.0), crate::Button::new(name)));
                });
            }
        });
        buttons
    };
    run(&mut h, vec![]);
    let r = run(&mut h, vec![]);
    let overlap = point(200.0, r[0].rect.center().y);
    assert!(r[0].rect.contains(overlap) && r[1].rect.contains(overlap));

    // B was opened last, so it is on top where the windows overlap.
    let r = run(&mut h, vec![move_to(overlap)]);
    assert!(!r[0].hovered() && r[1].hovered());

    // Clicking A's title (not covered by B) brings A to the front.
    let a_title = point(60.0, 70.0);
    run(&mut h, vec![button(a_title, true), button(a_title, false)]);
    let r = run(&mut h, vec![move_to(overlap)]);
    assert!(r[0].hovered() && !r[1].hovered());
}

#[test]
fn scroll_area_clips_and_scrolls() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut rects = Vec::new();
        let mut viewport = Rect::NOTHING;
        h.frame(events, |ui| {
            viewport = ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for i in 0..20 {
                        rects.push(ui.button(format!("item {i}")).rect);
                    }
                })
                .response
                .rect;
        });
        (viewport, rects)
    };
    run(&mut h, vec![]);
    let (viewport, rects) = run(&mut h, vec![]);
    assert_eq!(viewport.height(), 100.0);
    let first_y = rects[0].min.y;

    // Wheel down (negative delta) scrolls content up; applied next frame.
    let inside = viewport.center();
    run(&mut h, vec![move_to(inside), scroll(-60.0)]);
    let (_, rects) = run(&mut h, vec![]);
    assert_eq!(rects[0].min.y, first_y - 60.0);

    // Can't scroll past the end.
    run(&mut h, vec![scroll(-10_000.0)]);
    let (viewport, rects) = run(&mut h, vec![]);
    assert!((rects.last().unwrap().max.y - viewport.max.y).abs() < 0.01);

    // Items scrolled out of view can't be clicked.
    let mut clicked = false;
    let hidden = rects[0].center();
    assert!(!viewport.contains(hidden));
    h.frame(vec![button(hidden, true), button(hidden, false)], |ui| {
        ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
            for i in 0..20 {
                clicked |= ui.button(format!("item {i}")).clicked() && i == 0;
            }
        });
    });
    assert!(!clicked);
}

#[test]
fn menu_opens_runs_an_item_and_closes() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut out = (None, None);
        h.frame(events, |ui| {
            let r = ui.menu_button("File", |ui| ui.button("Open").clone());
            out = (Some(r.response), r.inner);
        });
        (out.0.unwrap(), out.1)
    };
    let (menu, item) = run(&mut h, vec![]);
    assert!(item.is_none());
    let m = menu.rect.center();
    let (_, item) = run(&mut h, vec![button(m, true), button(m, false)]);
    assert!(item.is_some(), "menu open");
    let (_, item) = run(&mut h, vec![]);
    let item_pos = item.unwrap().rect.center();
    let (_, item) = run(
        &mut h,
        vec![button(item_pos, true), button(item_pos, false)],
    );
    assert!(item.unwrap().clicked());
    let (_, item) = run(&mut h, vec![]);
    assert!(item.is_none(), "closed after choosing");

    // Reopen, then click elsewhere: closes without activating anything.
    run(&mut h, vec![button(m, true), button(m, false)]);
    run(&mut h, vec![]);
    let away = point(350.0, 250.0);
    run(&mut h, vec![button(away, true), button(away, false)]);
    let (_, item) = run(&mut h, vec![]);
    assert!(item.is_none());
}

#[test]
fn tooltip_appears_after_a_delay() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut rect = Rect::NOTHING;
        let out = h.frame(events, |ui| {
            rect = ui.button("hover me").on_hover_text(ui, "Helpful text").rect;
        });
        (rect, out)
    };
    let (rect, _) = run(&mut h, vec![]);
    let (_, out) = run(&mut h, vec![move_to(rect.center())]);
    let delay = out.repaint_after.expect("a timed repaint for the tooltip");
    assert!(delay > 0.4 && delay <= 0.5);
    let shapes_without = h.shapes.shapes().len();
    h.time += 1.0;
    run(&mut h, vec![]);
    run(&mut h, vec![]);
    assert!(h.shapes.shapes().len() > shapes_without, "tooltip drawn");
}

// ---- Text editing ----

use rustroke_core::ImeEvent;

fn text(s: &str) -> Event {
    Event::Text(s.to_owned())
}

fn press_key(k: Key) -> Event {
    key(k, Modifiers::NONE)
}

fn command(k: Key) -> Event {
    let mut modifiers = Modifiers::NONE;
    if cfg!(target_os = "macos") {
        modifiers.logo = true;
    } else {
        modifiers.ctrl = true;
    }
    key(k, modifiers)
}

/// A frame with one single-line (or multi-line) text field.
fn edit_frame(
    h: &mut Harness,
    s: &mut String,
    multiline: bool,
    events: Vec<Event>,
) -> (Response, FrameOutput) {
    let mut response = None;
    let out = h.frame(events, |ui| {
        response = Some(if multiline {
            ui.text_edit_multiline(s)
        } else {
            ui.text_edit_singleline(s)
        });
    });
    (response.unwrap(), out)
}

/// Creates a field and focuses it by clicking at its right end (cursor
/// at the end of the text).
fn focused_field(h: &mut Harness, s: &mut String, multiline: bool) -> Response {
    let (r, _) = edit_frame(h, s, multiline, vec![]);
    let end = point(r.rect.max.x - 4.0, r.rect.max.y - 6.0);
    let (r, _) = edit_frame(h, s, multiline, vec![button(end, true), button(end, false)]);
    assert!(r.has_focus());
    r
}

#[test]
fn typing_editing_and_cursor_movement() {
    let mut h = Harness::new();
    let mut s = String::from("ciao");
    focused_field(&mut h, &mut s, false);

    let (r, _) = edit_frame(&mut h, &mut s, false, vec![text(" mondo")]);
    assert!(r.changed());
    assert_eq!(s, "ciao mondo");

    edit_frame(
        &mut h,
        &mut s,
        false,
        vec![press_key(Key::Backspace), press_key(Key::Backspace)],
    );
    assert_eq!(s, "ciao mon");

    // Home, then type at the start; Delete removes the next character.
    edit_frame(
        &mut h,
        &mut s,
        false,
        vec![press_key(Key::Home), text("¡"), press_key(Key::Delete)],
    );
    assert_eq!(s, "¡iao mon");

    // Arrows move by character, also over multi-byte ones.
    edit_frame(
        &mut h,
        &mut s,
        false,
        vec![press_key(Key::ArrowLeft), text("_")],
    );
    assert_eq!(s, "_¡iao mon");
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![]);
    assert!(!r.changed());
}

#[test]
fn selection_clipboard_and_select_all() {
    let mut h = Harness::new();
    let mut s = String::from("uno due");
    focused_field(&mut h, &mut s, false);

    // Shift+Left x3 selects "due"; copy puts it on the clipboard.
    let shift_left = key(Key::ArrowLeft, Modifiers::SHIFT);
    let (_, out) = edit_frame(
        &mut h,
        &mut s,
        false,
        vec![
            shift_left.clone(),
            shift_left.clone(),
            shift_left,
            Event::Copy,
        ],
    );
    assert_eq!(out.copied_text.as_deref(), Some("due"));

    // Typing replaces the selection.
    edit_frame(&mut h, &mut s, false, vec![text("tre")]);
    assert_eq!(s, "uno tre");

    // Select all + cut empties the field; paste puts text back.
    let (_, out) = edit_frame(&mut h, &mut s, false, vec![command(Key::A), Event::Cut]);
    assert_eq!(out.copied_text.as_deref(), Some("uno tre"));
    assert_eq!(s, "");
    edit_frame(
        &mut h,
        &mut s,
        false,
        vec![Event::Paste("incollato\nqui".into())],
    );
    assert_eq!(s, "incollatoqui", "single-line fields drop newlines");
}

#[test]
fn enter_submits_single_line_and_breaks_lines_in_multi_line() {
    let mut h = Harness::new();
    let mut s = String::from("a");
    focused_field(&mut h, &mut s, false);
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![press_key(Key::Enter)]);
    assert!(r.lost_focus());
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![text("x")]);
    assert!(!r.has_focus());
    assert_eq!(s, "a");

    let mut h = Harness::new();
    let mut m = String::from("a");
    focused_field(&mut h, &mut m, true);
    edit_frame(&mut h, &mut m, true, vec![press_key(Key::Enter), text("b")]);
    assert_eq!(m, "a\nb");
    // Up moves to the previous row.
    edit_frame(
        &mut h,
        &mut m,
        true,
        vec![press_key(Key::ArrowUp), text("<")],
    );
    assert_eq!(m, "a<\nb");
}

#[test]
fn clicking_places_the_cursor() {
    let mut h = Harness::new();
    let mut s = String::from("abcdef");
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![]);
    // Near the left edge: before the first character.
    let start = point(r.rect.min.x + 9.0, r.rect.center().y);
    edit_frame(
        &mut h,
        &mut s,
        false,
        vec![button(start, true), button(start, false)],
    );
    edit_frame(&mut h, &mut s, false, vec![text("X")]);
    assert_eq!(s, "Xabcdef");
}

#[test]
fn ime_composition_then_commit() {
    let mut h = Harness::new();
    let mut s = String::from("a");
    focused_field(&mut h, &mut s, false);
    let (_, out) = edit_frame(
        &mut h,
        &mut s,
        false,
        vec![Event::Ime(ImeEvent::Preedit("か".into()))],
    );
    assert_eq!(s, "a", "composition is not part of the text yet");
    assert!(out.ime_cursor.is_some(), "focused field asks for IME");
    edit_frame(
        &mut h,
        &mut s,
        false,
        vec![Event::Ime(ImeEvent::Commit("漢".into()))],
    );
    assert_eq!(s, "a漢");
}

#[test]
fn space_and_enter_are_text_not_clicks() {
    let mut h = Harness::new();
    let mut s = String::new();
    focused_field(&mut h, &mut s, true);
    let (r, _) = edit_frame(
        &mut h,
        &mut s,
        true,
        vec![press_key(Key::Space), text(" "), press_key(Key::Enter)],
    );
    assert!(!r.clicked());
    assert_eq!(s, " \n");
}

// ---- Style ----

#[test]
fn animate_bool_moves_smoothly_and_requests_frames() {
    let mut h = Harness::new();
    let id = crate::Id::new("anim");
    let sample = |h: &mut Harness, on: bool, dt: f64| {
        h.time += dt;
        let mut v = 0.0;
        let out = h.frame_with(vec![], |h| v = h.ctx.animate_bool(id, on));
        (v, out.repaint)
    };
    assert_eq!(sample(&mut h, false, 0.0), (0.0, false));
    // Halfway through the default 0.12 s it is in between, and wants frames.
    let (v, repaint) = sample(&mut h, true, 0.0);
    assert!(v == 0.0 && repaint);
    let (v, repaint) = sample(&mut h, true, 0.06 - 1.0 / 60.0);
    assert!(v > 0.3 && v < 1.0 && repaint, "{v}");
    let (v, repaint) = sample(&mut h, true, 0.2);
    assert!(v == 1.0 && !repaint);
    // Reversing mid-way starts from the current value, not from 1.
    sample(&mut h, false, 0.0);
    let (v, _) = sample(&mut h, false, 0.03);
    assert!(v > 0.0 && v < 1.0);
}

#[test]
fn switching_style_takes_effect_immediately() {
    let mut h = Harness::new();
    h.ctx.set_style(crate::Style::light());
    let mut fill = None;
    h.frame(vec![], |ui| fill = Some(ui.style().visuals.background));
    assert_eq!(fill, Some(crate::Visuals::light().background));
}

// ---- Disabled widgets ----

#[test]
fn disabled_widgets_ignore_input_and_are_faded() {
    let mut h = Harness::new();
    let mut checked = false;
    let run = |h: &mut Harness, events, checked: &mut bool| {
        let mut out = None;
        h.frame(events, |ui| {
            let b = ui.add_enabled(false, crate::Button::new("off"));
            let c = ui
                .add_enabled_ui(false, |ui| ui.checkbox(checked, "off too"))
                .inner;
            out = Some((b, c));
        });
        out.unwrap()
    };
    let (b, c) = run(&mut h, vec![], &mut checked);
    let (bp, cp) = (b.rect.center(), c.rect.center());
    let (b, _) = run(
        &mut h,
        vec![button(bp, true), button(bp, false)],
        &mut checked,
    );
    assert!(!b.clicked() && !b.has_focus());
    run(
        &mut h,
        vec![button(cp, true), button(cp, false)],
        &mut checked,
    );
    assert!(!checked);
    // Tab skips disabled widgets.
    let (b, _) = run(&mut h, vec![press_key(Key::Tab)], &mut checked);
    assert!(!b.has_focus());
    // Everything drawn is faded.
    let alpha = crate::Visuals::dark().disabled_alpha;
    let all_faded = h.shapes.shapes().iter().all(|s| match &s.shape {
        rustroke_core::Shape::Text { color, .. } => (color.a - alpha).abs() < 1e-6,
        rustroke_core::Shape::Rect { fill, .. } => fill.a <= alpha + 1e-6,
        _ => true,
    });
    assert!(all_faded);
}

// ---- Images ----

#[test]
fn textures_are_uploaded_once_and_freed_on_drop() {
    use rustroke_core::{ColorImage, TextureId};
    let mut h = Harness::new();
    let image = ColorImage::from_rgba_unmultiplied([2, 1], &[255; 8]);
    let handle = h.ctx.load_texture(image);
    let copy = handle.clone();
    let out = h.frame(vec![], |_| {});
    assert_eq!(out.textures.set.len(), 1);
    assert_eq!(out.textures.set[0].0, handle.id());
    assert!(matches!(handle.id(), TextureId::User(_)));
    let out = h.frame(vec![], |_| {});
    assert!(out.textures.is_empty(), "uploaded only once");
    drop(handle);
    assert!(
        h.frame(vec![], |_| {}).textures.free.is_empty(),
        "a clone is still alive"
    );
    let id = copy.id();
    drop(copy);
    assert_eq!(h.frame(vec![], |_| {}).textures.free, [id]);
}

#[test]
fn image_widget_sizes() {
    use rustroke_core::ColorImage;
    let mut h = Harness::new();
    let handle = h.ctx.load_texture(ColorImage::from_rgba_unmultiplied(
        [200, 100],
        &vec![0; 200 * 100 * 4],
    ));
    let rects = layout_frame(&mut h, |ui| {
        [
            ui.image(&handle).rect,
            ui.add(crate::Image::new(&handle).max_width(50.0)).rect,
            ui.add(crate::Image::new(&handle).size(vec2(10.0, 30.0)))
                .rect,
        ]
    });
    assert_eq!(rects[0].size(), vec2(200.0, 100.0));
    assert_eq!(rects[1].size(), vec2(50.0, 25.0));
    assert_eq!(rects[2].size(), vec2(10.0, 30.0));
    let images = h
        .shapes
        .shapes()
        .iter()
        .filter(|s| matches!(s.shape, rustroke_core::Shape::Image { .. }))
        .count();
    assert_eq!(images, 3);
}

// ---- Accessibility ----

fn a11y_frame(h: &mut Harness, checked: &mut bool, value: &mut f32) -> (Response, FrameOutput) {
    let mut button = None;
    let out = h.frame(vec![], |ui| {
        ui.label("Title");
        button = Some(ui.button("Save"));
        ui.checkbox(checked, "Enabled");
        ui.add(crate::Slider::new(value, 0.0..=10.0).text("Volume"));
        ui.add_enabled(false, crate::Button::new("Disabled"));
    });
    (button.unwrap(), out)
}

fn node_by_label<'a>(
    update: &'a accesskit::TreeUpdate,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    update
        .nodes
        .iter()
        .find(|(_, n)| n.label() == Some(label))
        .map(|(id, n)| (*id, n))
        .unwrap_or_else(|| panic!("no node labelled {label}"))
}

#[test]
fn accessibility_tree_describes_widgets() {
    let mut h = Harness::new();
    let (mut checked, mut value) = (true, 3.0);
    let (_, out) = a11y_frame(&mut h, &mut checked, &mut value);
    assert!(
        out.accesskit_update.is_none(),
        "off until a screen reader asks"
    );

    h.ctx.set_accessibility_active(true);
    let (_, out) = a11y_frame(&mut h, &mut checked, &mut value);
    let update = out.accesskit_update.expect("tree while active");
    // Root window + 5 widgets.
    assert_eq!(update.nodes.len(), 6);
    assert_eq!(update.nodes[0].1.role(), accesskit::Role::Window);
    assert_eq!(update.nodes[0].1.children().len(), 5);

    let (_, label) = node_by_label(&update, "Title");
    assert_eq!(label.role(), accesskit::Role::Label);
    let (_, save) = node_by_label(&update, "Save");
    assert_eq!(save.role(), accesskit::Role::Button);
    assert!(save.supports_action(accesskit::Action::Click));
    let (_, check) = node_by_label(&update, "Enabled");
    assert_eq!(check.toggled(), Some(accesskit::Toggled::True));
    let (_, slider) = node_by_label(&update, "Volume");
    assert_eq!(slider.numeric_value(), Some(3.0));
    assert_eq!(slider.max_numeric_value(), Some(10.0));
    let (_, disabled) = node_by_label(&update, "Disabled");
    assert!(disabled.is_disabled());
    // Bounds are in points; the root scales them to pixels.
    assert!(save.bounds().is_some());
}

#[test]
fn screen_reader_actions_click_and_focus() {
    let mut h = Harness::new();
    let (mut checked, mut value) = (false, 0.0);
    h.ctx.set_accessibility_active(true);
    let (_, out) = a11y_frame(&mut h, &mut checked, &mut value);
    let update = out.accesskit_update.unwrap();
    let request = |action, label: &str| accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node: node_by_label(&update, label).0,
        data: None,
    };

    h.ctx
        .accesskit_action(request(accesskit::Action::Click, "Save"));
    let (button, _) = a11y_frame(&mut h, &mut checked, &mut value);
    assert!(button.clicked());

    h.ctx
        .accesskit_action(request(accesskit::Action::Click, "Enabled"));
    a11y_frame(&mut h, &mut checked, &mut value);
    assert!(checked);

    h.ctx
        .accesskit_action(request(accesskit::Action::Focus, "Save"));
    let (button, out) = a11y_frame(&mut h, &mut checked, &mut value);
    assert!(button.has_focus());
    let update = out.accesskit_update.unwrap();
    assert_eq!(update.focus, node_by_label(&update, "Save").0);
}

// ---- Regressions reported by CAD3D ----

/// LAY-01: a widget reaching under a side panel's resize grip must still
/// be clickable (the grip only takes clicks where there is no widget).
#[test]
fn widgets_win_over_the_panel_resize_grip() {
    let mut h = Harness::new();
    let mut checked = false;
    let mut rect = Rect::NOTHING;
    let run = |h: &mut Harness, events, checked: &mut bool, rect: &mut Rect| {
        h.frame_with(events, |h| {
            Panel::left("side").default_size(120.0).show(h, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(96.0);
                    *rect = ui.checkbox(checked, "").rect;
                });
            });
        });
    };
    run(&mut h, vec![], &mut checked, &mut rect);
    run(&mut h, vec![], &mut checked, &mut rect);
    let under_grip = point(118.0, rect.center().y);
    assert!(rect.contains(under_grip));
    run(
        &mut h,
        vec![button(under_grip, true), button(under_grip, false)],
        &mut checked,
        &mut rect,
    );
    assert!(checked, "the checkbox got the click, not the grip");

    // Where there is no widget the grip still resizes the panel.
    let free = point(120.0, 250.0);
    run(&mut h, vec![button(free, true)], &mut checked, &mut rect);
    run(
        &mut h,
        vec![move_to(point(160.0, 250.0))],
        &mut checked,
        &mut rect,
    );
    run(
        &mut h,
        vec![button(point(160.0, 250.0), false)],
        &mut checked,
        &mut rect,
    );
    let mut width = 0.0;
    h.frame_with(vec![], |h| {
        width = Panel::left("side")
            .default_size(120.0)
            .show(h, |_| {})
            .response
            .rect
            .width();
    });
    assert_eq!(width, 160.0);
}

/// LAY-01 (windows): a widget in a window's bottom-right corner is
/// clickable despite the resize grip there.
#[test]
fn widgets_win_over_the_window_resize_grip() {
    let mut h = Harness::new();
    let mut clicked = false;
    let mut rect = Rect::NOTHING;
    let run = |h: &mut Harness, events, rect: &mut Rect| {
        let mut c = false;
        h.frame_with(events, |h| {
            Window::new("W")
                .default_pos(point(20.0, 20.0))
                .default_width(200.0)
                .show(h, |ui| {
                    let r =
                        ui.add_sized(vec2(ui.available_width(), 40.0), crate::Button::new("wide"));
                    *rect = r.rect;
                    c = r.clicked();
                });
        });
        c
    };
    run(&mut h, vec![], &mut rect);
    run(&mut h, vec![], &mut rect);
    let corner = point(rect.max.x - 2.0, rect.max.y - 2.0);
    clicked |= run(
        &mut h,
        vec![button(corner, true), button(corner, false)],
        &mut rect,
    );
    assert!(clicked);
}

/// LAY-03: a field with a desired width added to a grid column that only
/// had empty cells gets its width (the column grows instead of staying
/// at its old size).
#[test]
fn grid_cells_allow_desired_widths() {
    let mut h = Harness::new();
    let mut text = String::from("40 mm");
    let mut width = 0.0;
    for frame in 0..4 {
        h.frame(vec![], |ui| {
            crate::Grid::new("g").show(ui, |ui| {
                ui.label("Value");
                if frame >= 2 {
                    width = ui
                        .add(crate::TextEdit::singleline(&mut text).desired_width(110.0))
                        .rect
                        .width();
                } else {
                    ui.label("");
                }
                ui.end_row();
            });
        });
    }
    assert_eq!(width, 110.0);
}

// ---- v0.2.0 (CAD3D requests) ----

/// INP-02: responses report where the pointer is.
#[test]
fn responses_report_pointer_positions() {
    let mut h = Harness::new();
    let rect = button_frame(&mut h, vec![]).rect;
    let p = rect.min + vec2(5.0, 6.0);
    let r = button_frame(&mut h, vec![move_to(p)]);
    assert_eq!(r.hover_pos(), Some(p));
    assert_eq!(r.interact_pointer_pos(), None);
    let r = button_frame(&mut h, vec![button(p, true)]);
    assert_eq!(r.interact_pointer_pos(), Some(p));
    let outside = point(390.0, 290.0);
    let r = button_frame(&mut h, vec![move_to(outside)]);
    assert_eq!(r.hover_pos(), None);
    assert_eq!(
        r.interact_pointer_pos(),
        Some(outside),
        "still pressed outside"
    );
}

/// INP-03: a widget can take the scrolling so a parent scroll area doesn't move.
#[test]
fn consumed_scroll_does_not_scroll_the_parent() {
    let mut h = Harness::new();
    let mut taken = vec2(0.0, 0.0);
    let mut first_y = 0.0;
    for (i, events) in [
        vec![],
        vec![],
        vec![move_to(point(10.0, 10.0)), scroll(-80.0)],
        vec![],
    ]
    .into_iter()
    .enumerate()
    {
        h.frame(events, |ui| {
            ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                let r = ui.button("viewport");
                if r.hovered() {
                    let d = ui.input_mut().consume_scroll();
                    if d.y != 0.0 {
                        taken = d;
                    }
                }
                if i == 3 {
                    first_y = r.rect.min.y;
                }
                for k in 0..20 {
                    ui.label(format!("row {k}"));
                }
            });
        });
    }
    assert_eq!(taken, vec2(0.0, -80.0));
    assert_eq!(first_y, 0.0, "the scroll area didn't scroll");
}

/// INT-03: repaint handles call the platform's wake-up from any thread.
#[test]
fn repaint_handle_wakes_the_platform() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut ctx = Context::new();
    ctx.repaint_handle().request_repaint(); // not connected: no-op
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    ctx.set_repaint_callback(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let handle = ctx.repaint_handle();
    std::thread::spawn(move || handle.request_repaint())
        .join()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// INT-05: replacing a texture keeps its id and uploads the new image.
#[test]
fn texture_handles_can_be_updated_in_place() {
    use rustroke_core::ColorImage;
    let mut h = Harness::new();
    let handle = h
        .ctx
        .load_texture(ColorImage::from_rgba_unmultiplied([1, 1], &[0; 4]));
    h.frame(vec![], |_| {});
    handle.set(ColorImage::from_rgba_unmultiplied([2, 1], &[255; 8]));
    let out = h.frame(vec![], |_| {});
    assert_eq!(out.textures.set.len(), 1);
    assert_eq!(out.textures.set[0].0, handle.id());
    assert_eq!(handle.size(), [2, 1]);
    assert!(out.textures.free.is_empty());
}

/// TST-03: the widgets of the last frame are listed even without a screen reader.
#[test]
fn widget_list_is_available_without_accessibility() {
    let mut h = Harness::new();
    let mut on = true;
    h.frame(vec![], |ui| {
        ui.label("Title");
        ui.add(crate::Checkbox::new(&mut on, "").accessible_label("Option"));
    });
    assert!(!h.ctx.is_accessibility_active());
    let labels: Vec<&str> = h
        .ctx
        .widgets()
        .iter()
        .map(|w| w.info.label.as_str())
        .collect();
    assert_eq!(labels, ["Title", "Option"]);
    let option = h.ctx.find_widget("Option").unwrap();
    assert_eq!(option.info.toggled, Some(true));
    assert!(option.focusable);
}

/// STY-01: semantic colors differ between themes.
#[test]
fn semantic_colors_exist_in_both_themes() {
    let (dark, light) = (crate::Visuals::dark(), crate::Visuals::light());
    for (d, l) in [
        (dark.success, light.success),
        (dark.warning, light.warning),
        (dark.error, light.error),
        (dark.info, light.info),
    ] {
        assert_ne!(d, l);
    }
}

// ---- v0.3: more buttons, menus and widgets ----

fn button_by(pos: Point, which: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: which,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn click_at(pos: Point) -> Vec<Event> {
    vec![move_to(pos), button(pos, true), button(pos, false)]
}

/// INP-01: secondary and middle clicks and drags are reported separately.
#[test]
fn other_buttons_click_and_drag() {
    let mut h = Harness::new();
    let rect = button_frame(&mut h, vec![]).rect;
    let p = rect.center();
    let r = button_frame(
        &mut h,
        vec![
            move_to(p),
            button_by(p, PointerButton::Secondary, true),
            button_by(p, PointerButton::Secondary, false),
        ],
    );
    assert!(r.secondary_clicked() && !r.clicked() && !r.middle_clicked());
    assert_eq!(r.clicked_by(), Some(PointerButton::Secondary));

    let drag_frame = |h: &mut Harness, events| {
        let mut response = None;
        h.frame(events, |ui| {
            response = Some(ui.allocate_response(vec2(100.0, 100.0), crate::Sense::DRAG));
        });
        response.unwrap()
    };
    let area = drag_frame(&mut h, vec![]).rect;
    let start = area.center();
    drag_frame(
        &mut h,
        vec![
            move_to(start),
            button_by(start, PointerButton::Middle, true),
        ],
    );
    let r = drag_frame(&mut h, vec![move_to(start + vec2(7.0, 3.0))]);
    assert!(r.dragged_by(PointerButton::Middle));
    assert!(!r.dragged(), "dragged() is for the primary button");
    assert_eq!(r.drag_delta(), vec2(7.0, 3.0));
    let r = drag_frame(
        &mut h,
        vec![button_by(
            start + vec2(7.0, 3.0),
            PointerButton::Middle,
            false,
        )],
    );
    assert!(!r.dragged_by(PointerButton::Middle));
}

/// WID-01: right-click opens a context menu, choosing an item closes it.
#[test]
fn context_menu_opens_on_right_click() {
    let mut h = Harness::new();
    let mut chosen = 0;
    let run = |h: &mut Harness, events, chosen: &mut i32| {
        let mut open = false;
        let mut target = None;
        let mut item_rect = None;
        h.frame(events, |ui| {
            let r = ui.button("Target");
            target = Some(r.rect);
            open = r
                .context_menu(ui, |ui| {
                    let item = ui.button("Delete");
                    item_rect = Some(item.rect);
                    if item.clicked() {
                        *chosen += 1;
                    }
                })
                .is_some();
        });
        (open, target.unwrap(), item_rect)
    };
    let (open, target, _) = run(&mut h, vec![], &mut chosen);
    assert!(!open);
    let p = target.center();
    let (open, ..) = run(
        &mut h,
        vec![
            move_to(p),
            button_by(p, PointerButton::Secondary, true),
            button_by(p, PointerButton::Secondary, false),
        ],
        &mut chosen,
    );
    assert!(open, "opened by the right click");
    // Measure the menu, then click its item.
    let (.., item_rect) = run(&mut h, vec![], &mut chosen);
    let item = item_rect.unwrap().center();
    assert!(item.y > p.y, "the menu is placed at the pointer");
    run(&mut h, click_at(item), &mut chosen);
    assert_eq!(chosen, 1);
    let (open, ..) = run(&mut h, vec![], &mut chosen);
    assert!(!open, "choosing an item closes the menu");
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Axis {
    X,
    Y,
    Z,
}

/// WID-04 and WID-02: a combo box with selectable values.
#[test]
fn combo_box_selects_a_value_and_closes() {
    let mut h = Harness::new();
    let mut axis = Axis::X;
    let run = |h: &mut Harness, events, axis: &mut Axis| {
        let mut item_rects = Vec::new();
        let mut out = None;
        h.frame(events, |ui| {
            let r = crate::ComboBox::from_label("Axis")
                .selected_text(format!("{axis:?}"))
                .show_ui(ui, |ui| {
                    for (v, t) in [(Axis::X, "X"), (Axis::Y, "Y"), (Axis::Z, "Z")] {
                        item_rects.push(ui.selectable_value(axis, v, t).rect);
                    }
                });
            out = Some((r.inner.is_some(), r.response.rect));
        });
        let (open, rect) = out.unwrap();
        (open, rect, item_rects)
    };
    let (open, rect, _) = run(&mut h, vec![], &mut axis);
    assert!(!open);
    let (open, ..) = run(&mut h, click_at(rect.min + vec2(10.0, 10.0)), &mut axis);
    assert!(open);
    let (.., item_rects) = run(&mut h, vec![], &mut axis);
    assert!(
        item_rects[0].width() >= 175.0,
        "the list is as wide as the box"
    );
    let z = item_rects[2].center();
    run(&mut h, click_at(z), &mut axis);
    assert_eq!(axis, Axis::Z);
    let (open, ..) = run(&mut h, vec![], &mut axis);
    assert!(!open, "choosing closes the list");
    let widgets = h.ctx.widgets();
    let combo = widgets.iter().find(|w| w.info.label == "Axis").unwrap();
    assert_eq!(combo.info.value.as_deref(), Some("Z"));
    assert_eq!(combo.info.expanded, Some(false));
}

#[test]
fn selectable_value_changes_once() {
    let mut h = Harness::new();
    let mut v = 1;
    let run = |h: &mut Harness, events, v: &mut i32| {
        let mut rects = Vec::new();
        let mut changed = Vec::new();
        h.frame(events, |ui| {
            for i in 1..=3 {
                let r = ui.selectable_value(v, i, format!("Item {i}"));
                rects.push(r.rect);
                changed.push(r.changed());
            }
        });
        (rects, changed)
    };
    let (rects, _) = run(&mut h, vec![], &mut v);
    let target = rects[1].center();
    let (_, changed) = run(&mut h, click_at(target), &mut v);
    assert_eq!(v, 2);
    assert_eq!(changed, [false, true, false]);
    let (_, changed) = run(&mut h, click_at(target), &mut v);
    assert_eq!(changed, [false, false, false], "already selected");
    let item = h.ctx.find_widget("Item 2").unwrap();
    assert_eq!(item.info.selected, Some(true));
}

fn drag_value_frame(h: &mut Harness, v: &mut f64, events: Vec<Event>) -> Response {
    let mut response = None;
    h.frame(events, |ui| {
        response = Some(ui.add(crate::DragValue::new(v).speed(0.5).suffix(" mm")));
    });
    response.unwrap()
}

/// WID-05: drag, arrows, click to type, Escape cancels.
#[test]
fn drag_value_drags_types_and_cancels() {
    let mut h = Harness::new();
    let mut v = 10.0;
    let rect = drag_value_frame(&mut h, &mut v, vec![]).rect;
    let p = rect.center();
    drag_value_frame(&mut h, &mut v, vec![move_to(p), button(p, true)]);
    let r = drag_value_frame(&mut h, &mut v, vec![move_to(p + vec2(20.0, 0.0))]);
    assert!(r.changed());
    assert_eq!(v, 20.0, "0.5 per point");
    drag_value_frame(&mut h, &mut v, vec![button(p + vec2(20.0, 0.0), false)]);
    assert_eq!(v, 20.0, "a drag is not a click: no typing");
    assert!(
        h.ctx
            .find_widget("mm")
            .is_some_and(|w| w.info.role == crate::WidgetRole::DragValue)
    );

    // Arrow keys while focused (the drag gave it focus).
    drag_value_frame(&mut h, &mut v, vec![press_key(Key::ArrowUp)]);
    assert_eq!(v, 20.5);

    // Click: type a value and press Enter.
    let rect = drag_value_frame(&mut h, &mut v, vec![]).rect;
    drag_value_frame(&mut h, &mut v, click_at(rect.center()));
    let r = drag_value_frame(&mut h, &mut v, vec![text("42,5 mm")]);
    assert!(
        r.has_focus() && !r.changed(),
        "typing applies only at the end"
    );
    let r = drag_value_frame(&mut h, &mut v, vec![press_key(Key::Enter)]);
    assert!(r.changed());
    assert_eq!(v, 42.5);
    drag_value_frame(&mut h, &mut v, vec![]);

    // Escape discards what was typed.
    let rect = drag_value_frame(&mut h, &mut v, vec![]).rect;
    drag_value_frame(&mut h, &mut v, click_at(rect.center()));
    drag_value_frame(&mut h, &mut v, vec![text("7")]);
    let r = drag_value_frame(&mut h, &mut v, vec![press_key(Key::Escape)]);
    assert!(!r.changed());
    assert_eq!(v, 42.5);
    let r = drag_value_frame(&mut h, &mut v, vec![]);
    assert!(!r.has_focus(), "back to the number");
}

#[test]
fn drag_value_respects_range_and_integers() {
    let mut h = Harness::new();
    let mut n = 5_i32;
    let run = |h: &mut Harness, n: &mut i32, events| {
        let mut response = None;
        h.frame(events, |ui| {
            response = Some(ui.add(crate::DragValue::new(n).range(0..=8).speed(0.25)));
        });
        response.unwrap()
    };
    let p = run(&mut h, &mut n, vec![]).rect.center();
    run(&mut h, &mut n, vec![move_to(p), button(p, true)]);
    run(&mut h, &mut n, vec![move_to(p + vec2(6.0, 0.0))]);
    assert_eq!(n, 7, "5 + 6 × 0.25 = 6.5, rounded");
    run(&mut h, &mut n, vec![move_to(p + vec2(100.0, 0.0))]);
    assert_eq!(n, 8, "clamped");
    run(&mut h, &mut n, vec![move_to(p - vec2(2.0, 0.0))]);
    assert_eq!(n, 5, "the whole drag counts, not the last frame's movement");
}

/// WID-06
#[test]
fn progress_bar_and_spinner() {
    let mut h = Harness::new();
    let out = h.frame(vec![], |ui| {
        ui.add(
            crate::ProgressBar::new(0.5)
                .text("3/7")
                .desired_width(100.0),
        );
        ui.spinner();
    });
    assert!(out.repaint, "the spinner animates");
    let bar = h.ctx.find_widget("3/7").unwrap();
    assert_eq!(bar.rect.width(), 100.0);
    assert_eq!(bar.info.numeric.map(|n| n.value), Some(0.5));
    let out = h.frame(vec![], |ui| {
        ui.add(crate::ProgressBar::new(2.0).show_percentage());
    });
    assert!(!out.repaint, "a still bar needs no frames");
    assert!(h.ctx.find_widget("100%").is_some(), "clamped to 1");
}

fn collapsing_frame(
    h: &mut Harness,
    events: Vec<Event>,
    header: crate::CollapsingHeader,
) -> (crate::CollapsingResponse<()>, bool, Rect) {
    let mut out = None;
    let mut body_shown = false;
    h.frame(events, |ui| {
        let r = header.show(ui, |ui| {
            body_shown = true;
            ui.label("body");
        });
        let after = ui.button("After").rect;
        out = Some((r, after));
    });
    let (r, after) = out.unwrap();
    (r, body_shown, after)
}

/// LAY-04
#[test]
fn collapsing_header_opens_and_closes() {
    let mut h = Harness::new();
    let header = || crate::CollapsingHeader::new("Parameters");
    let (r, shown, closed_after) = collapsing_frame(&mut h, vec![], header());
    assert!(!r.open && !shown && r.body_returned.is_none());
    let p = r.header_response.rect.center();
    let (r, ..) = collapsing_frame(&mut h, click_at(p), header());
    assert!(r.open && r.header_response.changed());
    // Let the animation finish.
    for _ in 0..30 {
        collapsing_frame(&mut h, vec![], header());
    }
    let (r, shown, open_after) = collapsing_frame(&mut h, vec![], header());
    assert!(shown && r.body_returned.is_some());
    assert!(
        open_after.min.y > closed_after.min.y + 15.0,
        "the body takes space"
    );
    let body = h.ctx.find_widget("body").unwrap();
    assert!(
        body.rect.min.x >= r.header_response.rect.min.x + 18.0,
        "indented"
    );
    let info = &h.ctx.find_widget("Parameters").unwrap().info;
    assert_eq!(info.expanded, Some(true));

    // Keyboard: Left closes the focused header (focused by the click).
    let (r, ..) = collapsing_frame(&mut h, vec![press_key(Key::ArrowLeft)], header());
    assert!(!r.open);

    let mut h = Harness::new();
    let (r, shown, _) = collapsing_frame(
        &mut h,
        vec![],
        crate::CollapsingHeader::new("Open").default_open(true),
    );
    assert!(r.open && shown);
}

#[test]
fn tree_nodes_select_with_the_label_and_toggle_with_the_triangle() {
    let mut h = Harness::new();
    let node = || crate::CollapsingHeader::new("Part").selected(false);
    let (r, ..) = collapsing_frame(&mut h, vec![], node());
    let rect = r.header_response.rect;
    let (r, ..) = collapsing_frame(&mut h, click_at(rect.center()), node());
    assert!(
        r.header_response.clicked() && !r.open,
        "label click selects"
    );
    let triangle = point(rect.min.x - 10.0, rect.center().y);
    let (r, ..) = collapsing_frame(&mut h, click_at(triangle), node());
    assert!(r.open && !r.header_response.clicked(), "triangle toggles");
}

/// TXT-01: Escape restores the text; the lost-focus reason tells why.
#[test]
fn text_edit_escape_restores_and_reports_reasons() {
    use crate::FocusLost;
    let mut h = Harness::new();
    let mut s = String::from("10");
    focused_field(&mut h, &mut s, false);
    assert!(h.ctx.wants_keyboard_input());
    edit_frame(&mut h, &mut s, false, vec![text("0")]);
    assert_eq!(s, "100");
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![press_key(Key::Escape)]);
    assert_eq!(r.lost_focus_reason(), Some(FocusLost::Cancel));
    assert!(r.changed());
    assert_eq!(s, "10");
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![]);
    assert!(!r.has_focus() && !r.lost_focus());
    assert!(!h.ctx.wants_keyboard_input());

    focused_field(&mut h, &mut s, false);
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![press_key(Key::Enter)]);
    assert_eq!(r.lost_focus_reason(), Some(FocusLost::Submit));
    assert!(r.submitted());

    focused_field(&mut h, &mut s, false);
    let (r, _) = edit_frame(&mut h, &mut s, false, click_at(point(390.0, 290.0)));
    assert_eq!(r.lost_focus_reason(), Some(FocusLost::Other));
}

/// TXT-02: typing replaces the whole value after focusing.
#[test]
fn text_edit_select_all_on_focus() {
    let mut h = Harness::new();
    let mut s = String::from("12.5");
    let run = |h: &mut Harness, s: &mut String, events| {
        let mut response = None;
        h.frame(events, |ui| {
            response = Some(ui.add(crate::TextEdit::singleline(s).select_all_on_focus(true)));
        });
        response.unwrap()
    };
    let rect = run(&mut h, &mut s, vec![]).rect;
    run(&mut h, &mut s, click_at(rect.center()));
    run(&mut h, &mut s, vec![text("7")]);
    assert_eq!(s, "7");
}

#[test]
fn request_focus_gives_a_text_field_focus() {
    let mut h = Harness::new();
    let mut s = String::from("abc");
    let id = crate::Id::new("name field");
    h.ctx.request_focus(id);
    h.frame(vec![], |ui| {
        ui.add(
            crate::TextEdit::singleline(&mut s)
                .id(id)
                .select_all_on_focus(true),
        );
    });
    h.frame(vec![text("x")], |ui| {
        ui.add(
            crate::TextEdit::singleline(&mut s)
                .id(id)
                .select_all_on_focus(true),
        );
    });
    assert_eq!(s, "x");
}

/// INP-04: menu items show their shortcut; the app handles the keys.
#[test]
fn menu_items_show_shortcut_text() {
    let shortcut = rustroke_core::KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
    let mut h = Harness::new();
    let out = h.frame(vec![command(Key::S)], |ui| {
        assert!(ui.input_mut().consume_shortcut(&shortcut));
        ui.add(crate::Button::new("Save").shortcut_text(shortcut.format()));
    });
    let _ = out;
    let with = h.ctx.find_widget("Save").unwrap().rect.width();
    h.frame(vec![], |ui| {
        ui.add(crate::Button::new("Save"));
    });
    let without = h.ctx.find_widget("Save").unwrap().rect.width();
    assert!(with > without + 20.0, "room for the shortcut");
}

// ---- v0.4 ----

/// LAY-06: a horizontal scroll area keeps wide content on one line and
/// scrolls it sideways with the wheel (vertical wheel, no Shift needed).
#[test]
fn horizontal_scroll_area_scrolls_wide_content() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut label = None;
        let mut area = None;
        h.frame(events, |ui| {
            let r = crate::ScrollArea::horizontal().show(ui, |ui| {
                label = Some(ui.label("A very long line of text that would wrap in a 400 point window if it could, but here it does not").rect);
                ui.separator();
            });
            area = Some(r.response.rect);
        });
        (label.unwrap(), area.unwrap())
    };
    run(&mut h, vec![]);
    let (label, area) = run(&mut h, vec![]);
    assert!(label.width() > area.width(), "no wrapping");
    assert!(label.height() < 30.0, "one line");
    assert!(area.height() < 80.0, "as tall as the content plus the bar");
    let p = area.center();
    run(&mut h, vec![move_to(p), Event::Scroll(vec2(0.0, -50.0))]);
    // The new offset shows from the next frame.
    let (moved, _) = run(&mut h, vec![]);
    assert_eq!(moved.min.x, label.min.x - 50.0);
}

#[test]
fn scroll_area_both_directions() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut first = None;
        h.frame(events, |ui| {
            crate::ScrollArea::both()
                .max_height(100.0)
                .max_width(150.0)
                .show(ui, |ui| {
                    for i in 0..20 {
                        let r =
                            ui.label(format!("Row {i} with some extra width to scroll sideways"));
                        if i == 0 {
                            first = Some(r.rect);
                        }
                    }
                });
        });
        first.unwrap()
    };
    run(&mut h, vec![]);
    let start = run(&mut h, vec![]);
    let p = start.min + vec2(20.0, 20.0);
    run(&mut h, vec![move_to(p), Event::Scroll(vec2(-30.0, -40.0))]);
    let moved = run(&mut h, vec![]);
    assert_eq!(moved.min, start.min - vec2(30.0, 40.0));
}

/// LAY-02
#[test]
fn auto_width_panel_fits_its_content() {
    let mut h = Harness::new();
    let mut width = 0.0;
    for _ in 0..3 {
        h.frame_with(vec![], |h| {
            width = crate::Panel::left("auto")
                .auto_width()
                .show(h, |ui| {
                    ui.label("Short");
                    ui.add(crate::Button::new("A wider button here"));
                })
                .response
                .rect
                .width();
        });
    }
    let button = h.ctx.find_widget("A wider button here").unwrap().rect;
    let pad = h.ctx.style().spacing.window_padding;
    assert_eq!(width, (button.width() + 2.0 * pad).round());
}

/// TXT-03: undo and redo while typing; typing within a second is one step.
#[test]
fn text_edit_undo_and_redo() {
    let mut h = Harness::new();
    let mut s = String::from("ab");
    focused_field(&mut h, &mut s, false);
    edit_frame(&mut h, &mut s, false, vec![text("c")]);
    edit_frame(&mut h, &mut s, false, vec![text("d")]);
    h.time += 2.0;
    edit_frame(&mut h, &mut s, false, vec![press_key(Key::Backspace)]);
    assert_eq!(s, "abc");
    let shift_command = Modifiers::COMMAND.plus(Modifiers::SHIFT);
    edit_frame(&mut h, &mut s, false, vec![command(Key::Z)]);
    assert_eq!(s, "abcd", "undo the deletion");
    assert!(
        !h.ctx.input().key_pressed(Key::Z),
        "the field took the shortcut"
    );
    edit_frame(&mut h, &mut s, false, vec![command(Key::Z)]);
    assert_eq!(s, "ab", "typing \"cd\" is one step");
    edit_frame(&mut h, &mut s, false, vec![command(Key::Z)]);
    assert_eq!(s, "ab", "nothing more to undo");
    edit_frame(&mut h, &mut s, false, vec![key(Key::Z, shift_command)]);
    assert_eq!(s, "abcd");
    edit_frame(&mut h, &mut s, false, vec![text("!")]);
    edit_frame(&mut h, &mut s, false, vec![key(Key::Z, shift_command)]);
    assert_eq!(s, "abcd!", "a new edit clears redo");
}

fn list_frame(
    h: &mut Harness,
    items: &mut Vec<&'static str>,
    selection: &mut Vec<usize>,
    events: Vec<Event>,
) -> crate::ListResponse {
    let mut out = None;
    h.frame(events, |ui| {
        out = Some(
            crate::List::new("list")
                .multi_select(true)
                .reorderable(true)
                .show(ui, items, selection, |ui, _, item| {
                    ui.label(*item);
                }),
        );
    });
    out.unwrap()
}

fn click_with_modifiers(pos: Point, modifiers: Modifiers) -> Vec<Event> {
    let b = |pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    vec![move_to(pos), b(true), b(false)]
}

/// WID-03: selection by click, Cmd/Ctrl+click, Shift+click and arrows.
#[test]
fn list_selects_rows() {
    let mut h = Harness::new();
    let mut items = vec!["a", "b", "c", "d"];
    let mut sel = Vec::new();
    list_frame(&mut h, &mut items, &mut sel, vec![]);
    let rows: Vec<Rect> = list_frame(&mut h, &mut items, &mut sel, vec![])
        .rows
        .iter()
        .map(|r| r.rect)
        .collect();
    assert!(rows[1].min.y >= rows[0].max.y, "rows don't overlap");

    let r = list_frame(&mut h, &mut items, &mut sel, click_at(rows[1].center()));
    assert_eq!(r.clicked, Some(1));
    assert!(r.selection_changed);
    assert_eq!(sel, [1]);

    list_frame(
        &mut h,
        &mut items,
        &mut sel,
        click_with_modifiers(rows[3].center(), Modifiers::COMMAND),
    );
    sel.sort_unstable();
    assert_eq!(sel, [1, 3], "Cmd/Ctrl+click adds");

    list_frame(
        &mut h,
        &mut items,
        &mut sel,
        click_with_modifiers(rows[0].center(), Modifiers::SHIFT),
    );
    sel.sort_unstable();
    assert_eq!(
        sel,
        [0, 1, 2, 3],
        "Shift+click: range from the anchor (row 3)"
    );

    list_frame(&mut h, &mut items, &mut sel, click_at(rows[2].center()));
    assert_eq!(sel, [2]);
    list_frame(
        &mut h,
        &mut items,
        &mut sel,
        vec![press_key(Key::ArrowDown)],
    );
    assert_eq!(sel, [3], "the clicked list has focus: arrows move");
    list_frame(&mut h, &mut items, &mut sel, vec![press_key(Key::Home)]);
    assert_eq!(sel, [0]);
    assert!(
        h.ctx
            .widgets()
            .iter()
            .any(|w| w.info.role == crate::WidgetRole::List)
    );
}

/// WID-03: dragging a row over several frames moves the item.
#[test]
fn list_reorders_by_dragging() {
    let mut h = Harness::new();
    let mut items = vec!["a", "b", "c", "d"];
    let mut sel = vec![0];
    list_frame(&mut h, &mut items, &mut sel, vec![]);
    let rows: Vec<Rect> = list_frame(&mut h, &mut items, &mut sel, vec![])
        .rows
        .iter()
        .map(|r| r.rect)
        .collect();
    let start = rows[0].center();
    list_frame(
        &mut h,
        &mut items,
        &mut sel,
        vec![move_to(start), button(start, true)],
    );
    // Move in small steps: the whole drag counts.
    let mut y = start.y;
    let target = rows[2].max.y - 2.0;
    while y < target {
        y = (y + 6.0).min(target);
        list_frame(
            &mut h,
            &mut items,
            &mut sel,
            vec![move_to(point(start.x, y))],
        );
    }
    assert_eq!(items, ["a", "b", "c", "d"], "nothing moves before the drop");
    let r = list_frame(
        &mut h,
        &mut items,
        &mut sel,
        vec![button(point(start.x, y), false)],
    );
    assert_eq!(r.moved, Some((0, 2)));
    assert_eq!(items, ["b", "c", "a", "d"]);
    assert_eq!(sel, [2], "the selection follows the item");
}

#[test]
fn horizontal_scroll_area_in_a_bottom_panel() {
    let mut h = Harness::new();
    let mut last = Vec::new();
    for _ in 0..4 {
        last.clear();
        h.frame_with(vec![], |h| {
            crate::Panel::bottom("timeline").show(h, |ui| {
                crate::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for i in 0..40 {
                            last.push(ui.button(format!("{i}")).rect);
                        }
                    });
                });
            });
        });
    }
    assert!(
        last[0].min.y >= 200.0 && last[0].max.y <= 300.0,
        "inside the window: {:?}",
        last[0]
    );
    assert_eq!(last[0].min.y, last[39].min.y, "one row");
}

const TEST_ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
    <rect x="2" y="2" width="12" height="12" fill="#000"/>
    <circle cx="8" cy="8" r="3" fill="#1E6FFF"/></svg>"##;

/// WID-07 / ICO-04: icons in buttons, alone or before text.
#[test]
fn buttons_with_icons() {
    let mut h = Harness::new();
    let icon = h.fonts.add_svg_icon(TEST_ICON).unwrap();
    let mut rects = Vec::new();
    for _ in 0..2 {
        rects.clear();
        h.frame(vec![], |ui| {
            rects.push(
                ui.add(crate::Button::icon_only(icon).accessible_label("Extrude"))
                    .rect,
            );
            rects.push(ui.add(crate::Button::new("Extrude").icon(icon)).rect);
            rects.push(ui.button("Extrude").rect);
            rects.push(ui.icon(icon).rect);
        });
    }
    assert!(h.ctx.find_widget("Extrude").is_some());
    assert!(rects[0].width() < rects[2].width(), "icon only is narrow");
    assert!(
        (rects[1].width() - rects[2].width() - (16.0 + 6.0)).abs() < 0.5,
        "icon + gap before the text"
    );
    assert_eq!(rects[3].size(), vec2(16.0, 16.0));
    // Two layers per icon: line and accent, drawn as text.
    let texts = h
        .shapes
        .shapes()
        .iter()
        .filter(|s| matches!(s.shape, rustroke_core::Shape::Text { .. }))
        .count();
    assert!(texts >= 2 * 3 + 2, "{texts}");
}

/// INP-07 / INP-08: Escape reaches the app when it closes nothing.
#[test]
fn escape_is_left_to_the_app_when_nothing_closes() {
    let mut h = Harness::new();
    let mut seen = false;
    h.frame(vec![press_key(Key::Escape)], |ui| {
        ui.button("Button");
        seen = ui.input().key_pressed(Key::Escape);
    });
    assert!(seen, "no popup, no focus: the app sees Escape");

    // With a popup open, Escape closes it and is consumed.
    let menu = |h: &mut Harness, events| {
        let mut seen = false;
        let mut open = false;
        let mut rect = Rect::NOTHING;
        h.frame(events, |ui| {
            rect = ui.menu_button("File", |ui| ui.button("New")).response.rect;
            seen = ui.input().key_pressed(Key::Escape);
            open = ui.ctx().any_popup_open();
        });
        (seen, open, rect)
    };
    let (.., rect) = menu(&mut h, vec![]);
    let (_, open, _) = menu(&mut h, click_at(rect.center()));
    assert!(open);
    assert!(h.ctx.open_popup_id().is_some());
    let (seen, open, _) = menu(&mut h, vec![press_key(Key::Escape)]);
    assert!(!open && !seen, "closing the menu took the key");
}

// ---- v0.6: tabs and docking ----

fn tab_bar_frame(
    h: &mut Harness,
    docs: &mut Vec<&'static str>,
    active: &mut usize,
    events: Vec<Event>,
) -> crate::TabBarResponse {
    let mut out = None;
    h.frame(events, |ui| {
        out = Some(
            crate::TabBar::new("docs")
                .add_button(true)
                .show(ui, docs, active, |d| crate::TabLabel::new(*d)),
        );
    });
    out.unwrap()
}

/// Drags from `from` to `to` in small steps, over several frames.
fn drag_events(from: Point, to: Point, steps: usize) -> Vec<Vec<Event>> {
    let mut frames = vec![vec![move_to(from), button(from, true)]];
    for s in 1..=steps {
        let t = s as f32 / steps as f32;
        frames.push(vec![move_to(from + (to - from) * t)]);
    }
    frames.push(vec![button(to, false)]);
    frames
}

/// WID-08: switch, close, add and reorder tabs.
#[test]
fn tab_bar_switches_closes_and_reorders() {
    let mut h = Harness::new();
    let mut docs = vec!["Motor mount", "Base plate", "Assembly"];
    let mut active = 0;
    tab_bar_frame(&mut h, &mut docs, &mut active, vec![]);
    let r = tab_bar_frame(&mut h, &mut docs, &mut active, vec![]);
    let tabs: Vec<Rect> = r.tabs.iter().map(|t| t.rect).collect();
    assert!(tabs[1].min.x > tabs[0].max.x);

    let r = tab_bar_frame(&mut h, &mut docs, &mut active, click_at(tabs[1].center()));
    assert_eq!(r.clicked, Some(1));
    assert_eq!(active, 1);

    // The × sits at the right end of the tab.
    let close = point(tabs[2].max.x - 18.0, tabs[2].center().y);
    let r = tab_bar_frame(&mut h, &mut docs, &mut active, click_at(close));
    assert_eq!(r.close_requested, Some(2));
    assert_eq!(active, 1, "closing doesn't switch");

    let add = point(tabs[2].max.x + 14.0, tabs[2].center().y);
    let r = tab_bar_frame(&mut h, &mut docs, &mut active, click_at(add));
    assert!(r.add_clicked);

    // Drag the first tab after the second.
    let mut last = None;
    for events in drag_events(
        tabs[0].center(),
        point(tabs[1].max.x - 5.0, tabs[0].center().y),
        6,
    ) {
        last = Some(tab_bar_frame(&mut h, &mut docs, &mut active, events));
    }
    assert_eq!(last.unwrap().moved, Some((0, 1)));
    assert_eq!(docs, ["Base plate", "Motor mount", "Assembly"]);
    assert_eq!(active, 0, "the active tab follows its document");
    assert!(
        h.ctx
            .widgets()
            .iter()
            .any(|w| w.info.role == crate::WidgetRole::Tab)
    );
}

struct Viewer {
    closed: Vec<&'static str>,
}

impl crate::DockViewer for Viewer {
    type Tab = &'static str;
    fn label(&mut self, tab: &Self::Tab) -> crate::TabLabel {
        crate::TabLabel::new(*tab)
    }
    fn ui(&mut self, ui: &mut Ui<'_>, tab: &mut Self::Tab) {
        ui.label(format!("Content of {tab}"));
    }
    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        self.closed.push(*tab);
        *tab != "Scene"
    }
}

fn dock_frame(
    h: &mut Harness,
    state: &mut crate::DockState<&'static str>,
    viewer: &mut Viewer,
    events: Vec<Event>,
) {
    h.frame(events, |ui| {
        crate::DockArea::new("dock").show(ui, state, viewer);
    });
}

/// LAY-05: drag tabs between groups and onto a side to split.
#[test]
fn dock_area_moves_tabs_and_splits() {
    let mut h = Harness::new();
    let mut viewer = Viewer { closed: Vec::new() };
    let mut state = crate::DockState::new(vec!["Scene", "Log"]);
    let root = state.groups()[0];
    state.split(
        root,
        crate::SplitAxis::Horizontal,
        false,
        0.5,
        vec!["Properties"],
    );
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    let tab = |h: &Harness, name: &str| h.ctx.find_widget(name).unwrap().rect;
    assert!(
        h.ctx.find_widget("Content of Scene").is_some(),
        "active tab shown"
    );
    assert!(
        h.ctx.find_widget("Content of Log").is_none(),
        "others hidden"
    );

    // "Log" into the right group's tab bar.
    let props = tab(&h, "Properties");
    for events in drag_events(
        tab(&h, "Log").center(),
        point(props.max.x + 10.0, props.center().y),
        8,
    ) {
        dock_frame(&mut h, &mut state, &mut viewer, events);
    }
    assert_eq!(state.groups().len(), 2);
    assert_eq!(state.tabs(), [&"Scene", &"Properties", &"Log"]);

    // "Log" onto the bottom of the left group: a new split.
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    let content = h.ctx.find_widget("Content of Scene").unwrap().rect;
    let bottom = point(content.min.x + 60.0, 290.0);
    for events in drag_events(tab(&h, "Log").center(), bottom, 8) {
        dock_frame(&mut h, &mut state, &mut viewer, events);
    }
    assert_eq!(state.groups().len(), 3);
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    let log = tab(&h, "Log");
    let scene = tab(&h, "Scene");
    assert!(
        log.min.y > scene.max.y + 50.0,
        "below: {log:?} vs {scene:?}"
    );

    // Closing: the viewer can refuse.
    let close = point(scene.max.x - 14.0, scene.center().y);
    dock_frame(&mut h, &mut state, &mut viewer, click_at(close));
    assert_eq!(viewer.closed, ["Scene"]);
    assert_eq!(state.tabs().len(), 3, "kept");
}

#[test]
fn dock_split_line_resizes_groups() {
    let mut h = Harness::new();
    let mut viewer = Viewer { closed: Vec::new() };
    let mut state = crate::DockState::new(vec!["Left"]);
    let root = state.groups()[0];
    state.split(
        root,
        crate::SplitAxis::Horizontal,
        false,
        0.5,
        vec!["Right"],
    );
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    let right = h.ctx.find_widget("Right").unwrap().rect;
    let line = point(right.min.x - 4.0, 150.0);
    for events in drag_events(line, line + vec2(-60.0, 0.0), 4) {
        dock_frame(&mut h, &mut state, &mut viewer, events);
    }
    dock_frame(&mut h, &mut state, &mut viewer, vec![]);
    let moved = h.ctx.find_widget("Right").unwrap().rect;
    assert!(
        (moved.min.x - (right.min.x - 60.0)).abs() < 2.0,
        "{moved:?} {right:?}"
    );
}

// ---- v0.8 ----

/// WID-14: a popup below any rectangle, toggled by the app's own button.
#[test]
fn popup_below_an_app_drawn_button() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut arrow = None;
        let mut item = None;
        let mut open = false;
        let mut picked = false;
        h.frame(events, |ui| {
            let r = ui.add(crate::Button::new("v").frame(false));
            if r.clicked() {
                ui.ctx().toggle_popup(r.id);
            }
            open = ui
                .popup_below(r.id, r.rect, |ui| {
                    let b = ui.button("Revolve");
                    item = Some(b.rect);
                    picked = b.clicked();
                })
                .is_some();
            arrow = Some(r.rect);
        });
        (arrow.unwrap(), item, open, picked)
    };
    let (arrow, ..) = run(&mut h, vec![]);
    let (_, _, open, _) = run(&mut h, click_at(arrow.center()));
    assert!(open);
    let (_, item, ..) = run(&mut h, vec![]);
    let item = item.unwrap();
    assert!(item.min.y >= arrow.max.y, "below the anchor");
    // Clicking the button again closes it (no close-then-reopen).
    let (_, _, open, _) = run(&mut h, click_at(arrow.center()));
    assert!(!open);
    run(&mut h, click_at(arrow.center()));
    let (_, _, _, picked) = run(&mut h, click_at(item.center()));
    assert!(picked);
    let (_, _, open, _) = run(&mut h, vec![]);
    assert!(!open, "choosing closes it");
}

/// WID-09: a tool button with a menu of variants.
#[test]
fn tool_button_with_variants() {
    let mut h = Harness::new();
    let icon = h.fonts.add_svg_icon(TEST_ICON).unwrap();
    let mut tool = 0;
    let run = |h: &mut Harness, events, tool: &mut i32| {
        let mut out = None;
        let mut items = Vec::new();
        h.frame(events, |ui| {
            ui.horizontal(|ui| {
                let r = crate::ToolButton::new(icon, "Extrude")
                    .shortcut_text("E")
                    .selected(*tool == 0)
                    .show_with_menu(ui, |ui| {
                        items.push(ui.selectable_value(tool, 0, "Extrude").rect);
                        items.push(ui.selectable_value(tool, 1, "Revolve").rect);
                    });
                out = Some((
                    r.response.rect,
                    r.arrow.rect,
                    r.response.clicked(),
                    r.inner.is_some(),
                ));
            });
        });
        let (main, arrow, clicked, open) = out.unwrap();
        (main, arrow, clicked, open, items)
    };
    let (main, arrow, ..) = run(&mut h, vec![], &mut tool);
    assert_eq!(arrow.min.x, main.max.x, "the arrow sticks to the icon");
    assert!(h.ctx.find_widget("Extrude").is_some());
    let (_, _, clicked, open, _) = run(&mut h, click_at(main.center()), &mut tool);
    assert!(clicked && !open, "the icon uses the tool");
    let (.., open, _) = run(&mut h, click_at(arrow.center()), &mut tool);
    assert!(open, "the arrow opens the variants");
    let (.., items) = run(&mut h, vec![], &mut tool);
    run(&mut h, click_at(items[1].center()), &mut tool);
    assert_eq!(tool, 1);
    let (.., open, _) = run(&mut h, vec![], &mut tool);
    assert!(!open);
}

/// WID-11: names aligned across sections, values editable, references
/// cleared with ×.
#[test]
fn property_grid_aligns_values_across_sections() {
    let mut h = Harness::new();
    let mut length = 10.0_f64;
    let mut reference = Some("Sketch 1");
    let run = |h: &mut Harness, events, length: &mut f64, reference: &mut Option<&str>| {
        let mut values = Vec::new();
        let mut field = None;
        h.frame(events, |ui| {
            crate::PropertyGrid::new("props")
                .header(None, "Base plate")
                .show(ui, |grid| {
                    grid.section("General", true, |grid| {
                        values.push(grid.row("Name", |ui| ui.label("Base plate")).rect);
                    });
                    grid.section("Parameters", true, |grid| {
                        values.push(
                            grid.row("A much longer name", |ui| {
                                ui.add(crate::DragValue::new(length))
                            })
                            .rect,
                        );
                        let r = grid.row("Sketch", |ui| {
                            ui.add(crate::ReferenceField::new(*reference).width(150.0))
                        });
                        if r.changed() {
                            *reference = None;
                        }
                        field = Some(r.rect);
                    });
                    grid.section("Advanced", false, |grid| {
                        grid.row("Hidden", |ui| ui.label("x"));
                    });
                });
        });
        (values, field.unwrap())
    };
    let _ = run(&mut h, vec![], &mut length, &mut reference);
    let (values, field) = run(&mut h, vec![], &mut length, &mut reference);
    assert_eq!(values[0].min.x, values[1].min.x, "one value column");
    assert!(h.ctx.find_widget("Base plate").is_some());
    assert!(h.ctx.find_widget("Hidden").is_none(), "closed section");
    let clear = point(field.max.x - 14.0, field.center().y);
    run(&mut h, click_at(clear), &mut length, &mut reference);
    assert_eq!(reference, None);
}

/// Background button "Below", plus a modal with "OK" when `open`.
struct ModalFrame {
    below: Response,
    ok: Option<Response>,
    should_close: bool,
}

fn modal_frame(h: &mut Harness, events: Vec<Event>, open: bool) -> ModalFrame {
    let mut below = None;
    let mut ok = None;
    let mut should_close = false;
    h.frame_with(events, |h| {
        h.ctx
            .ui(SCREEN, &mut h.fonts, |ui| below = Some(ui.button("Below")));
        if open {
            let r = crate::Modal::new("dialog").title("Confirm").show(h, |ui| {
                ok = Some(ui.button("OK"));
                ui.button("Cancel");
            });
            should_close = r.should_close;
        }
    });
    ModalFrame {
        below: below.unwrap(),
        ok,
        should_close,
    }
}

#[test]
fn modal_blocks_clicks_and_hover_below() {
    let mut h = Harness::new();
    let below = modal_frame(&mut h, vec![], false).below.rect.center();
    modal_frame(&mut h, vec![], true);
    modal_frame(&mut h, vec![], true); // measured and centered
    let f = modal_frame(&mut h, vec![move_to(below)], true);
    assert!(!f.below.hovered(), "the veil covers the button");
    modal_frame(&mut h, vec![button(below, true)], true);
    let f = modal_frame(&mut h, vec![button(below, false)], true);
    assert!(!f.below.clicked());
    assert!(!f.should_close, "the veil doesn't close by default");
    assert!(h.ctx.is_modal_open());

    let ok = f.ok.unwrap().rect.center();
    modal_frame(&mut h, vec![button(ok, true)], true);
    let f = modal_frame(&mut h, vec![button(ok, false)], true);
    assert!(f.ok.unwrap().clicked(), "widgets in the dialog work");

    // Closed: the button below works again.
    modal_frame(&mut h, vec![], false);
    modal_frame(&mut h, vec![button(below, true)], false);
    let f = modal_frame(&mut h, vec![button(below, false)], false);
    assert!(f.below.clicked());
    assert!(!h.ctx.is_modal_open());
}

#[test]
fn modal_keeps_tab_focus_inside_and_closes_with_escape() {
    let mut h = Harness::new();
    // Focus the button below first.
    let below = modal_frame(&mut h, vec![], false).below.rect.center();
    modal_frame(
        &mut h,
        vec![button(below, true), button(below, false)],
        false,
    );
    assert!(h.ctx.focused().is_some());
    modal_frame(&mut h, vec![], true);
    let f = modal_frame(&mut h, vec![], true);
    assert!(
        !f.below.has_focus(),
        "opening a dialog removes focus behind it"
    );

    let mut focused = Vec::new();
    for _ in 0..4 {
        let f = modal_frame(&mut h, vec![key(Key::Tab, Modifiers::NONE)], true);
        assert!(!f.below.has_focus(), "Tab never leaves the dialog");
        focused.push(f.ok.unwrap().has_focus());
    }
    assert_eq!(
        focused,
        [true, false, true, false],
        "OK, Cancel, OK, Cancel"
    );

    let f = modal_frame(&mut h, vec![key(Key::Escape, Modifiers::NONE)], true);
    assert!(f.should_close, "Escape closes even with a button focused");
}

#[test]
fn modal_can_close_on_a_click_outside() {
    let mut h = Harness::new();
    let frame = |h: &mut Harness, events| {
        let mut close = false;
        h.frame_with(events, |h| {
            close = crate::Modal::new("m")
                .close_on_click_outside(true)
                .show(h, |ui| ui.label("Hi"))
                .should_close;
        });
        close
    };
    frame(&mut h, vec![]);
    frame(&mut h, vec![]);
    let corner = point(5.0, 5.0);
    frame(&mut h, vec![button(corner, true)]);
    assert!(frame(&mut h, vec![button(corner, false)]));
}

/// The rectangle shape drawn exactly at `rect`, if any.
fn rect_shape_at(
    h: &Harness,
    rect: Rect,
) -> Option<(f32, rustroke_core::Color, rustroke_core::Stroke)> {
    h.shapes.shapes().iter().find_map(|s| match s.shape {
        rustroke_core::Shape::Rect {
            rect: r,
            corner_radius,
            fill,
            stroke,
        } if r == rect => Some((corner_radius, fill, stroke)),
        _ => None,
    })
}

#[test]
fn per_widget_frame_style_overrides_the_theme() {
    use rustroke_core::{Color, Stroke};
    let red = Color::from_srgb8(200, 30, 30);
    let mut h = Harness::new();
    let (button, field) = layout_frame(&mut h, |ui| {
        let b = ui.add(
            crate::Button::new("Go")
                .fill(red)
                .stroke(Stroke::new(2.0, red))
                .corner_radius(3.0)
                .min_size(vec2(90.0, 20.0)),
        );
        let mut text = String::from("x");
        let f = ui.add(
            crate::TextEdit::singleline(&mut text)
                .min_size(vec2(60.0, 22.0))
                .margin(vec2(4.0, 1.0))
                .corner_radius(4.0),
        );
        (b, f)
    });
    assert!(button.rect.width() >= 90.0);
    assert!(
        button.rect.height() < 28.0,
        "min_size replaces interact_height: {}",
        button.rect.height()
    );
    let (radius, fill, stroke) = rect_shape_at(&h, button.rect).expect("button frame");
    assert_eq!((radius, fill, stroke), (3.0, red, Stroke::new(2.0, red)));

    assert!(
        (field.rect.height() - 22.0).abs() < 0.5,
        "{}",
        field.rect.height()
    );
    let (radius, ..) = rect_shape_at(&h, field.rect).expect("field frame");
    assert_eq!(radius, 4.0);
}

#[test]
fn search_field_clears_with_the_cross() {
    let mut h = Harness::new();
    let mut text = String::from("bolt");
    let run = |h: &mut Harness, events, text: &mut String| {
        let mut r = None;
        h.frame(events, |ui| r = Some(ui.add(crate::SearchField::new(text))));
        r.unwrap()
    };
    let field = run(&mut h, vec![], &mut text).rect;
    let clear = h
        .ctx
        .find_widget("Clear Search…")
        .expect("× while not empty");
    assert!(field.contains(clear.rect.center()));
    let at = clear.rect.center();
    run(&mut h, vec![button(at, true)], &mut text);
    let r = run(&mut h, vec![button(at, false)], &mut text);
    assert!(r.changed());
    assert_eq!(text, "");
    run(&mut h, vec![], &mut text);
    assert!(
        h.ctx.find_widget("Clear Search…").is_none(),
        "no × when empty"
    );
    let r = run(&mut h, vec![], &mut text);
    assert!(r.has_focus(), "the field keeps focus after clearing");
}

#[test]
fn icon_toggle_switches_and_describes_its_state() {
    let mut h = Harness::new();
    let eye = h.fonts.add_svg_icon(TEST_ICON).unwrap();
    let mut visible = true;
    let run = |h: &mut Harness, events, visible: &mut bool| {
        let mut r = None;
        h.frame(events, |ui| {
            r = Some(ui.add(crate::IconToggle::new(visible, eye, eye, "Visible")));
        });
        r.unwrap()
    };
    let at = run(&mut h, vec![], &mut visible).rect.center();
    assert_eq!(
        h.ctx.find_widget("Visible").unwrap().info.toggled,
        Some(true)
    );
    run(&mut h, vec![button(at, true)], &mut visible);
    let r = run(&mut h, vec![button(at, false)], &mut visible);
    assert!(r.clicked() && r.changed());
    assert!(!visible);
}

/// A menu bar with File › (New, Recent › (a.txt, b.txt)).
fn submenu_frame(h: &mut Harness, events: Vec<Event>, chosen: &mut Option<&'static str>) {
    h.frame(events, |ui| {
        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").clicked() {
                    *chosen = Some("new");
                }
                ui.menu_button("Recent", |ui| {
                    if ui.button("a.txt").clicked() {
                        *chosen = Some("a");
                    }
                    ui.button("b.txt");
                });
            });
        });
    });
}

fn click_label(h: &mut Harness, label: &str, chosen: &mut Option<&'static str>) {
    let at = h.ctx.find_widget(label).unwrap().rect.center();
    submenu_frame(h, vec![move_to(at)], chosen);
    submenu_frame(h, vec![button(at, true)], chosen);
    submenu_frame(h, vec![button(at, false)], chosen);
    submenu_frame(h, vec![], chosen);
}

#[test]
fn submenus_open_on_hover_and_close_with_the_menu() {
    let mut h = Harness::new();
    let mut chosen = None;
    submenu_frame(&mut h, vec![], &mut chosen);
    click_label(&mut h, "File", &mut chosen);
    assert!(h.ctx.find_widget("Recent").is_some());
    assert!(h.ctx.find_widget("a.txt").is_none());

    // Hovering "Recent" opens its submenu.
    let recent = h.ctx.find_widget("Recent").unwrap().rect.center();
    submenu_frame(&mut h, vec![move_to(recent)], &mut chosen);
    submenu_frame(&mut h, vec![], &mut chosen);
    submenu_frame(&mut h, vec![], &mut chosen);
    let a = h.ctx.find_widget("a.txt").expect("submenu open").rect;
    assert!(
        a.min.x >= h.ctx.find_widget("Recent").unwrap().rect.max.x,
        "to the right"
    );

    // Hovering a plain item of the parent menu closes it.
    let new = h.ctx.find_widget("New").unwrap().rect.center();
    submenu_frame(&mut h, vec![move_to(new)], &mut chosen);
    submenu_frame(&mut h, vec![], &mut chosen);
    assert!(h.ctx.find_widget("a.txt").is_none());

    // Escape closes only the submenu.
    submenu_frame(&mut h, vec![move_to(recent)], &mut chosen);
    submenu_frame(&mut h, vec![], &mut chosen);
    submenu_frame(
        &mut h,
        vec![
            move_to(recent + vec2(0.0, 1.0)),
            key(Key::Escape, Modifiers::NONE),
        ],
        &mut chosen,
    );
    submenu_frame(&mut h, vec![], &mut chosen);
    assert!(h.ctx.find_widget("Recent").is_some(), "the menu stays open");

    // Choosing an item in the submenu closes everything.
    submenu_frame(&mut h, vec![move_to(recent)], &mut chosen);
    submenu_frame(&mut h, vec![], &mut chosen);
    submenu_frame(&mut h, vec![], &mut chosen);
    click_label(&mut h, "a.txt", &mut chosen);
    assert_eq!(chosen, Some("a"));
    assert!(h.ctx.find_widget("Recent").is_none());
    assert!(!h.ctx.any_popup_open());
}

/// Presses and releases the primary button at `at` in one frame.
fn click_events(at: Point) -> Vec<Event> {
    vec![button(at, true), button(at, false)]
}

#[test]
fn double_click_selects_a_word_and_triple_click_the_line() {
    let mut h = Harness::new();
    let mut s = String::from("uno due-tre");
    let (r, _) = edit_frame(&mut h, &mut s, false, vec![]);
    let body = h.ctx.style().body.clone();
    let x = h.fonts.layout("uno d", &body, None, 1.0).size.x;
    let at = point(r.rect.min.x + 8.0 + x, r.rect.center().y);

    edit_frame(&mut h, &mut s, false, click_events(at));
    let (r, _) = edit_frame(&mut h, &mut s, false, click_events(at));
    assert!(r.double_clicked());
    let (_, out) = edit_frame(&mut h, &mut s, false, vec![Event::Copy]);
    assert_eq!(
        out.copied_text.as_deref(),
        Some("due"),
        "stops at punctuation"
    );

    h.time += 1.0;
    edit_frame(&mut h, &mut s, false, click_events(at));
    edit_frame(&mut h, &mut s, false, click_events(at));
    let (r, _) = edit_frame(&mut h, &mut s, false, click_events(at));
    assert!(r.triple_clicked());
    let (_, out) = edit_frame(&mut h, &mut s, false, vec![Event::Copy]);
    assert_eq!(out.copied_text.as_deref(), Some("uno due-tre"));

    // Slow clicks are single clicks.
    h.time += 1.0;
    edit_frame(&mut h, &mut s, false, click_events(at));
    h.time += 1.0;
    let (r, _) = edit_frame(&mut h, &mut s, false, click_events(at));
    assert!(!r.double_clicked());
}

#[test]
fn password_fields_mask_the_text_and_refuse_to_copy() {
    let mut h = Harness::new();
    let mut s = String::new();
    let run = |h: &mut Harness, s: &mut String, events| {
        let mut response = None;
        let out = h.frame(events, |ui| {
            response = Some(
                ui.add(
                    crate::TextEdit::singleline(s)
                        .password(true)
                        .accessible_label("Key"),
                ),
            );
        });
        (response.unwrap(), out)
    };
    let (r, _) = run(&mut h, &mut s, vec![]);
    run(&mut h, &mut s, click_events(r.rect.center()));
    run(&mut h, &mut s, vec![text("pa€s")]);
    assert_eq!(s, "pa€s");
    // Arrows and Backspace work on the real characters.
    run(
        &mut h,
        &mut s,
        vec![press_key(Key::ArrowLeft), press_key(Key::Backspace)],
    );
    assert_eq!(s, "pas");
    let (_, out) = run(&mut h, &mut s, vec![command(Key::A), Event::Copy]);
    assert_eq!(out.copied_text, None);
    assert_eq!(
        h.ctx.find_widget("Key").unwrap().info.value.as_deref(),
        Some("•••")
    );
}

#[test]
fn hyperlink_asks_the_platform_to_open_its_url() {
    let mut h = Harness::new();
    let url = "https://crates.io/crates/rustroke";
    let link = |ui: &mut Ui<'_>| {
        ui.add(crate::Hyperlink::from_label_and_url("rustroke", url));
    };
    h.frame(vec![], link);
    let w = h.ctx.find_widget("rustroke").unwrap();
    assert_eq!(w.info.value.as_deref(), Some(url));
    let at = w.rect.center();
    let out = h.frame(vec![move_to(at)], link);
    assert_eq!(out.cursor, crate::CursorIcon::PointingHand);
    assert_eq!(out.open_url, None);
    let out = h.frame(click_events(at), link);
    assert_eq!(out.open_url.as_deref(), Some(url));
}

#[test]
fn windows_resize_in_height_and_clip_their_content() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut out = None;
        h.frame_with(events, |h| {
            out = Window::new("Long")
                .default_pos(point(20.0, 20.0))
                .show(h, |ui| {
                    crate::ScrollArea::vertical().show(ui, |ui| {
                        for i in 0..6 {
                            ui.label(format!("Line {i}"));
                        }
                    });
                })
                .map(|r| r.response.rect);
        });
        out.unwrap()
    };
    // The scroll area settles in a few frames.
    for _ in 0..3 {
        run(&mut h, vec![]);
    }
    let before = run(&mut h, vec![]);
    let grip = before.max - vec2(5.0, 5.0);
    let target = point(grip.x + 20.0, 150.0);
    run(&mut h, vec![button(grip, true)]);
    run(&mut h, vec![move_to(target)]);
    run(&mut h, vec![button(target, false)]);
    let after = run(&mut h, vec![]);
    assert!(
        (after.max.y - (150.0 + 5.0)).abs() < 1.0,
        "the bottom follows the grip: {after:?}"
    );
    assert!((after.width() - before.width() - 20.0).abs() < 1.0);
    // The height stays even though the content is taller.
    let again = run(&mut h, vec![]);
    assert_eq!(again, after);
}

#[test]
fn pointer_moves_over_empty_space_need_no_frame() {
    let mut h = Harness::new();
    let mut tracking = false;
    let run = |h: &mut Harness, events, tracking: bool| {
        let mut rects = (Rect::NOTHING, Rect::NOTHING);
        h.frame(events, |ui| {
            if tracking {
                ui.ctx().request_pointer_moves();
            }
            rects.0 = ui.button("Button").rect;
            rects.1 = ui.label("Tip").on_hover_text(ui, "tooltip").rect;
            ui.label("Plain text");
        });
        rects
    };
    let (button_rect, tip) = run(&mut h, vec![], tracking);
    let empty = point(350.0, 250.0);
    run(&mut h, vec![move_to(empty)], tracking);
    assert!(!h.ctx.pointer_move_needs_frame(empty + vec2(5.0, 0.0)));
    assert!(
        h.ctx.pointer_move_needs_frame(button_rect.center()),
        "hover starts"
    );
    assert!(
        h.ctx.pointer_move_needs_frame(tip.center()),
        "tooltip may start"
    );

    run(&mut h, vec![move_to(button_rect.center())], tracking);
    assert!(h.ctx.pointer_move_needs_frame(empty), "hover ends");

    tracking = true;
    run(&mut h, vec![move_to(empty)], tracking);
    assert!(
        h.ctx.pointer_move_needs_frame(empty + vec2(5.0, 0.0)),
        "app asked"
    );
}

#[test]
fn show_rows_only_adds_the_visible_rows() {
    let mut h = Harness::new();
    let mut added = Vec::new();
    let run = |h: &mut Harness, events, added: &mut Vec<usize>| {
        added.clear();
        h.frame(events, |ui| {
            crate::ScrollArea::vertical().max_height(100.0).show_rows(
                ui,
                20.0,
                1_000_000,
                |ui, rows| {
                    for i in rows {
                        added.push(i);
                        ui.add_sized(vec2(100.0, 20.0), crate::Label::new(format!("Row {i}")));
                    }
                },
            );
        });
    };
    run(&mut h, vec![], &mut added);
    run(&mut h, vec![], &mut added);
    assert!(added.len() < 10, "{added:?}");
    assert_eq!(added[0], 0);
    // Scroll far down: rows from there on, laid out where they belong.
    run(
        &mut h,
        vec![move_to(point(50.0, 50.0)), scroll(-280_000.0)],
        &mut added,
    );
    run(&mut h, vec![], &mut added);
    let first = added[0];
    assert!(
        (9_990..=10_000).contains(&first),
        "28 points per row: {first}"
    );
    let row = h.ctx.find_widget(&format!("Row {first}")).unwrap().rect;
    assert!(row.max.y > 0.0 && row.min.y < 100.0, "visible: {row:?}");
}

#[test]
fn scroll_to_rect_brings_a_row_into_view() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, target: Option<(usize, Option<crate::Align>)>| {
        let mut rect = Rect::NOTHING;
        let mut viewport = Rect::NOTHING;
        h.frame(vec![], |ui| {
            viewport = crate::ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for i in 0..50 {
                        let r = ui.add_sized(vec2(100.0, 20.0), crate::Label::new(format!("{i}")));
                        if let Some((t, align)) = target
                            && t == i
                        {
                            ui.scroll_to_rect(r.rect, align);
                            rect = r.rect;
                        }
                    }
                })
                .response
                .rect;
        });
        (rect, viewport)
    };
    run(&mut h, None);
    run(&mut h, None);
    run(&mut h, Some((30, None)));
    let (row, viewport) = run(&mut h, Some((30, None)));
    assert!(
        (row.max.y - viewport.max.y).abs() < 0.5,
        "scrolled just enough: {row:?} in {viewport:?}"
    );
    run(&mut h, Some((10, Some(crate::Align::Center))));
    let (row, viewport) = run(&mut h, Some((10, Some(crate::Align::Center))));
    assert!(
        (row.center().y - viewport.center().y).abs() < 0.5,
        "{row:?} {viewport:?}"
    );
}

#[test]
fn dragging_past_the_edge_scrolls_the_area() {
    let mut h = Harness::new();
    let run = |h: &mut Harness, events| {
        let mut first = Rect::NOTHING;
        h.frame(events, |ui| {
            crate::ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for i in 0..50 {
                        let r = ui.allocate_response(vec2(100.0, 20.0), crate::Sense::DRAG);
                        if i == 0 {
                            first = r.rect;
                        }
                    }
                });
        });
        first
    };
    run(&mut h, vec![]);
    let top = run(&mut h, vec![]);
    let inside = top.center();
    run(&mut h, vec![button(inside, true)]);
    // Below the area: it scrolls down a bit every frame.
    let below = point(inside.x, 140.0);
    let a = run(&mut h, vec![move_to(below)]);
    let b = run(&mut h, vec![]);
    let c = run(&mut h, vec![]);
    assert!(b.min.y < a.min.y && c.min.y < b.min.y, "{a:?} {b:?} {c:?}");
    // Released: it stops.
    run(&mut h, vec![button(below, false)]);
    let d = run(&mut h, vec![]);
    let e = run(&mut h, vec![]);
    assert_eq!(d, e);
}

#[test]
fn floating_scroll_bars_take_no_room() {
    let width = |floating: bool| {
        let mut h = Harness::new();
        let mut style = crate::Style::dark();
        style.spacing.floating_scrollbars = floating;
        h.ctx.set_style(style);
        let mut row = Rect::NOTHING;
        for _ in 0..3 {
            h.frame(vec![], |ui| {
                crate::ScrollArea::vertical()
                    .max_height(60.0)
                    .show(ui, |ui| {
                        for _ in 0..10 {
                            row = ui.add(crate::Separator).rect;
                        }
                    });
            });
        }
        row.width()
    };
    let (beside, floating) = (width(false), width(true));
    assert!(
        (floating - beside - 12.0).abs() < 0.5,
        "{beside} {floating}"
    );
}

/// A table of `rows` rows: "Name i" and a number; the first column sticky.
fn table_frame(
    h: &mut Harness,
    events: Vec<Event>,
    rows: usize,
    selection: &mut Option<usize>,
) -> crate::TableResponse {
    let mut out = None;
    h.frame(events, |ui| {
        out = Some(
            crate::Table::new("t")
                .column(crate::Column::new("Name").width(150.0).sortable(true))
                .column(
                    crate::Column::new("Qty")
                        .width(100.0)
                        .align(crate::Align::Max),
                )
                .column(crate::Column::new("Notes").width(300.0))
                .sticky_columns(1)
                .max_height(150.0)
                .show(ui, rows, selection, |ui, row, col| {
                    match col {
                        0 => ui.label(format!("Name {row}")),
                        1 => ui.label(format!("{}", row * 3)),
                        _ => ui.label("note"),
                    };
                }),
        );
    });
    out.unwrap()
}

#[test]
fn table_lays_out_only_visible_rows_and_sorts() {
    let mut h = Harness::new();
    let mut sel = None;
    table_frame(&mut h, vec![], 1_000_000, &mut sel);
    let t = table_frame(&mut h, vec![], 1_000_000, &mut sel);
    assert!(t.visible_rows.len() < 10, "{:?}", t.visible_rows);
    assert!(h.ctx.find_widget("Name 3").is_some());
    assert!(h.ctx.find_widget("Name 500").is_none());

    // Clicking a sortable header sorts ascending, then descending.
    let header = point(SCREEN.min.x + 40.0, 14.0);
    let t = table_frame(&mut h, click_events(header), 1_000_000, &mut sel);
    assert!(t.sort_changed);
    assert_eq!(
        t.sort,
        Some(crate::SortOrder {
            column: 0,
            ascending: true
        })
    );
    let t = table_frame(&mut h, click_events(header), 1_000_000, &mut sel);
    assert_eq!(t.sort.map(|s| s.ascending), Some(false));
    let t = table_frame(&mut h, vec![], 1_000_000, &mut sel);
    assert!(!t.sort_changed && t.sort.is_some(), "kept between frames");
}

#[test]
fn table_selection_follows_clicks_and_keys() {
    let mut h = Harness::new();
    let mut sel = None;
    table_frame(&mut h, vec![], 100, &mut sel);
    let row2 = h.ctx.find_widget("Name 2").unwrap().rect.center();
    let t = table_frame(&mut h, click_events(row2), 100, &mut sel);
    assert_eq!((t.clicked_row, sel), (Some(2), Some(2)));
    assert!(t.selection_changed);
    // Keys move it and keep it visible.
    for _ in 0..10 {
        table_frame(
            &mut h,
            vec![key(Key::ArrowDown, Modifiers::NONE)],
            100,
            &mut sel,
        );
    }
    assert_eq!(sel, Some(12));
    table_frame(&mut h, vec![], 100, &mut sel);
    assert!(
        h.ctx.find_widget("Name 12").is_some(),
        "scrolled to the selection"
    );
    table_frame(&mut h, vec![key(Key::End, Modifiers::NONE)], 100, &mut sel);
    let t = table_frame(&mut h, vec![], 100, &mut sel);
    assert_eq!(sel, Some(99));
    assert!(t.visible_rows.contains(&99));
}

#[test]
fn table_columns_resize_fit_and_stick() {
    let mut h = Harness::new();
    let mut sel = None;
    table_frame(&mut h, vec![], 20, &mut sel);
    table_frame(&mut h, vec![], 20, &mut sel);
    let name = h.ctx.find_widget("Name 0").unwrap().rect;
    // Drag the edge between "Name" and "Qty".
    let edge = point(SCREEN.min.x + 150.0, 14.0);
    table_frame(&mut h, vec![button(edge, true)], 20, &mut sel);
    table_frame(&mut h, vec![move_to(edge + vec2(30.0, 0.0))], 20, &mut sel);
    table_frame(
        &mut h,
        vec![button(edge + vec2(30.0, 0.0), false)],
        20,
        &mut sel,
    );
    table_frame(&mut h, vec![], 20, &mut sel);
    let qty = h.ctx.find_widget("3").unwrap().rect;
    assert!(
        (qty.max.x - (180.0 + 100.0 - 6.0)).abs() < 1.0,
        "right aligned in the moved column: {qty:?}"
    );

    // Double-click fits the column to its content.
    let edge = point(SCREEN.min.x + 180.0, 14.0);
    table_frame(&mut h, click_events(edge), 20, &mut sel);
    table_frame(&mut h, click_events(edge), 20, &mut sel);
    table_frame(&mut h, vec![], 20, &mut sel);
    let qty = h.ctx.find_widget("3").unwrap().rect;
    assert!(qty.max.x < 180.0 + 100.0 - 6.0 - 20.0, "narrower: {qty:?}");

    // Scrolling sideways moves the notes but not the sticky names.
    let inside = point(200.0, 80.0);
    table_frame(
        &mut h,
        vec![move_to(inside), Event::Scroll(vec2(-100.0, 0.0))],
        20,
        &mut sel,
    );
    table_frame(&mut h, vec![], 20, &mut sel);
    assert_eq!(h.ctx.find_widget("Name 0").unwrap().rect, name);
}

/// Roots 0, 1, 2; node n < 3 has children 100n..100n+big (leaves).
fn tree_frame(
    h: &mut Harness,
    events: Vec<Event>,
    big: u32,
    selection: &mut Option<u32>,
) -> crate::TreeResponse<u32> {
    let mut out = None;
    h.frame(events, |ui| {
        out = Some(crate::Tree::new("tree").max_height(200.0).show(
            ui,
            &[0, 1, 2],
            |n| {
                if n < 3 {
                    (1000 + n * big..1000 + (n + 1) * big).collect()
                } else {
                    Vec::new()
                }
            },
            selection,
            |ui, n| {
                ui.label(format!("Node {n}"));
            },
        ));
    });
    out.unwrap()
}

#[test]
fn tree_expands_with_the_arrow_and_the_keys() {
    let mut h = Harness::new();
    let mut sel = None;
    tree_frame(&mut h, vec![], 3, &mut sel);
    let t = tree_frame(&mut h, vec![], 3, &mut sel);
    assert_eq!(t.rows.len(), 3, "closed by default");
    // The arrow in front of "Node 1" opens it.
    let row1 = t.rows[1].1.rect;
    let arrow = point(row1.min.x + 6.0, row1.center().y);
    tree_frame(&mut h, click_events(arrow), 3, &mut sel);
    tree_frame(&mut h, vec![], 3, &mut sel);
    let t = tree_frame(&mut h, vec![], 3, &mut sel);
    let nodes: Vec<u32> = t.rows.iter().map(|r| r.0).collect();
    assert_eq!(nodes, [0, 1, 1003, 1004, 1005, 2]);
    assert!(sel.is_none(), "the arrow doesn't select");
    let child = h.ctx.find_widget("Node 1003").unwrap().rect;
    assert!(
        child.min.x > h.ctx.find_widget("Node 1").unwrap().rect.min.x,
        "indented"
    );

    // Select node 2, then → opens it and ← closes it, ← again goes up.
    let node2 = h.ctx.find_widget("Node 2").unwrap().rect.center();
    tree_frame(&mut h, click_events(node2), 3, &mut sel);
    assert_eq!(sel, Some(2));
    tree_frame(
        &mut h,
        vec![key(Key::ArrowRight, Modifiers::NONE)],
        3,
        &mut sel,
    );
    let t = tree_frame(
        &mut h,
        vec![key(Key::ArrowRight, Modifiers::NONE)],
        3,
        &mut sel,
    );
    assert_eq!(sel, Some(1006), "→ on an open node goes to its first child");
    assert_eq!(t.rows.len(), 9);
    tree_frame(
        &mut h,
        vec![key(Key::ArrowLeft, Modifiers::NONE)],
        3,
        &mut sel,
    );
    assert_eq!(sel, Some(2), "← on a leaf goes to the parent");
    tree_frame(
        &mut h,
        vec![key(Key::ArrowLeft, Modifiers::NONE)],
        3,
        &mut sel,
    );
    let t = tree_frame(&mut h, vec![], 3, &mut sel);
    assert_eq!(t.rows.len(), 6, "← closes it");
}

#[test]
fn tree_lays_out_only_visible_rows() {
    let mut h = Harness::new();
    let mut sel = None;
    tree_frame(&mut h, vec![], 100_000, &mut sel);
    let t = tree_frame(&mut h, vec![], 100_000, &mut sel);
    let row0 = t.rows[0].1.rect;
    let arrow = point(row0.min.x + 6.0, row0.center().y);
    tree_frame(&mut h, click_events(arrow), 100_000, &mut sel);
    let t = tree_frame(&mut h, vec![], 100_000, &mut sel);
    assert!(t.rows.len() < 15, "{}", t.rows.len());
    // End selects the last node and scrolls to it.
    let node0 = h.ctx.find_widget("Node 0").unwrap().rect.center();
    tree_frame(&mut h, click_events(node0), 100_000, &mut sel);
    tree_frame(
        &mut h,
        vec![key(Key::End, Modifiers::NONE)],
        100_000,
        &mut sel,
    );
    assert_eq!(sel, Some(2));
    tree_frame(&mut h, vec![], 100_000, &mut sel);
    let t = tree_frame(&mut h, vec![], 100_000, &mut sel);
    let nodes: Vec<u32> = t.rows.iter().map(|r| r.0).collect();
    assert!(
        h.ctx.find_widget("Node 2").is_some(),
        "scrolled to the end: {nodes:?}"
    );
}
