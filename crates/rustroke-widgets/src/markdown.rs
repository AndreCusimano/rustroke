//! Markdown rendering (feature `markdown`), built on pulldown-cmark.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use rustroke_core::{Rect, point, vec2};
use rustroke_text::{LayoutJob, TextFormat, TextStyle};

use crate::{Align, Grid, Image, Label, Layout, Response, Sense, TextureHandle, Ui};

/// Finds the image for a URL in Markdown (`![alt](url)`), e.g. among
/// images the app has loaded. `None` shows the alt text instead.
pub type ImageLoader<'a> = &'a dyn Fn(&str) -> Option<TextureHandle>;

/// Shows Markdown text (CommonMark plus tables, strike-through and task
/// lists): headings, paragraphs with **bold**, *italic*, ~~struck~~ and
/// `code` text, links (opened in the browser), bulleted and numbered
/// lists, block quotes, code blocks, tables, rules and images.
///
/// The text is parsed on every frame, which is fast for documents of a
/// few pages; keep longer ones in a [`crate::ScrollArea`].
///
/// ```ignore
/// ui.markdown("# Title\n\nSome **bold** text and a [link](https://example.com).");
/// ```
pub struct Markdown<'a> {
    source: &'a str,
    images: Option<ImageLoader<'a>>,
}

impl std::fmt::Debug for Markdown<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Markdown")
            .field("len", &self.source.len())
            .finish_non_exhaustive()
    }
}

/// One table being collected: rows of cells, the first `head` rows are
/// the header.
#[derive(Default)]
struct TableState {
    rows: Vec<Vec<LayoutJob>>,
    head: usize,
    in_head: bool,
}

/// Inline formatting in effect while walking the events.
#[derive(Default)]
struct Inline {
    bold: usize,
    italic: usize,
    strike: usize,
    link: Option<String>,
    heading: Option<HeadingLevel>,
}

impl<'a> Markdown<'a> {
    /// Markdown `source` to show.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            images: None,
        }
    }

    /// How to find images by URL; without it images show their alt text.
    pub fn image_loader(mut self, loader: ImageLoader<'a>) -> Self {
        self.images = Some(loader);
        self
    }

    /// Shows the text. The response covers all of it.
    pub fn show(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let body = style.body.clone();
        let visuals = style.visuals.clone();
        let indent = style.spacing.indent;
        let mono = TextStyle::monospace(body.size * 0.92);
        let code_bg = visuals.weak_text.with_alpha(0.15);
        let start = ui.available_rect().min;
        let mut used = Rect::NOTHING;

        let options =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        let mut inline = Inline::default();
        let mut job = LayoutJob::default();
        let mut lists: Vec<Option<u64>> = Vec::new();
        let mut quote_depth = 0usize;
        let mut code_block: Option<String> = None;
        let mut table: Option<TableState> = None;
        let mut image: Option<(String, String)> = None;
        let mut tables = 0usize;

        let format = |inline: &Inline| {
            let base = match inline.heading {
                Some(level) => heading_style(&body, level),
                None => body.clone(),
            };
            let style = if inline.bold > 0 {
                base.weight(700)
            } else {
                base
            };
            TextFormat {
                style: Some(style),
                italic: inline.italic > 0,
                strikethrough: inline.strike > 0,
                link: inline.link.clone(),
                ..TextFormat::default()
            }
        };

        // Adds the paragraph collected so far as a label, indented for the
        // lists and quotes it is in.
        let flush = |ui: &mut Ui<'_>,
                     job: &mut LayoutJob,
                     depth: usize,
                     quote_depth: usize,
                     used: &mut Rect| {
            if job.text.is_empty() {
                return;
            }
            let job = std::mem::take(job);
            let shift = depth as f32 * indent;
            let avail = ui.available_rect();
            let rect = Rect::from_min_max(point(avail.min.x + shift, avail.min.y), avail.max);
            let r = ui
                .scope_with(rect, Layout::top_down(Align::Min), |ui| {
                    ui.add(Label::rich(job))
                })
                .inner
                .rect;
            for q in 0..quote_depth {
                let x = avail.min.x + q as f32 * indent + 3.0;
                ui.painter().rect_filled(
                    Rect::from_min_max(point(x, r.min.y), point(x + 3.0, r.max.y)),
                    1.5,
                    visuals.weak_text.with_alpha(0.5),
                );
            }
            *used = used.union(r);
        };

        for event in Parser::new_ext(self.source, options) {
            let depth = lists.len() + quote_depth;
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    ui.add_space(body.size * 0.4);
                    inline.heading = Some(level);
                }
                Event::End(TagEnd::Heading(_)) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    inline.heading = None;
                }
                Event::Start(Tag::Paragraph) => {}
                Event::End(TagEnd::Paragraph) => flush(ui, &mut job, depth, quote_depth, &mut used),
                Event::Start(Tag::BlockQuote(_)) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    quote_depth += 1;
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    quote_depth -= 1;
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    code_block = Some(String::new());
                }
                Event::End(TagEnd::CodeBlock) => {
                    let code = code_block.take().unwrap_or_default();
                    let code = code.trim_end_matches('\n');
                    let galley = ui.layout_job(
                        &LayoutJob::simple(code, TextFormat::new().style(mono.clone())),
                        &body,
                        None,
                    );
                    let pad = 8.0;
                    let avail = ui.available_rect();
                    let shift = depth as f32 * indent;
                    let width = (avail.width() - shift).max(0.0);
                    let rect = ui.allocate_rect(vec2(avail.width(), galley.size.y + 2.0 * pad));
                    let block = Rect::from_min_size(
                        point(rect.min.x + shift, rect.min.y),
                        vec2(width, rect.height()),
                    );
                    ui.painter()
                        .rect_filled(block, visuals.small_corner_radius, code_bg);
                    let saved = ui.clip_rect();
                    ui.set_clip_rect(block.expand(-2.0));
                    ui.painter()
                        .galley(block.min + vec2(pad, pad), galley, visuals.text);
                    ui.clip_rect_restore(saved);
                    used = used.union(block);
                }
                Event::Start(Tag::List(first)) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    lists.push(first);
                }
                Event::End(TagEnd::List(_)) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    lists.pop();
                }
                Event::Start(Tag::Item) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    let marker = match lists.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}. ");
                            *n += 1;
                            m
                        }
                        _ => "•  ".to_owned(),
                    };
                    job.append(&marker, TextFormat::new());
                }
                Event::End(TagEnd::Item) => flush(ui, &mut job, depth, quote_depth, &mut used),
                Event::TaskListMarker(done) => {
                    // Replace the bullet with a box.
                    job = LayoutJob::default();
                    job.append(if done { "☑  " } else { "☐  " }, TextFormat::new());
                }
                Event::Start(Tag::Emphasis) => inline.italic += 1,
                Event::End(TagEnd::Emphasis) => inline.italic -= 1,
                Event::Start(Tag::Strong) => inline.bold += 1,
                Event::End(TagEnd::Strong) => inline.bold -= 1,
                Event::Start(Tag::Strikethrough) => inline.strike += 1,
                Event::End(TagEnd::Strikethrough) => inline.strike -= 1,
                Event::Start(Tag::Link { dest_url, .. }) => {
                    inline.link = Some(dest_url.to_string())
                }
                Event::End(TagEnd::Link) => inline.link = None,
                Event::Start(Tag::Image { dest_url, .. }) => {
                    image = Some((dest_url.to_string(), String::new()));
                }
                Event::End(TagEnd::Image) => {
                    let (url, alt) = image.take().unwrap_or_default();
                    match self.images.and_then(|load| load(&url)) {
                        Some(texture) => {
                            flush(ui, &mut job, depth, quote_depth, &mut used);
                            let width = ui.available_width();
                            let r = ui.add(Image::new(&texture).max_width(width).alt_text(alt));
                            used = used.union(r.rect);
                        }
                        None => {
                            job.append(&alt, TextFormat::new().italic().color(visuals.weak_text))
                        }
                    }
                }
                Event::Start(Tag::Table(_)) => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    table = Some(TableState::default());
                }
                Event::Start(Tag::TableHead) => {
                    if let Some(t) = &mut table {
                        t.in_head = true;
                        t.rows.push(Vec::new());
                    }
                }
                Event::End(TagEnd::TableHead) => {
                    if let Some(t) = &mut table {
                        t.in_head = false;
                        t.head = t.rows.len();
                    }
                }
                Event::Start(Tag::TableRow) => {
                    if let Some(t) = &mut table {
                        t.rows.push(Vec::new());
                    }
                }
                Event::Start(Tag::TableCell) => {
                    if let Some(row) = table.as_mut().and_then(|t| t.rows.last_mut()) {
                        row.push(LayoutJob::default());
                    }
                }
                Event::End(TagEnd::Table) => {
                    let Some(t) = table.take() else { continue };
                    tables += 1;
                    let r = ui.push_id(("markdown table", tables), |ui| {
                        let avail = ui.available_rect();
                        let shift = depth as f32 * indent;
                        let rect =
                            Rect::from_min_max(point(avail.min.x + shift, avail.min.y), avail.max);
                        ui.scope_with(rect, Layout::top_down(Align::Min), |ui| {
                            Grid::new("table").striped(true).show(ui, |ui| {
                                for row in t.rows {
                                    for cell in row {
                                        ui.add(Label::rich(cell));
                                    }
                                    ui.end_row();
                                }
                            });
                        })
                        .response
                        .rect
                    });
                    used = used.union(r);
                }
                Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                    if let Some(code) = &mut code_block {
                        code.push_str(&text);
                    } else if let Some((_, alt)) = &mut image {
                        alt.push_str(&text);
                    } else if let Some(t) = &mut table {
                        let mut f = format(&inline);
                        if t.in_head {
                            f.style = f.style.map(|s| s.weight(700));
                        }
                        if let Some(cell) = t.rows.last_mut().and_then(|r| r.last_mut()) {
                            cell.append(&text, f);
                        }
                    } else {
                        job.append(&text, format(&inline));
                    }
                }
                Event::Code(text) => {
                    let f = TextFormat::new().style(mono.clone()).background(code_bg);
                    match table
                        .as_mut()
                        .and_then(|t| t.rows.last_mut())
                        .and_then(|r| r.last_mut())
                    {
                        Some(cell) => cell.append(&text, f),
                        None => job.append(&text, f),
                    }
                }
                Event::SoftBreak => job.append(" ", format(&inline)),
                Event::HardBreak => job.append("\n", format(&inline)),
                Event::Rule => {
                    flush(ui, &mut job, depth, quote_depth, &mut used);
                    used = used.union(ui.separator().rect);
                }
                _ => {}
            }
        }
        flush(
            ui,
            &mut job,
            lists.len() + quote_depth,
            quote_depth,
            &mut used,
        );
        let rect = if used == Rect::NOTHING {
            Rect::from_min_size(start, vec2(0.0, 0.0))
        } else {
            used
        };
        let id = ui.next_auto_id();
        ui.interact(id, rect, Sense::HOVER)
    }
}

/// The text style of a heading.
fn heading_style(body: &TextStyle, level: HeadingLevel) -> TextStyle {
    let (scale, weight) = match level {
        HeadingLevel::H1 => (1.75, 700),
        HeadingLevel::H2 => (1.45, 700),
        HeadingLevel::H3 => (1.25, 650),
        _ => (1.1, 600),
    };
    TextStyle {
        size: body.size * scale,
        ..body.clone()
    }
    .weight(weight)
}

impl Ui<'_> {
    /// Shows Markdown text (see [`Markdown`]).
    pub fn markdown(&mut self, source: &str) -> Response {
        Markdown::new(source).show(self)
    }
}
