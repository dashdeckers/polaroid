//! A frame as a table on the canvas; only the cells in view are drawn, whatever its size.

mod draw;

use iced::{Point, Rectangle, Size};

use crate::polars::prelude::{Column, DataFrame};

pub const ROW: f32 = 20.0;
pub const HEADER: f32 = 24.0;
pub const COLUMN: f32 = 140.0;

/// A row and a column.
pub type Cell = (usize, usize);

/// Where a table is scrolled to, and the cells selected in it.
#[derive(Debug, Default)]
pub struct View {
    /// The row at the top. Small scrolls add up in its fraction.
    row: f64,
    x: f32,
    /// From the cell pressed to the one the cursor reached.
    selection: Option<(Cell, Cell)>,
}

impl View {
    /// Scrolls by `rows` and `x` pixels, as far as `frame` goes within `body`.
    pub fn scroll(
        &mut self,
        rows: f64,
        x: f32,
        frame: &DataFrame,
        body: Size,
    ) {
        let shown = ((body.height - HEADER) / ROW).floor() as f64;
        let last = (frame.height() as f64 - shown).max(0.0);
        self.row = (self.row + rows).clamp(0.0, last);
        let width = frame.width() as f32 * COLUMN;
        self.x = (self.x + x).clamp(0.0, (width - body.width).max(0.0));
    }

    /// The cell drawn at `point`, if `frame` has one there.
    pub fn cell_at(
        &self,
        point: Point,
        frame: &DataFrame,
        body: Rectangle,
    ) -> Option<Cell> {
        let down = point.y - body.y - HEADER;
        let along = point.x - body.x + self.x;
        if down < 0.0 || along < 0.0 {
            return None;
        }
        let cell = (
            self.row as usize + (down / ROW) as usize,
            (along / COLUMN) as usize,
        );
        (cell.0 < frame.height() && cell.1 < frame.width()).then_some(cell)
    }

    /// Selects `cell`, or nothing.
    pub fn press(
        &mut self,
        cell: Option<Cell>,
    ) {
        self.selection = cell.map(|cell| (cell, cell));
    }

    /// Extends the selection to `cell`.
    pub fn reach(
        &mut self,
        cell: Cell,
    ) {
        if let Some((_, end)) = &mut self.selection {
            *end = cell;
        }
    }

    /// The selected cells as tab-separated lines, headed by their names unless it is one cell.
    pub fn copy(
        &self,
        frame: &DataFrame,
    ) -> Option<String> {
        let ((top, left), (bottom, right)) = self.span(frame)?;
        let columns = &frame.columns()[left..=right];
        let names = (top, left) != (bottom, right);
        let header = names.then(|| {
            columns
                .iter()
                .map(|column| {
                    column
                        .name()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("\t")
        });
        let rows = (top..=bottom).map(|row| {
            columns
                .iter()
                .map(|column| value(column, row))
                .collect::<Vec<_>>()
                .join("\t")
        });
        Some(
            header
                .into_iter()
                .chain(rows)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// The selection as its top-left and bottom-right cells, within `frame`.
    fn span(
        &self,
        frame: &DataFrame,
    ) -> Option<(Cell, Cell)> {
        let ((r0, c0), (r1, c1)) = self.selection?;
        let bottom = r0.max(r1).min(
            frame
                .height()
                .checked_sub(1)?,
        );
        let right = c0.max(c1).min(
            frame
                .width()
                .checked_sub(1)?,
        );
        let span = ((r0.min(r1), c0.min(c1)), (bottom, right));
        (span.0.0 <= bottom && span.0.1 <= right).then_some(span)
    }
}

fn value(
    column: &Column,
    row: usize,
) -> String {
    column
        .get(row)
        .map(|value| {
            value
                .str_value()
                .into_owned()
        })
        .unwrap_or_default()
}
