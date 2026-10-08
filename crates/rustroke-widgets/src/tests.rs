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
