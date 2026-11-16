//! Drawing the cells in view, and saying which they are.

use iced::alignment::Vertical;
use iced::font::Weight;
use iced::theme::palette::Palette;
use iced::widget::canvas::{self, Frame};
use iced::widget::text;
use iced::{Color, Font, Point, Rectangle, Size};

use super::{COLUMN, HEADER, ROW, View, value};
use crate::polars::prelude::DataFrame;

const PAD: f32 = 6.0;

impl View {
    /// Draws the part of `frame` in view into `body`, which the canvas must clip to.
    pub fn draw(
        &self,
        canvas: &mut Frame,
        frame: &DataFrame,
        body: Rectangle,
        palette: &Palette,
    ) {
        let columns = frame.columns();
        let first = self.row.floor() as usize;
        let rows =
            first..(first + ((body.height - HEADER) / ROW).ceil() as usize + 1).min(frame.height());
        let left = (self.x / COLUMN) as usize;
        let shown = left..(left + (body.width / COLUMN).ceil() as usize + 1).min(columns.len());
        // where row `first` and column 0 start
        let top = body.y + HEADER;
        let x = body.x - self.x;
        let ink = palette
            .background
            .base
            .text;
        let bold = Font::DEFAULT.weight(Weight::Bold);

        for row in rows
            .clone()
            .filter(|row| row % 2 == 1)
        {
            let y = top + (row - first) as f32 * ROW;
            let stripe = Size::new(body.width, ROW);
            canvas.fill_rectangle(
                Point::new(body.x, y),
                stripe,
                palette
                    .background
                    .weakest
                    .color,
            );
        }
        if let Some(((r0, c0), (r1, c1))) = self.span(frame) {
            let corner = Point::new(
                x + c0 as f32 * COLUMN,
                top + (r0 as f32 - first as f32) * ROW,
            );
            let size = Size::new((c1 - c0 + 1) as f32 * COLUMN, (r1 - r0 + 1) as f32 * ROW);
            canvas.fill_rectangle(
                corner,
                size,
                palette
                    .primary
                    .weak
                    .color,
            );
        }
        // the header over the rows scrolled under it
        canvas.fill_rectangle(
            body.position(),
            Size::new(body.width, HEADER),
            palette
                .background
                .base
                .color,
        );
        canvas.fill_rectangle(
            Point::new(body.x, body.y + HEADER - 1.0),
            Size::new(body.width, 1.0),
            palette
                .background
                .strongest
                .color,
        );

        for (index, column) in columns
            .iter()
            .enumerate()
            .take(shown.end)
            .skip(shown.start)
        {
            let x = x + index as f32 * COLUMN;
            let numeric = column
                .dtype()
                .is_numeric();
            cell(
                canvas,
                column.name(),
                x,
                body.y + (HEADER - ROW) / 2.0,
                numeric,
                ink,
                bold,
            );
            for row in rows.clone() {
                let y = top + (row - first) as f32 * ROW;
                cell(
                    canvas,
                    &value(column, row),
                    x,
                    y,
                    numeric,
                    ink,
                    Font::DEFAULT,
                );
            }
        }
    }

    /// Which rows and columns of `frame` are in view in `body`.
    pub fn caption(
        &self,
        frame: &DataFrame,
        body: Size,
    ) -> String {
        let first = self.row.floor() as usize;
        let last = (first + ((body.height - HEADER) / ROW) as usize).min(frame.height());
        let mut caption = format!(
            "rows {}–{last} of {}",
            (first + 1).min(last),
            frame.height()
        );
        if frame.width() as f32 * COLUMN > body.width {
            let left = (self.x / COLUMN) as usize;
            let right = (left + (body.width / COLUMN) as usize).min(frame.width());
            caption += &format!(" · columns {}–{right} of {}", left + 1, frame.width());
        }
        caption
    }
}

/// Writes a cell's text in the column at `x` and the row at `y`, numbers to the right.
fn cell(
    canvas: &mut Frame,
    content: &str,
    x: f32,
    y: f32,
    numeric: bool,
    color: Color,
    font: Font,
) {
    let (position, align_x) = match numeric {
        true => (
            Point::new(x + COLUMN - PAD, y + ROW / 2.0),
            text::Alignment::Right,
        ),
        false => (Point::new(x + PAD, y + ROW / 2.0), text::Alignment::Left),
    };
    canvas.fill_text(canvas::Text {
        content: content.to_owned(),
        position,
        color,
        size: 12.0.into(),
        font,
        align_x,
        align_y: Vertical::Center,
        max_width: COLUMN - 2.0 * PAD,
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::End,
        ..canvas::Text::default()
    });
}
