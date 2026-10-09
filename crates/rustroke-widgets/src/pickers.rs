//! Color, date and time pickers.

use rustroke_core::{Color, Gradient, Rect, Stroke, Vec2, point, vec2};

use crate::{
    CursorIcon, DragValue, Grid, Id, Response, SelectableLabel, Sense, TextEdit, Ui, Widget,
    WidgetInfo, WidgetRole,
};

// ---------------------------------------------------------------------------
// Color

/// A button showing a color; clicking it opens a picker: a
/// saturation/value square, a hue bar, an opacity bar (with
/// [`ColorPicker::alpha`]) and a hex field (`#RRGGBB` or `#RRGGBBAA`).
/// The response is `changed()` while the color changes.
#[derive(Debug)]
pub struct ColorPicker<'a> {
    color: &'a mut Color,
    alpha: bool,
}

impl<'a> ColorPicker<'a> {
    /// Edits `color` (opaque).
    pub fn new(color: &'a mut Color) -> Self {
        Self {
            color,
            alpha: false,
        }
    }

    /// Also edit the opacity.
    pub fn alpha(mut self, alpha: bool) -> Self {
        self.alpha = alpha;
        self
    }
}

/// Draws a checkerboard (for transparent colors) in `rect`.
fn checkerboard(ui: &mut Ui<'_>, rect: Rect) {
    let dark = ui.style().visuals.dark_mode;
    let (a, b) = if dark {
        (
            Color::from_srgb8(90, 90, 90),
            Color::from_srgb8(130, 130, 130),
        )
    } else {
        (Color::from_srgb8(200, 200, 200), Color::WHITE)
    };
    ui.painter().rect_filled(rect, 0.0, a);
    let cell = 6.0;
    let saved = ui.clip_rect();
    ui.set_clip_rect(rect);
    let (cols, rows) = (
        (rect.width() / cell).ceil() as i32,
        (rect.height() / cell).ceil() as i32,
    );
    for y in 0..rows {
        for x in 0..cols {
            if (x + y) % 2 == 0 {
                let min = rect.min + vec2(x as f32 * cell, y as f32 * cell);
                ui.painter()
                    .rect_filled(Rect::from_min_size(min, Vec2::splat(cell)), 0.0, b);
            }
        }
    }
    ui.clip_rect_restore(saved);
}

impl Widget for ColorPicker<'_> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let size = vec2(
            style.spacing.interact_height * 1.6,
            style.spacing.interact_height,
        );
        let mut response = ui.allocate_response(size, Sense::CLICK);
        let id = response.id;
        ui.describe(
            &response,
            WidgetInfo::new(WidgetRole::Button, "Color").value(hex(*self.color, self.alpha)),
        );
        let rect = response.rect.expand(-2.0);
        let radius = style.visuals.small_corner_radius;
        if self.color.a < 1.0 {
            checkerboard(ui, rect);
        }
        let visuals = ui.widget_visuals(&response);
        ui.painter().rect(rect, radius, *self.color, visuals.stroke);
        if response.clicked() {
            ui.ctx().toggle_popup(id);
        }
        let alpha = self.alpha;
        let color = self.color;
        let changed = ui
            .popup_below(id, response.rect, |ui| color_edit(ui, id, color, alpha))
            .unwrap_or(false);
        if changed {
            response.mark_changed();
        }
        response
    }
}

/// The color as `#RRGGBB` (or `#RRGGBBAA` with alpha).
fn hex(color: Color, alpha: bool) -> String {
    let [r, g, b, a] = color.to_srgba8();
    if alpha {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}")
    }
}

/// Parses `#RGB`, `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
pub(crate) fn parse_hex(text: &str) -> Option<Color> {
    let t = text.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(t.get(i..i + 2)?, 16).ok();
    match t.len() {
        3 => {
            let d = |i: usize| u8::from_str_radix(t.get(i..=i)?, 16).ok().map(|v| v * 17);
            Some(Color::from_srgb8(d(0)?, d(1)?, d(2)?))
        }
        6 => Some(Color::from_srgb8(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(Color::from_srgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

/// The picker's content; returns whether the color changed.
fn color_edit(ui: &mut Ui<'_>, id: Id, color: &mut Color, alpha: bool) -> bool {
    // Keep the hue while saturation or value is zero (it is lost in RGB).
    let hsva_key = id.with("hsva");
    let mut hsva: [f32; 4] = match ui.ctx().data::<[f32; 4]>(hsva_key) {
        Some(h) if Color::from_hsva(h[0], h[1], h[2], h[3]).to_srgba8() == color.to_srgba8() => h,
        _ => color.to_hsva(),
    };
    let before = hsva;
    let side = 160.0;
    let bar = 18.0;
    let visuals = ui.style().visuals.clone();

    ui.horizontal(|ui| {
        // Saturation (x) and value (y).
        let sv = ui.allocate_response(Vec2::splat(side), Sense::DRAG);
        let r = sv.rect;
        let hue = Color::from_hsva(hsva[0], 1.0, 1.0, 1.0);
        let mut across = Gradient::linear(r.left_top(), r.right_top(), Color::WHITE, hue);
        for k in 1..4 {
            let s = k as f32 / 4.0;
            across = across.with_stop(s, Color::from_hsva(hsva[0], s, 1.0, 1.0));
        }
        ui.painter().rect_gradient(r, 0.0, across, Stroke::NONE);
        let mut down = Gradient::linear(
            r.left_top(),
            r.left_bottom(),
            Color::TRANSPARENT,
            Color::BLACK,
        );
        for k in 1..6 {
            let t = k as f32 / 6.0;
            // Value scales sRGB, so darkness grows faster in linear light.
            let v = 1.0 - t;
            down = down.with_stop(t, Color::BLACK.with_alpha(1.0 - v.powf(2.2)));
        }
        ui.painter().rect_gradient(r, 0.0, down, Stroke::NONE);
        if sv.is_pressed() || sv.dragged() {
            if let Some(p) = sv.interact_pointer_pos() {
                hsva[1] = ((p.x - r.min.x) / r.width()).clamp(0.0, 1.0);
                hsva[2] = 1.0 - ((p.y - r.min.y) / r.height()).clamp(0.0, 1.0);
            }
            ui.ctx().set_cursor(CursorIcon::Crosshair);
        }
        let marker = point(r.min.x + hsva[1] * side, r.min.y + (1.0 - hsva[2]) * side);
        let ring = if hsva[2] > 0.5 {
            Color::BLACK
        } else {
            Color::WHITE
        };
        ui.painter()
            .circle(marker, 5.0, Color::TRANSPARENT, Stroke::new(1.5, ring));

        // Hue.
        let hue_bar = ui.allocate_response(vec2(bar, side), Sense::DRAG);
        let hr = hue_bar.rect;
        let mut hues = Gradient::linear(
            hr.left_top(),
            hr.left_bottom(),
            Color::from_hsva(0.0, 1.0, 1.0, 1.0),
            Color::from_hsva(1.0, 1.0, 1.0, 1.0),
        );
        for k in 1..12 {
            let h = k as f32 / 12.0;
            hues = hues.with_stop(h, Color::from_hsva(h, 1.0, 1.0, 1.0));
        }
        ui.painter().rect_gradient(hr, 3.0, hues, Stroke::NONE);
        if (hue_bar.is_pressed() || hue_bar.dragged())
            && let Some(p) = hue_bar.interact_pointer_pos()
        {
            hsva[0] = ((p.y - hr.min.y) / hr.height()).clamp(0.0, 0.9999);
        }
        let y = hr.min.y + hsva[0] * side;
        ui.painter().rect_stroke(
            Rect::from_min_max(
                point(hr.min.x - 1.0, y - 2.0),
                point(hr.max.x + 1.0, y + 2.0),
            ),
            2.0,
            Stroke::new(1.5, visuals.text),
        );

        // Opacity.
        if alpha {
            let alpha_bar = ui.allocate_response(vec2(bar, side), Sense::DRAG);
            let ar = alpha_bar.rect;
            checkerboard(ui, ar);
            let opaque = Color::from_hsva(hsva[0], hsva[1], hsva[2], 1.0);
            ui.painter().rect_gradient(
                ar,
                0.0,
                Gradient::linear(
                    ar.left_top(),
                    ar.left_bottom(),
                    opaque,
                    opaque.with_alpha(0.0),
                ),
                Stroke::new(1.0, visuals.window_stroke.color),
            );
            if (alpha_bar.is_pressed() || alpha_bar.dragged())
                && let Some(p) = alpha_bar.interact_pointer_pos()
            {
                hsva[3] = 1.0 - ((p.y - ar.min.y) / ar.height()).clamp(0.0, 1.0);
            }
            let y = ar.min.y + (1.0 - hsva[3]) * side;
            ui.painter().rect_stroke(
                Rect::from_min_max(
                    point(ar.min.x - 1.0, y - 2.0),
                    point(ar.max.x + 1.0, y + 2.0),
                ),
                2.0,
                Stroke::new(1.5, visuals.text),
            );
        }
    });

    // Hex, edited as text; applied when valid.
    let text_key = id.with("hex");
    let current = hex(Color::from_hsva(hsva[0], hsva[1], hsva[2], hsva[3]), alpha);
    let mut text: String = ui.ctx().data(text_key).unwrap_or_else(|| current.clone());
    let field = ui.add(
        TextEdit::singleline(&mut text)
            .id(id.with("hex field"))
            .desired_width(side)
            .accessible_label("Hex color"),
    );
    if field.changed()
        && let Some(c) = parse_hex(&text)
    {
        let mut h = c.to_hsva();
        if !alpha {
            h[3] = hsva[3];
        }
        hsva = h;
    }
    if field.has_focus() {
        ui.ctx().insert_data(text_key, text);
    } else {
        ui.ctx().remove_data(text_key);
    }

    if !alpha {
        hsva[3] = color.a;
    }
    let changed = hsva != before;
    if changed {
        *color = Color::from_hsva(hsva[0], hsva[1], hsva[2], hsva[3]);
    }
    ui.ctx().insert_data(hsva_key, hsva);
    changed
}

// ---------------------------------------------------------------------------
// Date

/// A calendar date (proleptic Gregorian), without time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    /// Year, e.g. 2026.
    pub year: i32,
    /// Month, 1 (January) to 12.
    pub month: u32,
    /// Day of the month, from 1.
    pub day: u32,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl Date {
    /// The date, if it exists.
    pub fn new(year: i32, month: u32, day: u32) -> Option<Self> {
        ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month))
            .then_some(Self { year, month, day })
    }

    /// Today in UTC, from the system clock.
    pub fn today_utc() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self::from_days_since_epoch((secs / 86_400) as i64)
    }

    /// Days since 1970-01-01.
    pub fn days_since_epoch(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date `days` after 1970-01-01.
    pub fn from_days_since_epoch(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Self { year, month, day }
    }

    /// Day of the week: 0 = Monday … 6 = Sunday.
    pub fn weekday(self) -> u32 {
        // 1970-01-01 was a Thursday.
        (self.days_since_epoch() + 3).rem_euclid(7) as u32
    }

    /// The same day `months` later (or earlier), clamped to the month's
    /// length.
    pub fn add_months(self, months: i32) -> Self {
        let index = self.year * 12 + self.month as i32 - 1 + months;
        let (year, month) = (index.div_euclid(12), index.rem_euclid(12) as u32 + 1);
        Self {
            year,
            month,
            day: self.day.min(days_in_month(year, month)),
        }
    }
}

impl std::fmt::Display for Date {
    /// ISO 8601: `2026-10-09`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Days in `month` (1..12) of `year`.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A frameless button that keeps the popup it is in open.
fn nav_button(text: &str, label: &str) -> crate::Button {
    let mut button = crate::Button::new(text)
        .frame(false)
        .accessible_label(label);
    button.opens_submenu = true;
    button
}

/// A box showing a date; clicking it opens a calendar to pick another.
/// The response is `changed()` when a day is picked.
#[derive(Debug)]
pub struct DatePicker<'a> {
    date: &'a mut Date,
    sunday_first: bool,
}

impl<'a> DatePicker<'a> {
    /// Edits `date`. Weeks start on Monday.
    pub fn new(date: &'a mut Date) -> Self {
        Self {
            date,
            sunday_first: false,
        }
    }

    /// Weeks start on Sunday.
    pub fn sunday_first(mut self, sunday_first: bool) -> Self {
        self.sunday_first = sunday_first;
        self
    }
}

impl Widget for DatePicker<'_> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let text = self.date.to_string();
        let mut response =
            ui.add(crate::Button::new(format!("{text}  ▾")).accessible_label("Date"));
        let id = response.id;
        if response.clicked() {
            ui.ctx().toggle_popup(id);
        }
        let shown_key = id.with("shown month");
        let date = self.date;
        let sunday_first = self.sunday_first;
        let picked = ui
            .popup_below(id, response.rect, |ui| {
                // A calendar, not a menu: items keep their size and don't
                // close it (picking a day closes it below).
                ui.set_in_menu(None);
                // The month on screen (navigated with ‹ ›).
                let mut shown: Date = ui.ctx().data(shown_key).unwrap_or(*date);
                let mut picked = None;
                ui.horizontal(|ui| {
                    if ui.add(nav_button("‹", "Previous month")).clicked() {
                        shown = shown.add_months(-1);
                    }
                    ui.add_sized(
                        vec2(150.0, ui.style().spacing.interact_height),
                        crate::Label::new(format!(
                            "{} {}",
                            MONTHS[shown.month as usize - 1],
                            shown.year
                        )),
                    );
                    if ui.add(nav_button("›", "Next month")).clicked() {
                        shown = shown.add_months(1);
                    }
                });
                let names = if sunday_first {
                    ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
                } else {
                    ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
                };
                let first = Date { day: 1, ..shown };
                let offset = (first.weekday() + u32::from(sunday_first)) % 7;
                let days = days_in_month(shown.year, shown.month);
                let today = Date::today_utc();
                Grid::new(id.with("days")).show(ui, |ui| {
                    for n in names {
                        ui.add(crate::Label::new(n).color(ui.style().visuals.weak_text));
                    }
                    ui.end_row();
                    for cell in 0..offset + days {
                        if cell >= offset {
                            let day = cell - offset + 1;
                            let this = Date { day, ..shown };
                            let mut label =
                                SelectableLabel::new(this == *date, format!("{day:>2}"));
                            if this == today {
                                label = label.accessible_label(format!("{day} (today)"));
                            }
                            if ui.add(label).clicked() {
                                picked = Some(this);
                            }
                        } else {
                            ui.label("");
                        }
                        if cell % 7 == 6 {
                            ui.end_row();
                        }
                    }
                });
                ui.ctx().insert_data(shown_key, shown);
                picked
            })
            .flatten();
        if let Some(day) = picked {
            *date = day;
            ui.ctx().remove_data(shown_key);
            ui.ctx().close_popup();
            response.mark_changed();
        }
        response
    }
}

/// Hours and minutes, each edited by dragging or typing (`HH : MM`).
/// Minutes wrap into hours.
#[derive(Debug)]
pub struct TimePicker<'a> {
    hour: &'a mut u32,
    minute: &'a mut u32,
}

impl<'a> TimePicker<'a> {
    /// Edits `hour` (0..23) and `minute` (0..59).
    pub fn new(hour: &'a mut u32, minute: &'a mut u32) -> Self {
        Self { hour, minute }
    }
}

impl Widget for TimePicker<'_> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let before = (*self.hour, *self.minute);
        let mut minute = i64::from(*self.minute);
        let mut hour = i64::from(*self.hour);
        let inner = ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 4.0;
            ui.add(DragValue::new(&mut hour).range(0..=23).speed(0.1));
            ui.label(":");
            ui.add(DragValue::new(&mut minute).range(-1..=60).speed(0.2));
        });
        // Wrap minutes into hours.
        if minute >= 60 {
            minute = 0;
            hour = (hour + 1) % 24;
        } else if minute < 0 {
            minute = 59;
            hour = (hour + 23) % 24;
        }
        *self.hour = hour.clamp(0, 23) as u32;
        *self.minute = minute.clamp(0, 59) as u32;
        let mut response = inner.response;
        if (*self.hour, *self.minute) != before {
            response.mark_changed();
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_convert_to_and_from_days() {
        let d = Date::new(2026, 10, 9).unwrap();
        assert_eq!(Date::from_days_since_epoch(d.days_since_epoch()), d);
        assert_eq!(Date::new(1970, 1, 1).unwrap().days_since_epoch(), 0);
        assert_eq!(d.weekday(), 4, "a Friday");
        assert_eq!(
            Date::new(2024, 1, 31).unwrap().add_months(1),
            Date::new(2024, 2, 29).unwrap()
        );
        assert_eq!(
            Date::new(2026, 1, 15).unwrap().add_months(-2).to_string(),
            "2025-11-15"
        );
        assert!(Date::new(2025, 2, 29).is_none());
    }

    #[test]
    fn hex_colors_parse() {
        assert_eq!(parse_hex("#FF0000").unwrap().to_srgba8(), [255, 0, 0, 255]);
        assert_eq!(parse_hex("0f0").unwrap().to_srgba8(), [0, 255, 0, 255]);
        assert_eq!(parse_hex("#00000080").unwrap().to_srgba8(), [0, 0, 0, 128]);
        assert!(parse_hex("#12345").is_none());
        assert_eq!(hex(Color::from_srgb8(1, 2, 255), false), "#0102FF");
    }
}
