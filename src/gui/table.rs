//! A frame as a table under its columns; only the cells in view are drawn, whatever its size.

use iced::alignment::Vertical;
use iced::font::Weight;
use iced::keyboard::{self, Key};
use iced::widget::canvas::{self, Frame};
use iced::widget::text;
use iced::{Color, Font, Point, Rectangle, Renderer, Size, Theme, mouse};
use polaroid_iced::width_of;

use super::NOTE;
use crate::polars::prelude::{Column, DataFrame, DataType};

pub const ROW: f32 = 20.0;
/// The height of the names of the columns over their types.
pub const HEADER: f32 = 38.0;
const COLUMN: f32 = 140.0;
const PAD: f32 = 6.0;

/// A row and a column.
type Cell = (usize, usize);

/// The columns of a node by name and type, over its rows if a Run saved them.
pub struct Table<'a> {
    columns: Vec<(&'a str, &'a DataType)>,
    frame: Option<&'a DataFrame>,
}

impl<'a> Table<'a> {
    pub fn of(frame: &'a DataFrame) -> Self {
        Self {
            columns: frame
                .columns()
                .iter()
                .map(|column| {
                    (
                        column
                            .name()
                            .as_str(),
                        column.dtype(),
                    )
                })
                .collect(),
            frame: Some(frame),
        }
    }

    /// A table of no rows, as a Check finds the columns alone.
    pub fn listing(columns: &'a [(String, DataType)]) -> Self {
        Self {
            columns: columns
                .iter()
                .map(|(name, dtype)| (name.as_str(), dtype))
                .collect(),
            frame: None,
        }
    }

    /// The least height that shows it: its header, and up to five of its rows if it has any.
    pub fn least(&self) -> f32 {
        match self.frame {
            Some(frame) => {
                HEADER
                    + ROW
                        * frame
                            .height()
                            .clamp(1, 5) as f32
            }
            None => HEADER,
        }
    }

    fn rows(&self) -> usize {
        self.frame
            .map_or(0, DataFrame::height)
    }

    /// The width of the numbers of the rows, left of the cells.
    fn gutter(&self) -> f32 {
        match self.frame {
            Some(frame) => {
                let last = frame
                    .height()
                    .to_string();
                2.0 * PAD + width_of(&last, NOTE, Font::DEFAULT)
            }
            None => 0.0,
        }
    }

    /// The cells of the canvas, right of the numbers and below the header.
    fn body(
        &self,
        bounds: Size,
    ) -> Rectangle {
        let gutter = self.gutter();
        Rectangle::new(
            Point::new(gutter, HEADER),
            Size::new(
                (bounds.width - gutter).max(0.0),
                (bounds.height - HEADER).max(0.0),
            ),
        )
    }

    /// The cell drawn at `point` of the canvas, if there is one.
    fn cell_at(
        &self,
        view: &View,
        point: Point,
    ) -> Option<Cell> {
        let down = point.y - HEADER;
        let along = point.x - self.gutter() + view.x;
        if down < 0.0 || along < 0.0 {
            return None;
        }
        let cell = (
            view.row as usize + (down / ROW) as usize,
            (along / COLUMN) as usize,
        );
        (cell.0 < self.rows() && cell.1 < self.columns.len()).then_some(cell)
    }

    /// The selected cells as tab-separated lines, headed by their names unless it is one cell.
    fn copy(
        &self,
        view: &View,
    ) -> Option<String> {
        let ((top, left), (bottom, right)) = view.span()?;
        let columns = self
            .frame?
            .columns()
            .get(left..=right)?;
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
}

/// Where a table is scrolled to, and the cells selected in it.
#[derive(Default)]
pub struct View {
    /// The rows and columns of the table it is a view of; another table starts afresh.
    shape: (usize, usize),
    /// The row at the top. Small scrolls add up in its fraction.
    row: f64,
    x: f32,
    /// From the cell pressed to the one the cursor reached.
    selection: Option<(Cell, Cell)>,
    is_selecting: bool,
    /// Whether the latest press was on this table, which Ctrl+C then copies from.
    is_active: bool,
    modifiers: keyboard::Modifiers,
}

impl View {
    /// The selection as its top-left and bottom-right cells.
    fn span(&self) -> Option<(Cell, Cell)> {
        let ((r0, c0), (r1, c1)) = self.selection?;
        Some(((r0.min(r1), c0.min(c1)), (r0.max(r1), c0.max(c1))))
    }
}

/// A table publishes the text that Ctrl+C copies from it.
impl canvas::Program<String> for Table<'_> {
    type State = View;

    fn update(
        &self,
        view: &mut View,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<String>> {
        let shape = (self.rows(), self.columns.len());
        if view.shape != shape {
            *view = View {
                shape,
                modifiers: view.modifiers,
                ..View::default()
            };
        }
        let body = self.body(bounds.size());

        match event {
            canvas::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                view.modifiers = *modifiers;
                None
            }
            canvas::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if view.is_active && modifiers.command() && key.as_ref() == Key::Character("c") =>
            {
                Some(canvas::Action::publish(self.copy(view)?).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // a press elsewhere, as in a text field, takes Ctrl+C from the table
                view.is_active = cursor.is_over(bounds);
                let point = cursor.position_in(bounds)?;
                let cell = self.cell_at(view, point);
                view.selection = cell.map(|cell| (cell, cell));
                view.is_selecting = true;
                Some(canvas::Action::request_redraw().and_capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) if view.is_selecting => {
                let point = *position - (bounds.position() - Point::ORIGIN);
                let cell = self.cell_at(view, point)?;
                if let Some((_, end)) = &mut view.selection {
                    *end = cell;
                }
                Some(canvas::Action::request_redraw())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                view.is_selecting = false;
                None
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(bounds) =>
            {
                // a notch of the wheel is three rows, either way
                let notch = 3.0 * ROW;
                let (x, y) = match *delta {
                    mouse::ScrollDelta::Lines { x, y } => (x * notch, y * notch),
                    mouse::ScrollDelta::Pixels { x, y } => (x, y),
                };
                let (x, y) = match view
                    .modifiers
                    .shift()
                {
                    true => (y, 0.0),
                    false => (x, y),
                };
                let shown = (body.height / ROW).floor() as f64;
                // less a row at the least, so that the last one stays in view
                let last = (self.rows() as f64 - shown.max(1.0)).max(0.0);
                let width = self.columns.len() as f32 * COLUMN;
                let right = (width - body.width).max(0.0);
                // a table that is all in view leaves the wheel to what it lies on
                if last == 0.0 && right == 0.0 {
                    return None;
                }
                view.row = (view.row - (y / ROW) as f64).clamp(0.0, last);
                view.x = (view.x - x).clamp(0.0, right);
                Some(canvas::Action::request_redraw().and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        view: &View,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.palette();
        let back = palette.background;
        let ink = back.base.text;
        let muted = back.strongest.color;
        let bold = Font::DEFAULT.weight(Weight::Bold);
        let body = self.body(bounds.size());

        let first = view.row.floor() as usize;
        let rows = first..(first + (body.height / ROW).ceil() as usize + 1).min(self.rows());
        let left = (view.x / COLUMN) as usize;
        let shown =
            left..(left + (body.width / COLUMN).ceil() as usize + 1).min(self.columns.len());
        // where a row and a column start on the canvas
        let down = |row: usize| body.y + (row - first) as f32 * ROW;
        let along = |column: usize| body.x - view.x + column as f32 * COLUMN;

        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), back.base.color);
        for row in rows
            .clone()
            .filter(|row| row % 2 == 1)
        {
            let stripe = Size::new(bounds.width, ROW);
            frame.fill_rectangle(Point::new(0.0, down(row)), stripe, back.weakest.color);
        }
        frame.fill_rectangle(
            Point::new(0.0, HEADER - 1.0),
            Size::new(bounds.width, 1.0),
            muted,
        );

        // the numbers of the rows stay put, left of whatever is scrolled to
        for row in rows.clone() {
            let number = (row + 1).to_string();
            frame.fill_text(cell(&number, 0.0, down(row), body.x, true, muted));
        }
        // the names and types of the columns scroll along with their cells
        let header = Rectangle::new(Point::new(body.x, 0.0), Size::new(body.width, HEADER));
        frame.with_clip(header, |frame| {
            for index in shown.clone() {
                let (name, dtype) = self.columns[index];
                let numeric = dtype.is_numeric();
                let x = along(index);
                frame.fill_text(canvas::Text {
                    font: bold,
                    ..cell(name, x, 0.0, COLUMN, numeric, ink)
                });
                let dtype = dtype.to_string();
                frame.fill_text(cell(&dtype, x, HEADER - ROW, COLUMN, numeric, muted));
            }
        });
        let Some(table) = self.frame else {
            return vec![frame.into_geometry()];
        };
        frame.with_clip(body, |frame| {
            if let Some(((r0, c0), (r1, c1))) = view.span() {
                // from the first row in view, as nothing is drawn above the cells
                let r0 = r0.max(first);
                let size = Size::new(
                    (c1 - c0 + 1) as f32 * COLUMN,
                    (r1 + 1).saturating_sub(r0) as f32 * ROW,
                );
                let selected = palette
                    .primary
                    .weak
                    .color;
                frame.fill_rectangle(Point::new(along(c0), down(r0)), size, selected);
            }
            for index in shown {
                let column = &table.columns()[index];
                let numeric = column
                    .dtype()
                    .is_numeric();
                for row in rows.clone() {
                    let value = value(column, row);
                    frame.fill_text(cell(&value, along(index), down(row), COLUMN, numeric, ink));
                }
            }
        });
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _view: &View,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match cursor.is_over(bounds) {
            true => mouse::Interaction::Cell,
            false => mouse::Interaction::None,
        }
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

/// A line of text in a cell of this `width` at `x` and `y`, numbers to the right.
fn cell(
    content: &str,
    x: f32,
    y: f32,
    width: f32,
    numeric: bool,
    color: Color,
) -> canvas::Text {
    let (x, align_x) = match numeric {
        true => (x + width - PAD, text::Alignment::Right),
        false => (x + PAD, text::Alignment::Left),
    };
    canvas::Text {
        content: content.to_owned(),
        position: Point::new(x, y + ROW / 2.0),
        color,
        size: NOTE.into(),
        align_x,
        align_y: Vertical::Center,
        max_width: width - 2.0 * PAD,
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::End,
        ..canvas::Text::default()
    }
}
