//! How wide text is when it is drawn.

use iced::advanced::Text;
use iced::advanced::graphics::text::Paragraph;
use iced::advanced::text::{self, Paragraph as _};
use iced::alignment::Vertical;
use iced::{Font, Size};

/// The width of a line of text of this size in this font.
pub fn width_of(
    content: &str,
    size: f32,
    font: Font,
) -> f32 {
    let line = Text {
        content,
        bounds: Size::INFINITE,
        size: size.into(),
        line_height: text::LineHeight::default(),
        font,
        align_x: text::Alignment::Default,
        align_y: Vertical::Top,
        shaping: text::Shaping::default(),
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::None,
        hint_factor: None,
    };
    Paragraph::with_text(line).min_width()
}
