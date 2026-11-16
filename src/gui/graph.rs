//! The pipeline as a graph on a canvas: a card per node, a port per input, a wire per reading.

mod card;
mod draw;
mod layout;

use std::borrow::Cow;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::keyboard::{self, Key};
use iced::widget::canvas;
use iced::{Point, Rectangle, Renderer, Size, Theme, Vector, mouse};
use serde_json::Value;

use self::layout::position;
pub use self::layout::{beside, is_open, layout, place, resize, toggle};
use super::table;
use crate::Pipeline;
use crate::polars::prelude::DataFrame;

const WIDTH: f32 = 180.0;
/// The height of a card's id and block, and of each of its inputs below.
const HEADER: f32 = 46.0;
const INPUT: f32 = 22.0;
/// The height of the "save output" box of a card, below its inputs.
const SAVE: f32 = 22.0;
const PORT: f32 = 5.0;
/// The size of an open card, until it is resized, and of its corner grip.
const TABLE: Size = Size::new(520.0, 320.0);
const GRIP: f32 = 14.0;
/// How soon a second press must follow a first to make a double click.
const DOUBLE: Duration = Duration::from_millis(400);
/// The width of a border that shows a status, which grows into its card.
const STATUS: f32 = 5.0;

/// What was done to the graph.
#[derive(Debug, Clone)]
pub enum Event {
    Select(usize),
    Move(usize, Point),
    Resize(usize, Size),
    /// Node `to` reads node `from` as its input `slot`, or nothing.
    Wire {
        to: usize,
        slot: usize,
        from: Option<usize>,
    },
    /// The menu of blocks is asked for at `at` on the canvas, which is `point` of the graph.
    Menu {
        at: Point,
        point: Point,
    },
    /// The node's card is to be laid open, or closed if it is open.
    Toggle(usize),
    /// Whether a Run saves the node's output is to be the other way.
    Save(usize),
    /// Text to put on the clipboard.
    Copy(String),
}

/// What is known of a node, which colours the border of its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Nothing, but that it is wired well.
    Unknown,
    /// Its columns resolve.
    Checked,
    /// It is wired badly, or its columns do not resolve.
    Rejected,
    /// A Run computed it.
    Ran,
    /// A Run of it failed.
    Failed,
}

/// The graph of a pipeline, to be drawn on a canvas.
pub struct Graph<'a> {
    pub pipeline: &'a Pipeline,
    /// The catalog's blocks, which name each node's inputs.
    pub blocks: &'a Value,
    pub selected: Option<usize>,
    /// What is known of each node, in the order of the nodes.
    pub status: Vec<Status>,
    /// What the open card of each node shows: its saved frame, or else its columns.
    pub tables: Vec<Option<Cow<'a, DataFrame>>>,
    /// Whether each node has a saved frame.
    pub saved: Vec<bool>,
}

/// The view of a graph, and what is being dragged across it.
#[derive(Default)]
pub struct State {
    offset: Vector,
    /// How many steps of the scroll wheel the view is zoomed in by.
    zoom: f32,
    drag: Option<Drag>,
    /// The view of each table that was scrolled or pressed, by node id, and the table's shape.
    views: HashMap<String, (table::View, (usize, usize))>,
    /// The node whose table was pressed last, which Ctrl+C copies from.
    active: Option<String>,
    /// When and where the latest press was, which tells a double click.
    pressed: Option<(Instant, Point)>,
    modifiers: keyboard::Modifiers,
}

enum Drag {
    /// The view, from where the cursor was last.
    View(Point),
    /// A card, held this far from its corner.
    Card(usize, Vector),
    /// A wire from this node's output, to wherever the cursor is.
    Wire(usize, Point),
    /// The corner of this node's open card.
    Corner(usize),
    /// A selection across the cells of this node's table.
    Cells(usize),
}

impl State {
    fn scale(&self) -> f32 {
        1.1_f32.powf(self.zoom)
    }

    /// Where on the canvas a point of the graph is drawn.
    fn project(
        &self,
        point: Point,
    ) -> Point {
        Point::ORIGIN + self.offset + (point - Point::ORIGIN) * self.scale()
    }

    /// The point of the graph that is drawn at `at` on the canvas.
    fn locate(
        &self,
        at: Point,
    ) -> Point {
        let Vector { x, y } = (at - self.offset - Point::ORIGIN) * (1.0 / self.scale());
        Point::new(x, y)
    }
}

impl canvas::Program<Event> for Graph<'_> {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Event>> {
        let nodes = &self.pipeline.nodes;
        // a table of another shape, as after a Run, starts afresh
        state
            .views
            .retain(|id, (_, shape)| {
                nodes
                    .iter()
                    .position(|node| node.id == *id)
                    .and_then(|index| self.shown(index))
                    .is_some_and(|(_, table)| table.shape() == *shape)
            });

        let event = match event {
            canvas::Event::Mouse(event) => event,
            canvas::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
                return None;
            }
            canvas::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if modifiers.command() && key.as_ref() == Key::Character("c") =>
            {
                let id = state
                    .active
                    .as_ref()?;
                let (view, _) = state
                    .views
                    .get(id)?;
                let index = nodes
                    .iter()
                    .position(|node| node.id == *id)?;
                let text = view.copy(self.shown(index)?.1)?;
                return Some(canvas::Action::publish(Event::Copy(text)).and_capture());
            }
            _ => return None,
        };
        let at = cursor.position_from(bounds.position())?;
        let point = state.locate(at);

        match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) if cursor.is_over(bounds) => {
                let now = Instant::now();
                let is_double = state
                    .pressed
                    .is_some_and(|(then, there)| {
                        now.duration_since(then) <= DOUBLE && there.distance(at) <= 6.0
                    });
                // a third press starts over
                state.pressed = (!is_double).then_some((now, at));

                if let Some(index) = self.grip_at(point) {
                    state.drag = Some(Drag::Corner(index));
                    Some(canvas::Action::capture())
                } else if let Some((index, body, table)) = self.shown_at(point) {
                    let id = &nodes[index].id;
                    let (view, _) = state
                        .views
                        .entry(id.clone())
                        .or_insert_with(|| (table::View::default(), table.shape()));
                    view.press(view.cell_at(point, table, body));
                    state.active = Some(id.clone());
                    state.drag = Some(Drag::Cells(index));
                    Some(canvas::Action::request_redraw().and_capture())
                } else if let Some(index) = self.save_at(point) {
                    Some(canvas::Action::publish(Event::Save(index)).and_capture())
                } else if let Some(from) = self.output_at(point) {
                    state.drag = Some(Drag::Wire(from, point));
                    Some(canvas::Action::request_redraw().and_capture())
                } else if let Some(index) = self.card_at(point)
                    && is_double
                    && self
                        .header(index)
                        .contains(point)
                {
                    Some(canvas::Action::publish(Event::Toggle(index)).and_capture())
                } else if let Some(index) = self.card_at(point) {
                    let held = point - position(&self.pipeline.nodes[index]);
                    state.drag = Some(Drag::Card(index, held));
                    Some(canvas::Action::publish(Event::Select(index)).and_capture())
                } else {
                    state.drag = Some(Drag::View(at));
                    Some(canvas::Action::capture())
                }
            }
            mouse::Event::ButtonPressed(mouse::Button::Right) if cursor.is_over(bounds) => {
                let is_card = self
                    .card_at(point)
                    .is_some();
                let event = match self.input_at(point) {
                    Some((to, slot)) => Event::Wire {
                        to,
                        slot,
                        from: None,
                    },
                    // the menu is of the background, not of a card
                    None if is_card => return None,
                    None => Event::Menu { at, point },
                };
                Some(canvas::Action::publish(event).and_capture())
            }
            mouse::Event::CursorMoved { .. } => match state
                .drag
                .as_mut()?
            {
                Drag::View(last) => {
                    state.offset += at - *last;
                    *last = at;
                    Some(canvas::Action::request_redraw())
                }
                Drag::Card(index, held) => {
                    Some(canvas::Action::publish(Event::Move(*index, point - *held)))
                }
                Drag::Wire(_, end) => {
                    *end = point;
                    Some(canvas::Action::request_redraw())
                }
                Drag::Corner(index) => {
                    let node = &self.pipeline.nodes[*index];
                    let Vector { x, y } = point - position(node);
                    let least = self.top(node) + table::HEADER + table::ROW;
                    let size = Size::new(x.max(WIDTH), y.max(least));
                    Some(canvas::Action::publish(Event::Resize(*index, size)))
                }
                Drag::Cells(index) => {
                    let (body, table) = self.shown(*index)?;
                    let (view, _) = state
                        .views
                        .get_mut(&nodes[*index].id)?;
                    view.reach(view.cell_at(point, table, body)?);
                    Some(canvas::Action::request_redraw())
                }
            },
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                let wire = match state.drag.take()? {
                    Drag::Wire(from, end) => self
                        .input_at(end)
                        .map(|(to, slot)| Event::Wire {
                            to,
                            slot,
                            from: Some(from),
                        }),
                    _ => None,
                };
                Some(wire.map_or(canvas::Action::request_redraw(), canvas::Action::publish))
            }
            mouse::Event::WheelScrolled { delta } if cursor.is_over(bounds) => {
                if let Some((index, body, table)) = self.shown_at(point) {
                    // a notch of the wheel is three rows, either way
                    let notch = 3.0 * table::ROW;
                    let (x, y) = match *delta {
                        mouse::ScrollDelta::Lines { x, y } => (x * notch, y * notch),
                        mouse::ScrollDelta::Pixels { x, y } => (x, y),
                    };
                    let (x, y) = match state
                        .modifiers
                        .shift()
                    {
                        true => (y, 0.0),
                        false => (x, y),
                    };
                    let (view, _) = state
                        .views
                        .entry(
                            nodes[index]
                                .id
                                .clone(),
                        )
                        .or_insert_with(|| (table::View::default(), table.shape()));
                    view.scroll(-(y / table::ROW) as f64, -x, table, body.size());
                    return Some(canvas::Action::request_redraw().and_capture());
                }

                let steps = match *delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 50.0,
                };
                state.zoom = (state.zoom + steps).clamp(-15.0, 10.0);
                // what is under the cursor stays under it
                let moved = state.locate(at) - point;
                state.offset += moved * state.scale();
                Some(canvas::Action::request_redraw().and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        draw::draw(self, state, renderer, theme, bounds)
    }

    fn mouse_interaction(
        &self,
        state: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let Some(at) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };
        let point = state.locate(at);
        let is_port = self
            .output_at(point)
            .is_some()
            || self
                .input_at(point)
                .is_some();

        let is_grip = self
            .grip_at(point)
            .is_some();
        let is_body = self
            .shown_at(point)
            .is_some();
        let is_card = self
            .card_at(point)
            .is_some();

        match state.drag {
            Some(Drag::Corner(_)) => mouse::Interaction::ResizingDiagonallyDown,
            Some(Drag::Cells(_)) => mouse::Interaction::Cell,
            Some(_) => mouse::Interaction::Grabbing,
            None if is_grip => mouse::Interaction::ResizingDiagonallyDown,
            None if is_body => mouse::Interaction::Cell,
            None if is_port => mouse::Interaction::Crosshair,
            None if is_card => mouse::Interaction::Grab,
            None => mouse::Interaction::default(),
        }
    }
}
