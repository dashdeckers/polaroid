//! The card of a node: its id, inputs and args, and what is known of it.

use std::rc::Rc;

use iced::advanced::widget;
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::{Component, canvas as draw, column, container, markdown, row, space, text};
use iced::{Center, Fill, Point, Rectangle, Renderer, Size, Theme, Widget, mouse};
use polaroid_iced::theme::{self, Scheme, Tone, control, font, icon, size, spacing, stroke};
use polaroid_iced::{Asked, Heading, leave, width_of};
use serde_json::Value;

use super::form::{Expressions, Form};
use super::gallery::{self, Gallery};
use super::known::{Known, Status};
use super::look::{self, Help, cross, error, name_field, round, toggle};
use super::{Editor, Message, drawer, form, layout};
use crate::polars::prelude::{AnyValue, DataFrame, DataType};
use crate::{Described, Node, Pipeline, Registry, schema};

/// The space around what a card holds, and between its lines.
const PAD: f32 = spacing::M;
const GAP: f32 = spacing::S;
/// The height of the line of the id.
const HEAD: f32 = theme::line(size::TITLE) + theme::FIELD.top + theme::FIELD.bottom;
/// The small table, a pixel under a sign so that its lines are crisp.
const ICON: Size = Size::new(icon::M - 1.0, icon::S - 1.0);
/// What the border and the cursor of a field take beside the room around its text.
const EDGE: f32 = stroke::MEDIUM;
/// The width of the handle that drags a card.
const GRAB: f32 = theme::surface::GRIP;

/// The width of a card whose line of the id shows all of an id that is this wide.
fn around(id: f32) -> f32 {
    let field = id + theme::FIELD.x() + 2.0 * EDGE;
    // in the card's padding: the handle, three buttons and the field, with a gap between each two
    2.0 * PAD + GRAB + 3.0 * control::S + field + 4.0 * GAP
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
                if let Some(shown) = self
                    .shown
                    .remove(id)
                {
                    self.shown
                        .insert(new.clone(), shown);
                }
                if let Some(draft) = self
                    .drafts
                    .remove(id)
                {
                    self.drafts
                        .insert(new.clone(), draft);
                }
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
                    false => self
                        .known
                        .unsave(id),
                }
            }
            Edit::Args(args) => {
                // no draft of them is left, and the same again are no edit
                self.drafts
                    .remove(id);
                if self.pipeline.nodes[index].args == args {
                    return;
                }
                self.forget(|node| node.id == id);
                self.pipeline.nodes[index].args = args;
            }
            Edit::Toggle => layout::toggle(&mut nodes[index]),
            Edit::Delete => {
                self.shown
                    .remove(id);
                self.drafts
                    .remove(id);
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
            pipeline: &self.pipeline,
            registry: &self.registry,
            known: &self.known,
            draft: self
                .drafts
                .get(&node.id),
            is_output: self
                .pipeline
                .outputs
                .contains(&node.id),
            status: self
                .known
                .status(&node.id),
            found: self
                .known
                .columns(&node.id),
            columns: self
                .known
                .names(&node.inputs),
            failed: self
                .known
                .failure(&node.id),
            frame: self
                .known
                .frame(&node.id),
            gallery: self
                .known
                .gallery(&node.id),
            shown: self
                .shown
                .get(&node.id)
                .unwrap_or(&gallery::UNTOUCHED),
            doc: self
                .docs
                .get(&node.block),
            theme: &self.theme,
            help: self.help,
            zoom: self.zoom,
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
    /// The pipeline, of which it may read the other nodes and name the expressions and values.
    pipeline: &'a Pipeline,
    /// What say whether its block and a Check take args.
    registry: &'a Registry,
    known: &'a Known,
    /// Its args as they are edited, if those are not taken yet.
    draft: Option<&'a form::Draft>,
    is_output: bool,
    status: Status,
    found: Option<Result<&'a Vec<(String, DataType)>, &'a String>>,
    /// The columns of the frames it reads that a Check found, which its args may name.
    columns: Vec<String>,
    failed: Option<&'a str>,
    frame: Option<&'a DataFrame>,
    /// The images in its frame, if a Run saved it.
    gallery: Option<&'a Gallery>,
    /// Which of those images is shown.
    shown: &'a gallery::State,
    /// What its block does, as Markdown, and the theme to lay that out in.
    doc: Option<&'a markdown::Content>,
    theme: &'a Theme,
    help: Help,
    /// How many times their size the cards are drawn.
    zoom: f32,
}

/// What a card shows that its node does not hold.
#[derive(Default)]
pub struct State {
    form: form::State,
    /// The id as it is being typed, until its field is left.
    name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Event {
    Name(String),
    Rename,
    /// The field of the id lost the focus.
    Leave,
    Form(form::Message),
    Gallery(gallery::Message),
    Edit(Edit),
    Table(Asked),
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
            // one that is refused stays typed
            Event::Rename => Edit::Rename(state.name.clone()?),
            Event::Leave => {
                state.name = None;
                return None;
            }
            Event::Gallery(message) => {
                return Some(Message::Gallery(self.node.id.clone(), message));
            }
            // the image of a row that is pressed is the one to show
            Event::Table(Asked::Row(row)) => {
                self.gallery?;
                let show = gallery::Message::Show(row);
                return Some(Message::Gallery(self.node.id.clone(), show));
            }
            Event::Form(message) => {
                let check = self.check();
                let change = state
                    .form
                    .update(message, &self.node.args, self.draft, &*check);
                let id = self.node.id.clone();
                return match change {
                    form::Change::Nothing => None,
                    form::Change::Value(args) => Some(Message::Edit(id, Edit::Args(args))),
                    // until its block and a Check take them, nothing runs and what it showed stays
                    form::Change::Draft(draft) => Some(Message::Draft(id, draft)),
                    // what is named is edited where it is named, for every node that names it
                    form::Change::Named(name, at, value) => {
                        let put = drawer::Message::Put(name, at, value);
                        Some(Message::Drawer(put))
                    }
                };
            }
            Event::Edit(edit) => edit,
            Event::Table(Asked::Copy(text)) => return Some(Message::Copy(text)),
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

        let typed = state
            .name
            .as_deref();
        let name = typed
            .unwrap_or(&node.id)
            .to_owned();
        // as wide as it was made, and as the line of its id with all of the id in view
        let made = match is_open {
            true => layout::size(node).width,
            false => layout::WIDTH,
        };
        let width = made.max(around(width_of(&name, size::TITLE, font::SANS)));
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
        let label_width = look::label_width(reads.chain(args));

        let head = self.head(typed, is_open);
        let inputs = self.inputs(label_width, is_open);
        let wrong = match self.found {
            Some(Err(e)) => Some(error(e)),
            _ => None,
        };
        let failed = self
            .failed
            .map(error);
        // the errors last: a line that comes and goes takes the state of all below it
        let lines = match is_open {
            true => {
                // what the block does, laid out as the Markdown it is written in
                let does = self.doc.map(|doc| {
                    let settings = markdown::Settings {
                        font: font::SANS,
                        inline_code_font: font::MONO,
                        code_block_font: font::MONO,
                        ..markdown::Settings::with_text_size(size::NOTE)
                    };
                    markdown::view(doc.items(), settings, self.theme.clone()).map(|_| Event::Link)
                });
                // an expression is picked by its name, and edited in the drawer
                let (value, held) = form::Draft::shown(self.draft, &node.args);
                let form = Form {
                    state: &state.form,
                    defs: self.defs,
                    pipeline: self.pipeline,
                    offers: &form::Offers::default(),
                    columns: &self.columns,
                    expressions: Expressions::Named,
                    check: &self.check(),
                    value,
                    held,
                    label_width,
                    help: self.help,
                }
                .view(schema)
                .map(Event::Form);
                column![
                    head,
                    does,
                    inputs,
                    form,
                    self.gallery(width, true),
                    self.table(),
                    wrong,
                    failed
                ]
            }
            false => {
                // a closed card says no more of its block than its name
                let block = text(&node.block)
                    .size(size::NOTE)
                    .font(font::MONO)
                    .style(theme::text::muted);
                // and of its images one, small
                column![
                    head,
                    inputs,
                    block,
                    self.gallery(width, false),
                    wrong,
                    failed
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
    /// What says whether args are taken: by its block, and by a Check of it on what it reads.
    pub(super) fn check(&self) -> form::Check<'a> {
        let (registry, pipeline, known, node) =
            (self.registry, self.pipeline, self.known, self.node);
        Rc::new(move |args| {
            registry
                .resolved(&node.block, args, pipeline)
                .map_err(|e| form::line(&e.why))?;
            known
                .refuses(registry, pipeline, node, args)
                .map_or(Ok(()), Err)
        })
    }

    /// The names of the frames that its block reads, in order.
    fn reads(&self) -> &'static [&'static str] {
        self.block
            .map_or(&[], |block| block.inputs)
    }

    /// The line of its id: room for the handle, its toggle, the small table, the id and the cross.
    fn head(
        &self,
        typed: Option<&str>,
        is_open: bool,
    ) -> impl Widget<Event> + use<'a> {
        let save = match self.is_output {
            true => theme::button::filled,
            false => theme::button::icon,
        };
        let icon = draw(Icon(self.is_output))
            .width(ICON.width)
            .height(ICON.height);
        row![
            // where the surface draws the handle that drags the card
            space().width(GRAB),
            self.help.of(
                toggle(is_open, Event::Edit(Edit::Toggle)),
                "Lays the card open to edit its args and see its table, or closes it. A \
                     double-click on a closed card opens it too. Drag the corner of an open \
                     card to resize it.",
            ),
            self.help.of(
                round(icon)
                    .style(save)
                    .on_press(Event::Edit(Edit::Output(!self.is_output))),
                "Keeps the frame of this node at the next Run, to show it on the open \
                     card. Scroll over the table, with Shift for sideways; drag across cells \
                     and press Ctrl+C to copy them.",
            ),
            self.help.of(
                leave(
                    name_field("id", typed, &self.node.id, |id| {
                        self.pipeline
                            .nodes
                            .iter()
                            .any(|node| node.id == id)
                    })
                    .on_input(Event::Name)
                    .on_submit(Event::Rename),
                    Event::Leave,
                ),
                "The id of the node. Type another and press Enter to rename it; the border \
                     is orange until then, and red while the id is empty or another node's. \
                     Leaving the field shows the id again.",
            ),
            self.help.of(
                cross(Event::Edit(Edit::Delete)),
                "Deletes the node, and unwires those that read it.",
            ),
        ]
        .spacing(GAP)
        .align_y(Center)
        .height(HEAD)
    }

    /// A line for each frame it reads: the name of the input, and in an open card the node read.
    fn inputs(
        &self,
        label_width: f32,
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
                    None => Tone::Refused,
                };
                let wired = is_open.then(|| {
                    let others: Vec<&str> = self
                        .pipeline
                        .nodes
                        .iter()
                        .map(|other| other.id.as_str())
                        .filter(|id| *id != node.id)
                        .collect();
                    let unwired = "unwired";
                    look::pick(
                        source,
                        others,
                        ToString::to_string,
                        unwired,
                        font::SANS,
                        tone,
                    )
                    .on_select(wire)
                    .width(Fill)
                });
                container(look::labelled(name, label_width, None, wired))
                    .id(anchor(slot))
                    .boxed()
            });
        column(inputs).spacing(spacing::XS)
    }

    /// An image of its frame; to browse, with what picks the rows and steps through them.
    fn gallery(
        &self,
        width: f32,
        is_browsed: bool,
    ) -> Option<impl Widget<Event> + use<'a>> {
        let gallery = self
            .gallery?
            // within the padding of the card
            .view(
                self.shown,
                width - 2.0 * PAD,
                is_browsed,
                self.zoom,
                self.help,
            )?;
        Some(gallery.map(Event::Gallery))
    }

    /// A saved frame, or else the columns that a Check found, to select cells in and copy them.
    fn table(&self) -> Option<impl Widget<Event> + use<'a>> {
        let heading = |name: &'a str, dtype: &DataType| Heading {
            name,
            kind: dtype.to_string(),
            is_numeric: dtype.is_numeric(),
        };
        let table = match (self.frame, self.found) {
            (Some(frame), _) => {
                let columns = frame.columns();
                let headings = columns
                    .iter()
                    .map(|column| {
                        let name = column
                            .name()
                            .as_str();
                        heading(name, column.dtype())
                    })
                    .collect();
                let cell = move |row: usize, column: usize| {
                    columns[column]
                        .get(row)
                        .map(shown)
                        .unwrap_or_default()
                };
                polaroid_iced::table(headings).rows(frame.height(), cell)
            }
            (None, Some(Ok(columns))) => {
                let headings = columns
                    .iter()
                    .map(|(name, dtype)| heading(name, dtype))
                    .collect();
                polaroid_iced::table(headings)
            }
            _ => return None,
        };
        // it takes what the card has left over, and its first rows at the least
        let height = Fill.min(table.least());
        let table = draw(table)
            .width(Fill)
            .height(height)
            .map(Event::Table);
        Some(table)
    }
}

/// The text of a cell that holds `value`: what is no text is told by its size.
pub(super) fn shown(value: AnyValue) -> String {
    match value {
        AnyValue::Binary(bytes) => format!("{} bytes", bytes.len()),
        AnyValue::BinaryOwned(bytes) => format!("{} bytes", bytes.len()),
        value => value
            .str_value()
            .into_owned(),
    }
}

/// A card in a border of the colour of its status, once anything is known of it.
fn framed(
    theme: &Theme,
    status: Status,
) -> container::Style {
    let card = theme::container::card(theme);
    let scheme = Scheme::of(theme);
    let (success, error) = (scheme.success.color, scheme.error.color);
    // paler for what a Check found and no Run
    let color = match status {
        Status::Unknown => return card,
        Status::Checked => scheme.pale(success),
        Status::Rejected => scheme.pale(error),
        Status::Ran => success,
        Status::Failed => error,
    };
    let border = card
        .border
        .color(color)
        .width(stroke::THICK);
    card.border(border)
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
        let scheme = Scheme::of(theme);
        // as a sign is in the button it is in, filled or not
        let color = match self.0 {
            true => scheme.primary.on,
            false => scheme.on_surface_variant,
        };
        let Size { width, height } = bounds.size();
        let lines = Path::new(|path| {
            path.rectangle(Point::new(0.5, 0.5), Size::new(width - 1.0, height - 1.0));
            path.move_to(Point::new(0.5, 4.0));
            path.line_to(Point::new(width - 0.5, 4.0));
            path.move_to(Point::new(5.0, 4.0));
            path.line_to(Point::new(5.0, height - 0.5));
        });
        let line = Stroke::default()
            .with_color(color)
            .with_width(stroke::THIN);
        let mut frame = Frame::new(renderer, bounds.size());
        frame.stroke(&lines, line);
        vec![frame.into_geometry()]
    }
}
