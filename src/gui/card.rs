//! The card of a node: its id, inputs and args, and what is known of it.

use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::{
    Component,
    button,
    canvas as draw,
    column,
    container,
    pick_list,
    row,
    space,
    stack,
    text,
    text_input,
};
use iced::{
    Center,
    Color,
    Fill,
    Length,
    Point,
    Rectangle,
    Renderer,
    Shrink,
    Size,
    Theme,
    Widget,
    border,
    mouse,
};
use serde_json::Value;

use super::table::{self, Table};
use super::{Editor, Message, error, form, layout};
use crate::polars::prelude::{DataFrame, DataType};
use crate::{Node, schema};

/// The space around what a card holds, and between its lines.
const PAD: f32 = 10.0;
const GAP: f32 = 6.0;
/// The heights of the lines above the args: the id, the block, and each input.
const HEAD: f32 = 28.0;
const BLOCK: f32 = 16.0;
const INPUT: f32 = 26.0;
/// The width of the handle that drags a card.
const GRAB: f32 = 14.0;
/// The width of a border that shows a status.
const STATUS: f32 = 4.0;

/// A change to a node.
#[derive(Debug, Clone)]
pub enum Edit {
    Rename(String),
    /// It reads the node of this id as its input of this slot.
    Wire(usize, String),
    /// Whether a Run saves its output.
    Output(bool),
    Args(Value),
    /// Its card is laid open, or closed if it is open.
    Toggle,
    Delete,
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

impl Editor {
    /// Applies `edit` to the node of this id, if there is one.
    pub(super) fn edit(
        &mut self,
        id: &str,
        edit: Edit,
    ) {
        let nodes = &mut self.pipeline.nodes;
        let Some(index) = nodes
            .iter()
            .position(|node| node.id == id)
        else {
            return;
        };
        match edit {
            Edit::Rename(new) => {
                // an empty or taken id would have its readers taken for another node's
                let is_taken = nodes
                    .iter()
                    .any(|node| node.id == new);
                if new.is_empty() || is_taken {
                    return;
                }
                self.forget(index);
                let nodes = &mut self.pipeline.nodes;
                nodes[index]
                    .id
                    .clone_from(&new);
                let readers = nodes
                    .iter_mut()
                    .flat_map(|node| &mut node.inputs)
                    .chain(
                        &mut self
                            .pipeline
                            .outputs,
                    );
                for reader in readers {
                    if *reader == id {
                        reader.clone_from(&new);
                    }
                }
            }
            Edit::Wire(slot, source) => {
                self.forget(index);
                let inputs = &mut self.pipeline.nodes[index].inputs;
                let len = inputs
                    .len()
                    .max(slot + 1);
                inputs.resize(len, String::new());
                inputs[slot] = source;
            }
            Edit::Output(is_output) => {
                let outputs = &mut self
                    .pipeline
                    .outputs;
                outputs.retain(|output| output != id);
                match is_output {
                    true => outputs.push(id.to_owned()),
                    // a frame that is no longer saved is let go
                    false => {
                        if let Some(known) = self
                            .known
                            .get_mut(id)
                        {
                            known.frame = None;
                        }
                    }
                }
            }
            Edit::Args(args) => {
                self.forget(index);
                self.pipeline.nodes[index].args = args;
            }
            Edit::Toggle => layout::toggle(&mut nodes[index]),
            Edit::Delete => {
                self.forget(index);
                let nodes = &mut self.pipeline.nodes;
                nodes.remove(index);
                self.pipeline
                    .outputs
                    .retain(|output| output != id);
                // its readers are unwired, lest a later node of its id be wired to them
                let inputs = nodes
                    .iter_mut()
                    .flat_map(|reader| &mut reader.inputs);
                for input in inputs {
                    if input == id {
                        input.clear();
                    }
                }
            }
        }
        self.is_dirty = true;
        self.check_wiring();
    }

    /// The card of `node`, with what is known of it.
    pub(super) fn card<'a>(
        &'a self,
        node: &'a Node,
    ) -> Card<'a> {
        // the columns its args may name are those of its inputs, once each
        let mut columns = Vec::new();
        let named = node
            .inputs
            .iter()
            .filter_map(|id| self.found(id)?.ok())
            .flatten();
        for (name, _) in named {
            if !columns.contains(name) {
                columns.push(name.clone());
            }
        }
        Card {
            node,
            defs: &self.catalog["$defs"],
            block: &self.catalog["blocks"][&node.block],
            others: self
                .pipeline
                .nodes
                .iter()
                .map(|other| other.id.as_str())
                .filter(|id| *id != node.id)
                .collect(),
            columns,
            is_output: self
                .pipeline
                .outputs
                .contains(&node.id),
            status: self.status(&node.id),
            found: self.found(&node.id),
            failed: self
                .known
                .get(&node.id)
                .and_then(|known| {
                    known
                        .failed
                        .as_deref()
                }),
            frame: self.frame(&node.id),
        }
    }
}

/// The part of a card that drags it: left of its toggle.
pub fn handle() -> Rectangle {
    Rectangle::new(Point::new(PAD, PAD), Size::new(GRAB, HEAD))
}

/// How far below the top of a card its input ports are, and its output port.
pub fn ports(inputs: usize) -> (impl Iterator<Item = f32>, f32) {
    let first = PAD + HEAD + GAP + BLOCK + GAP + INPUT / 2.0;
    let inputs = (0..inputs).map(move |slot| first + slot as f32 * (INPUT + GAP));
    (inputs, PAD + HEAD / 2.0)
}

/// A node as a card: closed it is its id and inputs, and open its args and table as well.
pub struct Card<'a> {
    node: &'a Node,
    defs: &'a Value,
    /// The entry of its block in the catalog.
    block: &'a Value,
    /// The ids of the other nodes, which it may read.
    others: Vec<&'a str>,
    columns: Vec<String>,
    is_output: bool,
    status: Status,
    found: Option<Result<&'a Vec<(String, DataType)>, &'a String>>,
    failed: Option<&'a str>,
    frame: Option<&'a DataFrame>,
}

/// What a card shows that its node does not hold.
#[derive(Default)]
pub struct State {
    form: form::State,
    /// The id as it is being typed, until Enter renames the node.
    name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Event {
    Name(String),
    Rename,
    Form(form::Message),
    Edit(Edit),
    Copy(String),
}

impl<'a> Component<'a, Message> for Card<'a> {
    type State = State;
    type Event = Event;

    fn update(
        &self,
        state: &mut State,
        event: Event,
        _renderer: &Renderer,
    ) -> Option<Message> {
        let edit = match event {
            Event::Name(name) => {
                state.name = Some(name);
                return None;
            }
            Event::Rename => Edit::Rename(state.name.take()?),
            Event::Form(message) => {
                let mut args = self
                    .node
                    .args
                    .clone();
                // the form may only show its value another way
                let is_changed = state
                    .form
                    .update(message, &mut args);
                if !is_changed {
                    return None;
                }
                Edit::Args(args)
            }
            Event::Edit(edit) => edit,
            Event::Copy(text) => return Some(Message::Copy(text)),
        };
        let id = self.node.id.clone();
        Some(Message::Edit(id, edit))
    }

    fn view(
        &self,
        state: &State,
    ) -> impl Widget<Event> + 'a {
        let node = self.node;
        let status = self.status;
        let is_open = layout::is_open(node);
        let size = layout::size(node);
        let schema = schema::resolve(self.defs, &self.block["args"]);

        // an open card is as wide as it was made, and a closed one as what it holds
        let (width, wide) = match is_open {
            true => (Length::Fixed(size.width), Fill),
            false => (Shrink, Shrink),
        };
        let name = state
            .name
            .clone()
            .unwrap_or_else(|| node.id.clone());
        let toggle = match is_open {
            true => "−",
            false => "+",
        };
        let save = match self.is_output {
            true => button::primary,
            false => button::text,
        };
        let icon = draw(Icon(self.is_output))
            .width(15)
            .height(11);
        // under the field, the id as text, which makes a closed card wide enough for it
        let sizer = text(name.clone())
            .size(14)
            .wrapping(text::Wrapping::None);
        let id = stack![
            container(sizer).padding([4, 6]),
            text_input("id", name)
                .on_input(Event::Name)
                .on_submit(Event::Rename)
                .size(14)
                .padding(4),
        ]
        .width(wide);
        let head = row![
            // where the surface draws the handle that drags the card
            space().width(GRAB),
            button(toggle)
                .style(button::text)
                .padding([0, 6])
                .on_press(Event::Edit(Edit::Toggle)),
            button(icon)
                .style(save)
                .padding(6)
                .on_press(Event::Edit(Edit::Output(!self.is_output))),
            id,
            button("×")
                .style(button::danger)
                .padding([0, 6])
                .on_press(Event::Edit(Edit::Delete)),
        ]
        .spacing(GAP)
        .align_y(Center)
        .height(HEAD);
        let block = text(&node.block)
            .size(12)
            .style(text::secondary);

        let others = self.others.clone();
        let inputs = self.block["inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(move |(slot, name)| {
                let source = node
                    .inputs
                    .get(slot)
                    .map(String::as_str)
                    .filter(|source| !source.is_empty());
                let wire = move |source: &str| Event::Edit(Edit::Wire(slot, source.to_owned()));
                row![
                    text(
                        name.as_str()
                            .unwrap_or_default()
                    )
                    .size(12),
                    is_open.then(|| {
                        pick_list(source, others.clone(), ToString::to_string)
                            .on_select(wire)
                            .placeholder("unwired")
                            .text_size(12)
                            .padding(4)
                            .width(Fill)
                    }),
                ]
                .spacing(GAP)
                .align_y(Center)
                .height(INPUT)
            });

        let open = is_open.then(|| {
            let description = schema["description"]
                .as_str()
                .map(|description| {
                    text(description.replace('\n', " "))
                        .size(12)
                        .style(text::secondary)
                });
            let columns = self.columns.clone();
            let form = form::view(&state.form, self.defs, schema, &node.args, columns);
            // a saved frame, or else the columns that a Check found
            let table = match (self.frame, self.found) {
                (Some(frame), _) => Some(draw(Table::of(frame)).height(size.height)),
                (None, Some(Ok(columns))) => {
                    Some(draw(Table::listing(columns)).height(table::HEADER))
                }
                _ => None,
            };
            column![
                description,
                form.map(Event::Form),
                table.map(|table| {
                    table
                        .width(Fill)
                        .map(Event::Copy)
                }),
            ]
            .spacing(GAP)
        });
        // the least a card is wide, which is where its errors wrap
        let least = match is_open {
            true => Fill,
            false => Length::Fixed(layout::WIDTH - 2.0 * PAD),
        };
        let wrong = match self.found {
            Some(Err(e)) => Some(error(e).width(least)),
            _ => None,
        };

        // the errors last: a line that comes and goes takes the state of all below it
        let lines = column![
            head,
            container(block)
                .width(least)
                .height(BLOCK),
            column(inputs.map(|input| input.boxed())).spacing(GAP),
            open,
            wrong,
            self.failed
                .map(|failed| error(failed).width(least)),
        ]
        .spacing(GAP);
        container(lines)
            .width(width)
            .padding(PAD)
            .style(move |theme| framed(theme, status))
    }
}

/// The paper of a card, in a border of the colour of its status.
fn framed(
    theme: &Theme,
    status: Status,
) -> container::Style {
    let palette = theme.palette();
    let paper = palette
        .background
        .base
        .color;
    // halfway to the card, for what a Check found and no Run
    let pale = |color: Color| color.mix(paper, 0.5);
    let success = palette
        .success
        .strong
        .color;
    let danger = palette
        .danger
        .strong
        .color;
    let unknown = palette
        .background
        .strongest
        .color;
    let (color, width) = match status {
        Status::Unknown => (unknown, 1.0),
        Status::Checked => (pale(success), STATUS),
        Status::Rejected => (pale(danger), STATUS),
        Status::Ran => (success, STATUS),
        Status::Failed => (danger, STATUS),
    };
    let border = border::rounded(5)
        .color(color)
        .width(width);
    container::Style::default()
        .background(paper)
        .border(border)
}

/// A small table: the button that has a Run save the frame of its node, lit when it does.
struct Icon(bool);

impl<Message> canvas::Program<Message> for Icon {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.palette();
        let color = match self.0 {
            true => {
                palette
                    .primary
                    .base
                    .text
            }
            false => {
                palette
                    .background
                    .base
                    .text
            }
        };
        let Size { width, height } = bounds.size();
        let lines = Path::new(|path| {
            path.rectangle(Point::new(0.5, 0.5), Size::new(width - 1.0, height - 1.0));
            path.move_to(Point::new(0.5, 4.0));
            path.line_to(Point::new(width - 0.5, 4.0));
            path.move_to(Point::new(5.0, 4.0));
            path.line_to(Point::new(5.0, height - 0.5));
        });
        let stroke = Stroke::default()
            .with_color(color)
            .with_width(1.0);
        let mut frame = Frame::new(renderer, bounds.size());
        frame.stroke(&lines, stroke);
        vec![frame.into_geometry()]
    }
}
