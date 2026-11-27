//! How wide text is when it is drawn, and where its lines break.

use iced::advanced::Text;
use iced::advanced::graphics::text::Paragraph;
use iced::advanced::text::{self, Paragraph as _, Span};
use iced::alignment::Vertical;
use iced::{Font, Size};

/// `content` of this size in this font, to lay out in lines no wider than `width`.
fn sized<Content>(
    content: Content,
    size: f32,
    font: Font,
    width: f32,
    wrapping: text::Wrapping,
) -> Text<Content> {
    Text {
        content,
        bounds: Size::new(width, f32::INFINITY),
        size: size.into(),
        line_height: crate::theme::LINE_HEIGHT,
        font,
        align_x: text::Alignment::Default,
        align_y: Vertical::Top,
        shaping: text::Shaping::default(),
        wrapping,
        ellipsis: text::Ellipsis::None,
        hint_factor: None,
    }
}

/// The width of a line of text of this size in this font.
pub fn width_of(
    content: &str,
    size: f32,
    font: Font,
) -> f32 {
    let line = sized(content, size, font, f32::INFINITY, text::Wrapping::None);
    Paragraph::with_text(line).min_width()
}

/// `spans` wrapped at `width`, cut where a line ends: their first `lines` lines, and the rest.
pub fn cut<'a, Link: Clone>(
    spans: &[Span<'a, Link>],
    size: f32,
    font: Font,
    width: f32,
    lines: usize,
) -> (Vec<Span<'a, Link>>, Vec<Span<'a, Link>>) {
    let wrapped = sized(spans, size, font, width, text::Wrapping::default());
    let paragraph = Paragraph::with_spans(wrapped);
    let buffer = paragraph.buffer();
    // where the first line of the rest starts in the text of all spans, if there is such a line
    let at = buffer
        .layout_runs()
        .nth(lines)
        .map_or(usize::MAX, |run| {
            let before: usize = buffer.lines[..run.line_i]
                .iter()
                .map(|line| {
                    let ending = line
                        .ending()
                        .as_str();
                    line.text().len() + ending.len()
                })
                .sum();
            let start = run
                .glyphs
                .iter()
                .map(|glyph| glyph.start)
                .min();
            before + start.unwrap_or(0)
        });
    let (mut first, mut rest) = (Vec::new(), Vec::new());
    let mut from = 0;
    for span in spans {
        let to = from + span.text.len();
        let (head, tail) = span
            .text
            .split_at(at.clamp(from, to) - from);
        for (part, written) in [
            (&mut first, head),
            (&mut rest, tail),
        ] {
            if !written.is_empty() {
                part.push(Span {
                    text: written
                        .to_owned()
                        .into(),
                    ..span.clone()
                });
            }
        }
        from = to;
    }
    (first, rest)
}
