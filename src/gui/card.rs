//! The card of a node: closed its tile, and open its id, inputs and args, and what is known of it.

use std::rc::Rc;

use iced::advanced::widget;
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::{Component, canvas as draw, column, container, markdown, row, space, svg, text};
use iced::{Center, Fill, Point, Rectangle, Renderer, Size, Theme, Widget, mouse};
use polaroid_iced::theme::surface::Ring;
use polaroid_iced::theme::{self, Scheme, Tone, control, font, icon, size, spacing, stroke};
use polaroid_iced::{Asked, Heading, width_of};
use serde_json::Value;

use super::form::{Expressions, Form};
use super::gallery::{self, Gallery};
use super::icon::Icons;
use super::known::{Known, Status};
use super::look::{self, Help, cross, error, round, toggle};
use super::{Editor, Message, drawer, form, layout};
use crate::polars::prelude::{AnyValue, DataFrame, DataType};
use crate::{Described, Node, Pipeline, Registry, schema};

/// The space around what a card holds, and between its lines.
const PAD: f32 = spacing::M;
const GAP: f32 = spacing::S;
/// The side of a tile, which a closed card is.
const TILE: f32 = theme::surface::TILE;
/// The height of the line of the id, and the side of the plate that an open card is dragged by.
const HEAD: f32 = theme::HEAD;
/// The small table, a pixel under a sign so that its lines are crisp.
const ICON: Size = Size::new(icon::M - 1.0, icon::S - 1.0);
/// What the border and the cursor of a field take beside the room around its text.
const EDGE: f32 = stroke::MEDIUM;

/// The width of a card whose line of the id shows all of an id that is this wide.
fn around(id: f32) -> f32 {
    let field = id + theme::FIELD.x() + 2.0 * EDGE;
    // in the card's padding: the plate, three buttons and the field, with a gap between each two
    2.0 * PAD + HEAD + 3.0 * control::S + field + 4.0 * GAP
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
        // a card that the cursor opened is open or closed for good, or gone
        if let Edit::Toggle | Edit::Delete = edit {
            self.peek = None;
        }
        match edit {
            Edit::Rename(new) => {
                let is_renamed = self
                    .pipeline
                    .rename(id, &new);
                if !is_renamed {
                    return;
                }
                // its card stays open for the cursor that rests on it
                if self.peek.as_deref() == Some(id) {
                    self.peek = Some(new.clone());
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

    /// Whether the card of `node` is shown open: kept open, or for the cursor that rests on it.
    pub(super) fn shows_open(
        &self,
        node: &Node,
    ) -> bool {
        layout::is_open(node) || self.peek.as_ref() == Some(&node.id)
    }

    /// The card of `node`, with what is known of it.
    pub(super) fn card<'a>(
        &'a self,
        node: &'a Node,
    ) -> Card<'a> {
        Card {
            node,
            is_open: self.shows_open(node),
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
            icons: &self.icons,
            theme: &self.theme,
            help: self.help,
            zoom: self.zoom,
        }
    }
}

/// The part of a card that drags it, and opens or closes it at a double press: its plate.
pub fn handle(is_open: bool) -> Rectangle {
    match is_open {
        true => Rectangle::new(Point::new(PAD, PAD), Size::new(HEAD, HEAD)),
        false => Rectangle::with_size(Size::new(TILE, TILE)),
    }
}

/// The id of the line of a card that the port of its input `slot` is level with.
pub fn anchor(slot: usize) -> widget::Id {
    widget::Id::from(format!("input {slot}"))
}

/// How far below the top of a card its output port is: level with its id, or halfway down its tile.
pub fn output(is_open: bool) -> f32 {
    match is_open {
        true => PAD + HEAD / 2.0,
        false => TILE / 2.0,
    }
}

/// A node as a card: closed it is its tile, and open its id, its args and what it gives.
pub struct Card<'a> {
    node: &'a Node,
    /// Whether it is shown open, and not as its tile alone.
    is_open: bool,
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
    /// The icon of each block, of which its own is on its plate.
    icons: &'a Icons,
    theme: &'a Theme,
    help: Help,
    /// How many times their size the cards are drawn.
    zoom: f32,
}

#[derive(Debug, Clone)]
pub enum Event {
    Rename(String),
    Form(form::Message),
    Gallery(gallery::Message),
    Edit(Edit),
    Table(Asked),
    /// A link in what a block does was pressed, which leads nowhere.
    Link,
}

impl<'a> Component<'a, Message> for Card<'a> {
    type State = form::State;
    type Event = Event;

    fn update(
        &self,
        state: &mut form::State,
        event: Event,
        _renderer: &Renderer,
    ) -> Option<Message> {
        let edit = match event {
            Event::Rename(id) => Edit::Rename(id),
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
                let change = state.update(message, &self.node.args, self.draft, &*check);
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
        state: &form::State,
    ) -> impl Widget<Event> + 'a {
        let node = self.node;
        let style = theme::container::card;
        if !self.is_open {
            // a closed card is its tile alone, with a port down its edge for each frame it reads
            let ports = (0..self.reads().len()).map(|slot| {
                container(space())
                    .id(anchor(slot))
                    .height(Fill)
                    .boxed()
            });
            let ports = column(ports).height(TILE);
            return container(row![ports, self.tile()].boxed()).style(style);
        }
        let args = self
            .block
            .map_or(&Value::Null, |block| &block.args);
        let schema = schema::resolve(self.defs, args);

        // as wide as it was made, and as the line of its id with all of the id in view
        let width = layout::size(node)
            .width
            .max(around(width_of(&node.id, size::TITLE, font::SANS)));
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
            state,
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
        // the errors last: a line that comes and goes takes the state of all below it
        let lines = column![
            self.head(),
            does,
            self.inputs(label_width),
            form,
            self.gallery(width),
            self.table(),
            self.wrong()
                .map(error),
        ];
        // as tall as what it holds, or as the surface makes it, where the table takes the rest
        container(
            lines
                .spacing(GAP)
                .height(Fill)
                .boxed(),
        )
        .width(width)
        .height(Fill)
        .padding(PAD)
        .style(style)
    }
}

impl<'a> Card<'a> {
    /// What says whether args are taken: by its block, and by a Check of it on what it reads.
    pub(super) fn check(&self) -> form::Check<'a> {
        let (registry, pipeline, known, node) =
            (self.registry, self.pipeline, self.known, self.node);
        Rc::new(move |args| {
            registry.resolved(&node.block, args, pipeline)?;
            known
                .refuses(registry, pipeline, node, args)
                .map_or(Ok(()), |why| Err(why.into()))
        })
    }

    /// The names of the frames that its block reads, in order.
    fn reads(&self) -> &'static [&'static str] {
        self.block
            .map_or(&[], |block| block.inputs)
    }

    /// Why a Check rejected it or a Run of it failed, if either did.
    fn wrong(&self) -> Option<&'a str> {
        match self.found {
            Some(Err(e)) => Some(e.as_str()),
            _ => self.failed,
        }
    }

    /// The icon of its block, this large.
    fn icon(
        &self,
        side: f32,
    ) -> Option<impl Widget<Event> + use<'a>> {
        let icon = self
            .icons
            .of(&self.node.block)?;
        let icon = svg(icon)
            .width(side)
            .height(side);
        Some(icon)
    }

    /// The button that has a Run save the frame of its node, lit when it does.
    fn save(&self) -> impl Widget<Event> + use<'a> {
        let style = match self.is_output {
            true => theme::button::filled,
            false => theme::button::icon,
        };
        let table = draw(Icon(self.is_output))
            .width(ICON.width)
            .height(ICON.height);
        self.help.of(
            round(table)
                .style(style)
                .on_press(Event::Edit(Edit::Output(!self.is_output))),
            "Keeps the frame of this node at the next Run, to show it on the open card. Scroll \
             over the table, with Shift for sideways; drag across cells and press Ctrl+C to copy \
             them.",
        )
    }

    /// Its tile: a plate with the icon of its block or the image it drew, over its block and id.
    fn tile(&self) -> impl Widget<Event> + use<'a> {
        let node = self.node;
        // an image that a Run kept is shown in the place of the icon, as a plot that was drawn
        let drawn = self
            .gallery
            .and_then(|gallery| gallery.thumbnail(self.shown));
        let shown = match drawn {
            Some(drawn) => drawn,
            None => self
                .icon(icon::XL)
                .boxed(),
        };
        // the plate takes what the lines under it leave of the tile
        let plate = self.help.of(
            container(shown)
                .padding(spacing::XS)
                .center(Fill)
                .style(theme::container::section),
            "The tile of the node: the icon of its block, or the image it drew once a Run kept \
             that, over the block and the id. Drag it to move the card, and double-click it to \
             open the card.",
        );
        let line = |written: &'a str| {
            text(written)
                .width(Fill)
                .wrapping(text::Wrapping::None)
        };
        let block = line(&node.block)
            .size(size::NOTE)
            .font(font::MONO)
            .style(theme::text::muted)
            .ellipsis(text::Ellipsis::End);
        // an id is told from the next by how it ends as much as by how it starts
        let id = line(&node.id).ellipsis(text::Ellipsis::Middle);
        let lines = column![
            plate,
            row![block, self.save()].align_y(Center),
            id
        ]
        .spacing(spacing::XXS);
        let tile = container(lines)
            .padding(PAD)
            .width(TILE)
            .height(TILE);
        // what is wrong with it is said where the open card would show it
        look::told(
            tile,
            self.wrong()
                .map(str::to_owned),
        )
    }

    /// The line of its id: its plate, its toggle, the small table, the id and the cross.
    fn head(&self) -> impl Widget<Event> + use<'a> {
        let nodes = &self.pipeline.nodes;
        let plate = self.help.of(
            container(self.icon(icon::L))
                .center(HEAD)
                .style(theme::container::section),
            "The icon of the block of the node. Drag it to move the card, and double-click it to \
             close the card to its tile.",
        );
        row![
            plate,
            self.help.of(
                toggle(layout::is_open(self.node), Event::Edit(Edit::Toggle)),
                "Closes the card to its tile, or keeps it open if the cursor resting on its tile \
                 opened it; a double-click on the icon does so too. Drag the corner of an open \
                 card to resize it.",
            ),
            self.save(),
            self.help.of(
                look::name(
                    "id",
                    &self.node.id,
                    move |id| {
                        nodes
                            .iter()
                            .any(|node| node.id == id)
                    },
                    Event::Rename,
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

    /// A line for each frame it reads: the name of the input, and the node read.
    fn inputs(
        &self,
        label_width: f32,
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
                let others: Vec<&str> = self
                    .pipeline
                    .nodes
                    .iter()
                    .map(|other| other.id.as_str())
                    .filter(|id| *id != node.id)
                    .collect();
                let wired = look::pick(
                    source,
                    others,
                    ToString::to_string,
                    "unwired",
                    font::SANS,
                    tone,
                )
                .on_select(wire)
                .width(Fill);
                container(look::labelled(name, label_width, None, wired))
                    .id(anchor(slot))
                    .boxed()
            });
        column(inputs).spacing(spacing::XS)
    }

    /// An image of its frame to browse, with what picks the rows and steps through them.
    fn gallery(
        &self,
        width: f32,
    ) -> Option<impl Widget<Event> + use<'a>> {
        let gallery = self
            .gallery?
            // within the padding of the card
            .view(self.shown, width - 2.0 * PAD, self.zoom, self.help)?;
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

/// How the theme rings the card of a node of this status, once anything is known of it.
pub fn ring(status: Status) -> Option<Ring> {
    match status {
        Status::Unknown => None,
        Status::Checked => Some(Ring::Sound),
        Status::Rejected => Some(Ring::Unsound),
        Status::Ran => Some(Ring::Done),
        Status::Failed => Some(Ring::Failed),
    }
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
