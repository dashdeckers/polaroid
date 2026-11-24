//! A table under its headings: only the cells in view are drawn, whatever its size.

use iced::alignment::Vertical;
use iced::font::Weight;
use iced::keyboard::{self, Key};
use iced::widget::canvas::{self, Frame};
use iced::widget::text;
use iced::{Color, Font, Point, Rectangle, Renderer, Size, Theme, mouse};

use crate::width_of;

/// The height of a row, and of the names of the columns over their kinds.
const ROW: f32 = 20.0;
const HEADER: f32 = 38.0;
const COLUMN: f32 = 140.0;
const PAD: f32 = 6.0;
const TEXT: f32 = 12.0;

/// A row and a column.
type Cell = (usize, usize);

/// What a table asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// This text is to be copied: the cells selected when Ctrl+C was pressed.
    Copy(String),
    /// A cell of this row was pressed.
    Row(usize),
}

/// What heads a column: its name over its kind; cells of numbers are set to the right.
pub struct Heading<'a> {
    pub name: &'a str,
    pub kind: String,
    pub is_numeric: bool,
}

/// A table of these columns and no rows, to draw on a canvas.
pub fn table(columns: Vec<Heading<'_>>) -> Table<'_> {
    Table {
        columns,
        cells: None,
    }
}

/// Columns over their rows, if it has any; it asks for a row pressed and the text Ctrl+C copies.
pub struct Table<'a> {
    columns: Vec<Heading<'a>>,
    /// How many rows there are, and the text of the cell of a row and a column.
    cells: Option<(usize, Cells<'a>)>,
}

/// What gives the text of the cell of a row and a column.
type Cells<'a> = Box<dyn Fn(usize, usize) -> String + 'a>;

impl<'a> Table<'a> {
    /// Gives it this many rows, numbered on the left, with `cell` for the text of each cell.
    pub fn rows(
        mut self,
        rows: usize,
        cell: impl Fn(usize, usize) -> String + 'a,
    ) -> Self {
        self.cells = Some((rows, Box::new(cell)));
        self
    }

    /// The least height that shows it: its header, and up to five of its rows if it was given any.
    pub fn least(&self) -> f32 {
        match self.cells {
            Some((rows, _)) => HEADER + ROW * rows.clamp(1, 5) as f32,
            None => HEADER,
        }
    }

    fn height(&self) -> usize {
        self.cells
            .as_ref()
            .map_or(0, |(rows, _)| *rows)
    }

    /// The width of the numbers of the rows, left of the cells.
    fn gutter(&self) -> f32 {
        match self.cells {
            Some((rows, _)) => 2.0 * PAD + width_of(&rows.to_string(), TEXT, Font::DEFAULT),
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
        (cell.0 < self.height() && cell.1 < self.columns.len()).then_some(cell)
    }

    /// The selected cells as tab-separated lines, headed by their names unless it is one cell.
    fn copy(
        &self,
        view: &View,
    ) -> Option<String> {
        let ((top, left), (bottom, right)) = view.span()?;
        let (_, cell) = self
            .cells
            .as_ref()?;
        let is_headed = (top, left) != (bottom, right);
        let header = is_headed.then(|| {
            self.columns[left..=right]
                .iter()
                .map(|column| column.name)
                .collect::<Vec<_>>()
                .join("\t")
        });
        let rows = (top..=bottom).map(|row| {
            (left..=right)
                .map(|column| cell(row, column))
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

impl canvas::Program<Asked> for Table<'_> {
    type State = View;

    fn update(
        &self,
        view: &mut View,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Asked>> {
        let shape = (self.height(), self.columns.len());
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
                Some(canvas::Action::publish(Asked::Copy(self.copy(view)?)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // a press elsewhere, as in a text field, takes Ctrl+C from the table
                view.is_active = cursor.is_over(bounds);
                let point = cursor.position_in(bounds)?;
                let cell = self.cell_at(view, point);
                view.selection = cell.map(|cell| (cell, cell));
                view.is_selecting = true;
                let action = match cell {
                    Some((row, _)) => canvas::Action::publish(Asked::Row(row)),
                    None => canvas::Action::request_redraw(),
                };
                Some(action.and_capture())
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
                let last = (self.height() as f64 - shown.max(1.0)).max(0.0);
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
        let rows = first..(first + (body.height / ROW).ceil() as usize + 1).min(self.height());
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
            frame.fill_text(line(&number, 0.0, down(row), body.x, true, muted));
        }
        // the names and kinds of the columns scroll along with their cells
        let header = Rectangle::new(Point::new(body.x, 0.0), Size::new(body.width, HEADER));
        frame.with_clip(header, |frame| {
            for index in shown.clone() {
                let column = &self.columns[index];
                let numeric = column.is_numeric;
                let x = along(index);
                frame.fill_text(canvas::Text {
                    font: bold,
                    ..line(column.name, x, 0.0, COLUMN, numeric, ink)
                });
                frame.fill_text(line(&column.kind, x, HEADER - ROW, COLUMN, numeric, muted));
            }
        });
        let Some((_, cell)) = &self.cells else {
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
                let numeric = self.columns[index].is_numeric;
                for row in rows.clone() {
                    let text = cell(row, index);
                    frame.fill_text(line(&text, along(index), down(row), COLUMN, numeric, ink));
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

/// A line of text in a cell of this `width` at `x` and `y`, numbers to the right.
fn line(
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
        size: TEXT.into(),
        align_x,
        align_y: Vertical::Center,
        max_width: width - 2.0 * PAD,
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::End,
        ..canvas::Text::default()
    }
}
