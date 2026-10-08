//! Phase 2 demo: text. Sizes and weights, measuring text to center it,
//! a paragraph that re-wraps as the window is resized, the monospace font,
//! and scripts/emoji rendered through system font fallback.
//!
//! Run with: `cargo run -p rustroke --example text`

use rustroke::{App, Color, Frame, Rect, Stroke, TextStyle, WindowOptions, point, vec2};

const LOREM: &str = "Rustroke is an immediate-mode GUI library written in Rust. \
    Text is laid out with cosmic-text, rasterized once into the texture atlas \
    and drawn as pixel-aligned quads. Resize the window: this paragraph \
    wraps accordingly. Accents work too: perché, déjà vu, naïve, Ålesund.";

struct TextDemo;

impl App for TextDemo {
    fn update(&mut self, frame: &mut Frame) {
        let bg = Color::from_srgb8(30, 30, 46);
        let surface = Color::from_srgb8(49, 50, 68);
        let text = Color::from_srgb8(205, 214, 244);
        let subtle = Color::from_srgb8(166, 173, 200);
        let blue = Color::from_srgb8(137, 180, 250);
        let green = Color::from_srgb8(166, 227, 161);
        frame.clear_color = bg;

        let margin = 24.0;
        let width = frame.screen_rect.width() - 2.0 * margin;
        let mut y = margin;

        // Headings in decreasing sizes.
        y += frame
            .text(
                point(margin, y),
                "Text",
                &TextStyle::proportional(32.0).bold(),
                text,
            )
            .height();
        y += frame
            .text(
                point(margin, y),
                "Shaping, fallback, wrapping and measuring",
                &TextStyle::proportional(16.0),
                subtle,
            )
            .height()
            + 16.0;

        // Wrapped paragraph inside a card that follows the window width.
        let style = TextStyle::proportional(14.0);
        let paragraph = frame.layout_text(LOREM, &style, Some(width - 32.0));
        let card = Rect::from_min_size(point(margin, y), vec2(width, paragraph.size.y + 32.0));
        frame
            .shapes
            .rect(card, 10.0, surface, Stroke::new(1.0, text.with_alpha(0.08)));
        frame
            .shapes
            .galley(card.min + vec2(16.0, 16.0), paragraph, text);
        y = card.max.y + 16.0;

        // Buttons whose size comes from measuring their label.
        let mut x = margin;
        for label in ["OK", "Cancel", "Save as…"] {
            let galley = frame.layout_text(label, &TextStyle::proportional(14.0), None);
            let button = Rect::from_min_size(point(x, y), galley.size + vec2(28.0, 14.0));
            frame.shapes.rect_filled(button, 8.0, blue);
            frame
                .shapes
                .galley(button.center() - galley.size / 2.0, galley, bg);
            x = button.max.x + 10.0;
        }
        y += 50.0;

        // Monospace (system font).
        y += frame
            .text(
                point(margin, y),
                "fn main() { println!(\"monospace 0O 1lI\"); }",
                &TextStyle::monospace(14.0),
                green,
            )
            .height()
            + 12.0;

        // Other scripts and emoji come from system fallback fonts.
        for line in [
            "Ελληνικά · Русский · Español · Português",
            "日本語のテキスト · 中文文本 · 한국어 텍스트",
            "العربية من اليمين إلى اليسار · עברית",
            "Emoji: 🎨 🦀 🚀 ✅ 👍🏽",
        ] {
            y += frame
                .text(point(margin, y), line, &TextStyle::proportional(18.0), text)
                .height()
                + 4.0;
        }
    }
}

fn main() -> Result<(), rustroke::RunError> {
    env_logger::init();
    rustroke::run(
        WindowOptions {
            title: "text".to_owned(),
            inner_size: (720.0, 600.0),
        },
        TextDemo,
    )
}
