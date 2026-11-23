//! The card of a node: its id, inputs and args, and what is known of it.

use iced::advanced::widget;
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::{
    Component,
    button,
    canvas as draw,
    column,
    container,
    markdown,
    pick_list,
    row,
    space,
    text,
    text_input,
};
use iced::{
    Center,
    Color,
    Fill,
    Font,
    Point,
    Rectangle,
    Renderer,
    Size,
    Theme,
    Widget,
    border,
    mouse,
};
use polaroid_iced::width_of;
use serde_json::{Map, Value};

use super::form::{Expressions, Tone};
use super::table::Table;
use super::{Editor, Message, NOTE, TEXT, TITLE, cross, error, form, layout, toggle};
use crate::polars::prelude::{DataFrame, DataType};
use crate::{Described, Node, schema};

/// The space around what a card holds, and between its lines.
const PAD: f32 = 10.0;
const GAP: f32 = 6.0;
/// The height of the line of the id.
const HEAD: f32 = 28.0;
/// The width of the buttons that are a sign: the toggle and the cross.
const BUTTON: f32 = 22.0;
/// The small table, and the room around it in its button.
const ICON: Size = Size::new(15.0, 11.0);
const AROUND_ICON: f32 = 6.0;
/// The room around the text of a field, and what its border and cursor take beside that.
const FIELD: f32 = 4.0;
const EDGE: f32 = 2.0;
/// The width of the handle that drags a card.
const GRAB: f32 = 14.0;
/// The width of a border that shows a status.
const STATUS: f32 = 4.0;

/// The width of a card whose line of the id shows all of an id that is this wide.
fn around(id: f32) -> f32 {
    let buttons = 2.0 * BUTTON + ICON.width + 2.0 * AROUND_ICON;
    let field = id + 2.0 * (FIELD + EDGE);
    // in the card's padding: the handle, the buttons and the field, with a gap between each two
    2.0 * PAD + GRAB + buttons + field + 4.0 * GAP
}

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
                let is_renamed = self
                    .pipeline
                    .rename(id, &new);
                if !is_renamed {
                    return;
                }
                // what was known of it was known by its old id
                self.known
                    .remove(id);
                self.forget(|node| node.id == new);
            }
            Edit::Wire(slot, source) => {
                self.forget(|node| node.id == id);
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
                self.forget(|node| node.id == id);
                self.pipeline.nodes[index].args = args;
            }
            Edit::Toggle => layout::toggle(&mut nodes[index]),
            Edit::Delete => {
                self.forget(|node| node.id == id);
                self.pipeline
                    .remove(id);
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
        Card {
            node,
            defs: &self.catalog.defs,
            block: self.block(&node.block),
            nodes: &self.pipeline.nodes,
            expressions: &self
                .pipeline
                .expressions,
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
            doc: self
                .docs
                .get(&node.block),
            theme: &self.theme,
        }
    }
}

/// The part of a card that drags it: left of its toggle.
pub fn handle() -> Rectangle {
    Rectangle::new(Point::new(PAD, PAD), Size::new(GRAB, HEAD))
}

/// The id of the line of a card that the port of its input `slot` is level with.
pub fn anchor(slot: usize) -> widget::Id {
    widget::Id::from(format!("input {slot}"))
}

/// How far below the top of a card its output port is: level with its id.
pub fn output() -> f32 {
    PAD + HEAD / 2.0
}

/// A node as a card: closed it is its id and inputs, and open its args and table as well.
pub struct Card<'a> {
    node: &'a Node,
    defs: &'a Value,
    /// What the catalog says of its block, if there is such a block.
    block: Option<&'a Described>,
    /// Every node of the pipeline, of which it may read the others.
    nodes: &'a [Node],
    /// The expressions by name, which its args pick from.
    expressions: &'a Map<String, Value>,
    is_output: bool,
    status: Status,
    found: Option<Result<&'a Vec<(String, DataType)>, &'a String>>,
    failed: Option<&'a str>,
    frame: Option<&'a DataFrame>,
    /// What its block does, as Markdown, and the theme to lay that out in.
    doc: Option<&'a markdown::Content>,
    theme: &'a Theme,
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
    /// A link in what a block does was pressed, which leads nowhere.
    Link,
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
            Event::Link => return None,
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
        let args = self
            .block
            .map_or(&Value::Null, |block| &block.args);
        let schema = schema::resolve(self.defs, args);

        let name = state
            .name
            .clone()
            .unwrap_or_else(|| node.id.clone());
        // as wide as it was made, and as the line of its id with all of the id in view
        let made = match is_open {
            true => layout::size(node).width,
            false => layout::WIDTH,
        };
        let width = made.max(around(width_of(&name, TITLE, Font::DEFAULT)));
        // the names of all it is given in one column: the nodes it reads, then its args
        let args = schema["properties"]
            .as_object()
            .into_iter()
            .flat_map(|args| args.keys())
            .map(String::as_str);
        let reads = self
            .reads()
            .iter()
            .copied();
        let names = form::names(reads.chain(args));

        let head = self.head(name, is_open);
        let inputs = self.inputs(names, is_open);
        let wrong = match self.found {
            Some(Err(e)) => Some(error(e).width(Fill)),
            _ => None,
        };
        let failed = self
            .failed
            .map(|failed| error(failed).width(Fill));
        // the errors last: a line that comes and goes takes the state of all below it
        let lines = match is_open {
            true => {
                // what the block does, laid out as the Markdown it is written in
                let does = self.doc.map(|doc| {
                    let settings = markdown::Settings::with_text_size(NOTE);
                    markdown::view(doc.items(), settings, self.theme.clone()).map(|_| Event::Link)
                });
                // an expression is picked by its name, and edited in the drawer
                let named = Expressions::Named(self.expressions);
                let form = form::view(&state.form, self.defs, schema, &node.args, named, names)
                    .map(Event::Form);
                column![
                    head,
                    does,
                    inputs,
                    form,
                    self.table(),
                    wrong,
                    failed
                ]
            }
            false => {
                // a closed card says no more of its block than its name
                let block = text(&node.block)
                    .size(NOTE)
                    .font(Font::MONOSPACE)
                    .style(text::secondary);
                column![
                    head, inputs, block, wrong, failed
                ]
            }
        };
        // as tall as what it holds, or as the surface makes it, where the table takes the rest
        container(
            lines
                .spacing(GAP)
                .height(Fill),
        )
        .width(width)
        .height(Fill)
        .padding(PAD)
        .style(move |theme| framed(theme, status))
    }
}

impl<'a> Card<'a> {
    /// The names of the frames that its block reads, in order.
    fn reads(&self) -> &'static [&'static str] {
        self.block
            .map_or(&[], |block| block.inputs)
    }

    /// The line of its id: room for the handle, its toggle, the small table, the id and the cross.
    fn head(
        &self,
        name: String,
        is_open: bool,
    ) -> impl Widget<Event> + use<'a> {
        let save = match self.is_output {
            true => button::primary,
            false => button::text,
        };
        let icon = draw(Icon(self.is_output))
            .width(ICON.width)
            .height(ICON.height);
        row![
            // where the surface draws the handle that drags the card
            space().width(GRAB),
            toggle(is_open, Event::Edit(Edit::Toggle)).width(BUTTON),
            button(icon)
                .style(save)
                .padding(AROUND_ICON)
                .on_press(Event::Edit(Edit::Output(!self.is_output))),
            text_input("id", name)
                .on_input(Event::Name)
                .on_submit(Event::Rename)
                .size(TITLE)
                .padding(FIELD),
            cross(Event::Edit(Edit::Delete)).width(BUTTON),
        ]
        .spacing(GAP)
        .align_y(Center)
        .height(HEAD)
    }

    /// A line for each frame it reads: the name of the input, and in an open card the node read.
    fn inputs(
        &self,
        names: f32,
        is_open: bool,
    ) -> impl Widget<Event> + use<'a> {
        let node = self.node;
        let inputs = self
            .reads()
            .iter()
            .enumerate()
            .map(|(slot, name)| {
                let source = node
                    .inputs
                    .get(slot)
                    .map(String::as_str)
                    .filter(|source| !source.is_empty());
                let wire = move |source: &str| Event::Edit(Edit::Wire(slot, source.to_owned()));
                // a node to read is as much an argument as any, and without one it is missing
                let tone = match source {
                    Some(_) => Tone::Given,
                    None => Tone::Missing,
                };
                let wired = is_open.then(|| {
                    let others: Vec<&str> = self
                        .nodes
                        .iter()
                        .map(|other| other.id.as_str())
                        .filter(|id| *id != node.id)
                        .collect();
                    pick_list(source, others, ToString::to_string)
                        .on_select(wire)
                        .placeholder("unwired")
                        .text_size(TEXT)
                        .padding(FIELD)
                        .width(Fill)
                        .style(form::picked(tone))
                });
                container(form::labelled(name, names, None, wired))
                    .id(anchor(slot))
                    .boxed()
            });
        column(inputs).spacing(5)
    }

    /// A saved frame, or else the columns that a Check found, to select cells in and copy them.
    fn table(&self) -> Option<impl Widget<Event> + use<'a>> {
        let table = match (self.frame, self.found) {
            (Some(frame), _) => Table::of(frame),
            (None, Some(Ok(columns))) => Table::listing(columns),
            _ => return None,
        };
        // it takes what the card has left over, and its first rows at the least
        let height = Fill.min(table.least());
        let table = draw(table)
            .width(Fill)
            .height(height)
            .map(Event::Copy);
        Some(table)
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
