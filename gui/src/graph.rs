//! The pipeline as a graph on a canvas: each node a card with a port per input
//! on its left and its output on its right, each input a wire. Drag a card to
//! move it, the background to pan, and from an output to an input to connect
//! them; scroll to zoom, and right-click an input to disconnect it.

use std::collections::HashMap;

use iced::mouse::{self, ScrollDelta};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Size, Theme, Vector};
use polaroid_core::{Node, Pipeline};
use serde_json::{Value, json};

const WIDTH: f32 = 190.0;
const HEADER: f32 = 44.0;
const ROW: f32 = 20.0;
const PORT: f32 = 5.0;

#[derive(Debug, Clone)]
pub enum Event {
    Select(usize),
    Move(usize, Point),
    Connect { from: usize, to: usize, slot: usize },
    Disconnect { to: usize, slot: usize },
}

/// A node as drawn.
pub struct Card<'a> {
    pub id: &'a str,
    pub block: &'a str,
    pub at: Point,
    /// The block's input roles, each a port.
    pub roles: Vec<&'a str>,
    /// The card each port is wired to.
    pub sources: Vec<Option<usize>>,
    pub failed: bool,
    pub output: bool,
}

impl Card<'_> {
    fn size(&self) -> Size {
        let rows = self
            .roles
            .len()
            .max(1);
        Size::new(WIDTH, HEADER + rows as f32 * ROW + 6.0)
    }

    fn input(
        &self,
        slot: usize,
    ) -> Point {
        self.at + Vector::new(0.0, HEADER + (slot as f32 + 0.5) * ROW)
    }

    fn output(&self) -> Point {
        self.at + Vector::new(WIDTH, HEADER / 2.0)
    }
}

pub struct Graph<'a> {
    pub cards: Vec<Card<'a>>,
    pub selected: Option<usize>,
}

pub fn view(graph: Graph<'_>) -> Element<'_, Event> {
    canvas::Canvas::new(graph)
        .width(Fill)
        .height(Fill)
        .into()
}

/// Where a node is drawn, kept in its `ui`.
pub fn position(node: &Node) -> Option<Point> {
    let x = node
        .ui
        .get("x")?
        .as_f64()?;
    let y = node
        .ui
        .get("y")?
        .as_f64()?;
    Some(Point::new(x as f32, y as f32))
}

pub fn place(
    node: &mut Node,
    at: Point,
) {
    if !node.ui.is_object() {
        node.ui = json!({});
    }
    node.ui["x"] = Value::from(at.x.round());
    node.ui["y"] = Value::from(at.y.round());
}

/// A place for every node that has none: a column per step from the sources.
pub fn layout(pipeline: &mut Pipeline) {
    let mut depth: HashMap<String, usize> = HashMap::new();
    // as many passes as nodes settles every depth, cycles or not
    for _ in 0..pipeline.nodes.len() {
        for node in &pipeline.nodes {
            let below = node
                .inputs
                .iter()
                .filter_map(|input| depth.get(input))
                .max()
                .map_or(0, |d| d + 1);
            depth.insert(node.id.clone(), below);
        }
    }

    let mut next_y: HashMap<usize, f32> = HashMap::new();
    for node in &mut pipeline.nodes {
        if position(node).is_none() {
            let column = depth[&node.id];
            let y = next_y
                .entry(column)
                .or_insert(0.0);
            place(node, Point::new(column as f32 * (WIDTH + 70.0), *y));
            *y += HEADER + 2.0 * ROW + 40.0;
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    Pan { from: Point, offset: Vector },
    Card { index: usize, grab: Vector },
    Wire { from: usize, to: Point },
}

pub struct State {
    offset: Vector,
    zoom: f32,
    drag: Option<Drag>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            offset: Vector::new(30.0, 30.0),
            zoom: 1.0,
            drag: None,
        }
    }
}

impl State {
    /// The point of the graph under `at`, a point of the canvas.
    fn world(
        &self,
        at: Point,
    ) -> Point {
        Point::new(
            (at.x - self.offset.x) / self.zoom,
            (at.y - self.offset.y) / self.zoom,
        )
    }
}

impl Graph<'_> {
    fn card_at(
        &self,
        p: Point,
    ) -> Option<usize> {
        self.cards
            .iter()
            .rposition(|card| Rectangle::new(card.at, card.size()).contains(p))
    }

    fn output_at(
        &self,
        p: Point,
    ) -> Option<usize> {
        self.cards
            .iter()
            .rposition(|card| {
                card.output()
                    .distance(p)
                    <= 2.0 * PORT
            })
    }

    fn input_at(
        &self,
        p: Point,
    ) -> Option<(usize, usize)> {
        self.cards
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, card)| {
                (0..card.roles.len())
                    .find(|&slot| {
                        card.input(slot)
                            .distance(p)
                            <= 2.0 * PORT
                    })
                    .map(|slot| (index, slot))
            })
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
        let canvas::Event::Mouse(event) = event else {
            return None;
        };
        // where the cursor is on the canvas, inside it or not
        let at = Point::ORIGIN + (cursor.position()? - bounds.position());

        match event {
            mouse::Event::ButtonPressed(button) if cursor.is_over(bounds) => {
                let p = state.world(at);
                match button {
                    mouse::Button::Left => {
                        if let Some(from) = self.output_at(p) {
                            state.drag = Some(Drag::Wire { from, to: p });
                            return Some(canvas::Action::request_redraw().and_capture());
                        }
                        if let Some(index) = self.card_at(p) {
                            let grab = p - self.cards[index].at;
                            state.drag = Some(Drag::Card { index, grab });
                            return Some(
                                canvas::Action::publish(Event::Select(index)).and_capture(),
                            );
                        }
                        state.drag = Some(Drag::Pan {
                            from: at,
                            offset: state.offset,
                        });
                        Some(canvas::Action::capture())
                    }
                    mouse::Button::Right => {
                        let (to, slot) = self.input_at(p)?;
                        Some(canvas::Action::publish(Event::Disconnect { to, slot }).and_capture())
                    }
                    _ => None,
                }
            }
            mouse::Event::CursorMoved { .. } => match state.drag? {
                Drag::Pan { from, offset } => {
                    state.offset = offset + (at - from);
                    Some(canvas::Action::request_redraw())
                }
                Drag::Card { index, grab } => Some(canvas::Action::publish(Event::Move(
                    index,
                    state.world(at) - grab,
                ))),
                Drag::Wire { from, .. } => {
                    state.drag = Some(Drag::Wire {
                        from,
                        to: state.world(at),
                    });
                    Some(canvas::Action::request_redraw())
                }
            },
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                if let Drag::Wire { from, to } = state.drag.take()?
                    && let Some((to, slot)) = self.input_at(to)
                {
                    return Some(canvas::Action::publish(Event::Connect { from, to, slot }));
                }
                Some(canvas::Action::request_redraw())
            }
            mouse::Event::WheelScrolled { delta } if cursor.is_over(bounds) => {
                let lines = match delta {
                    ScrollDelta::Lines { y, .. } => *y,
                    ScrollDelta::Pixels { y, .. } => y / 50.0,
                };
                let before = state.world(at);
                state.zoom = (state.zoom * 1.1f32.powf(lines)).clamp(0.2, 3.0);
                // keep the point under the cursor where it is
                state.offset =
                    Vector::new(at.x - before.x * state.zoom, at.y - before.y * state.zoom);
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
    ) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(
            Point::ORIGIN,
            bounds.size(),
            palette
                .background
                .weak
                .color,
        );
        frame.translate(state.offset);
        frame.scale(state.zoom);

        let muted = palette
            .background
            .strongest
            .color;
        for card in &self.cards {
            for (slot, source) in card
                .sources
                .iter()
                .enumerate()
            {
                if let Some(source) = source {
                    wire(
                        &mut frame,
                        self.cards[*source].output(),
                        card.input(slot),
                        muted,
                    );
                }
            }
        }
        if let Some(Drag::Wire { from, to }) = state.drag {
            let color = palette
                .primary
                .base
                .color;
            wire(&mut frame, self.cards[from].output(), to, color);
        }

        for (index, card) in self
            .cards
            .iter()
            .enumerate()
        {
            let selected = self.selected == Some(index);
            let body = Path::rounded_rectangle(card.at, card.size(), 6.0.into());
            frame.fill(
                &body,
                palette
                    .background
                    .base
                    .color,
            );
            let (border, width) = match (selected, card.failed) {
                (true, _) => (
                    palette
                        .primary
                        .strong
                        .color,
                    2.5,
                ),
                (false, true) => (
                    palette
                        .danger
                        .base
                        .color,
                    1.5,
                ),
                (false, false) => (
                    palette
                        .background
                        .strong
                        .color,
                    1.0,
                ),
            };
            frame.stroke(
                &body,
                Stroke::default()
                    .with_width(width)
                    .with_color(border),
            );

            let ink = palette
                .background
                .base
                .text;
            label(
                &mut frame,
                card.id,
                card.at + Vector::new(10.0, 8.0),
                15.0,
                ink,
            );
            label(
                &mut frame,
                card.block,
                card.at + Vector::new(10.0, 26.0),
                12.0,
                muted,
            );

            for (slot, role) in card
                .roles
                .iter()
                .enumerate()
            {
                let port = card.input(slot);
                frame.fill(&Path::circle(port, PORT), muted);
                label(
                    &mut frame,
                    role,
                    port + Vector::new(10.0, -7.0),
                    12.0,
                    muted,
                );
            }
            let output = match card.output {
                true => {
                    palette
                        .success
                        .base
                        .color
                }
                false => muted,
            };
            frame.fill(&Path::circle(card.output(), PORT), output);
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag.is_some() {
            return mouse::Interaction::Grabbing;
        }
        let Some(at) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };
        let p = state.world(at);
        if self
            .output_at(p)
            .is_some()
            || self
                .input_at(p)
                .is_some()
        {
            mouse::Interaction::Crosshair
        } else if self
            .card_at(p)
            .is_some()
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::Grab
        }
    }
}

/// A wire from an output to an input, leaving and entering level.
fn wire(
    frame: &mut Frame,
    from: Point,
    to: Point,
    color: Color,
) {
    let bend = ((to.x - from.x).abs() / 2.0).max(40.0);
    let path = Path::new(|path| {
        path.move_to(from);
        path.bezier_curve_to(
            from + Vector::new(bend, 0.0),
            to - Vector::new(bend, 0.0),
            to,
        );
    });
    frame.stroke(
        &path,
        Stroke::default()
            .with_width(1.5)
            .with_color(color),
    );
}

fn label(
    frame: &mut Frame,
    content: &str,
    at: Point,
    size: f32,
    color: Color,
) {
    frame.fill_text(Text {
        content: content.to_string(),
        position: at,
        color,
        size: size.into(),
        max_width: WIDTH - 20.0,
        ..Text::default()
    });
}
