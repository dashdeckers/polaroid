//! Marks: the tags that the names and the nodes of a pipeline carry, and their colours.

use std::collections::BTreeSet;

use iced::widget::{button, column, container, mouse_area, row, rule, space, stack, text};
use iced::{Center, Color, Element, Fill, Function, Right, Widget, padding};
use polaroid_iced::theme::{self, Tone, dot, extent, font, hue, spacing, stroke};
use serde_json::Value;

use super::look::{self, Help, cross};
use super::ui::object;
use crate::{Catalog, Pipeline};

/// Where a `ui` keeps tags, by name in that of a pipeline, and the colours of tags and categories.
const TAGS: &str = "tags";
const COLORS: &str = "colors";
const CATEGORIES: &str = "categories";

/// What of the tags is under way: the picker that is open, of tags or of a colour.
#[derive(Default)]
pub struct Marks {
    picking: Option<Picking>,
    /// What the picker of a colour is open for.
    painting: Option<Mark>,
}

/// What a picker of tags is open for, and what it shows that the pipeline does not hold.
struct Picking {
    carrier: Carrier,
    /// The name of a new tag as it is typed.
    typed: String,
    /// The tags that were picked in it, which it lists though nothing carries them any more.
    picked: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The picker of the tags of this opens.
    Pick(Carrier),
    Close,
    /// What the picker is open for carries this tag, or no longer if it did.
    Toggle(String),
    /// The name of a new tag as it is typed.
    Type(String),
    /// What the picker is open for carries the tag that was typed, which is made if it is new.
    Add,
    /// The first tag is called the second wherever it is carried, if no tag has that name.
    Rename(String, String),
    /// Nothing carries this tag any more.
    Remove(String),
    /// The picker of the colour of this opens, or with none the one that is open closes.
    Paint(Option<Mark>),
    /// This is of this colour from now on.
    Painted(Mark, Color),
}

/// What carries tags: what a pipeline names this, or its node of this id.
#[derive(Debug, Clone, PartialEq)]
pub enum Carrier {
    Name(String),
    Node(String),
}

/// What has a colour of its own: a tag, or a category of blocks.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Tag(String),
    Category(String),
}

impl Marks {
    /// Does what `message` asks; true if that changed the pipeline.
    pub fn update(
        &mut self,
        message: Message,
        pipeline: &mut Pipeline,
    ) -> bool {
        match (message, &mut self.picking) {
            // a picker of tags opens with no colour being picked in it
            (Message::Pick(carrier), _) => {
                self.painting = None;
                self.picking = Some(Picking {
                    carrier,
                    typed: String::new(),
                    picked: BTreeSet::new(),
                });
            }
            (Message::Close, _) => (self.picking, self.painting) = (None, None),
            (Message::Paint(mark), _) => self.painting = mark,
            (Message::Painted(mark, color), _) => {
                self.painting = None;
                let (kind, name) = match mark {
                    Mark::Tag(name) => (TAGS, name),
                    Mark::Category(name) => (CATEGORIES, name),
                };
                object(&mut pipeline.ui, &[COLORS, kind]).insert(name, hex(color).into());
                return true;
            }
            (Message::Type(typed), Some(picking)) => picking.typed = typed,
            (Message::Toggle(tag), Some(picking)) => {
                let is_carried = of(pipeline, &picking.carrier).contains(tag.as_str());
                carry(pipeline, &picking.carrier, &tag, !is_carried);
                picking
                    .picked
                    .insert(tag);
                return true;
            }
            (Message::Add, Some(picking)) => {
                let tag = std::mem::take(&mut picking.typed)
                    .trim()
                    .to_owned();
                if tag.is_empty() {
                    return false;
                }
                // one that is new takes the hue that the fewest tags have
                let hue = fresh(pipeline);
                object(&mut pipeline.ui, &[COLORS, TAGS])
                    .entry(tag.as_str())
                    .or_insert(hex(hue).into());
                carry(pipeline, &picking.carrier, &tag, true);
                return true;
            }
            (Message::Rename(old, new), _) => return rename(pipeline, &old, new.trim()),
            (Message::Remove(tag), _) => {
                let carriers: Vec<Carrier> = carriers(pipeline).collect();
                retag(pipeline, &carriers, |carried| {
                    carried.remove(&tag);
                });
                return true;
            }
            // nothing is picked in a picker that is not open
            (_, None) => {}
        }
        false
    }

    /// The tags in use to rename or to take off everything, over the categories of blocks.
    pub fn list<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        catalog: &'a Catalog,
        shows: impl Fn(&str) -> bool,
        help: Help,
    ) -> Element<'a, Message> {
        let used = all(pipeline);
        // a colour is picked in the picker of tags while that is open, and else here
        let painting = self
            .painting
            .as_ref()
            .filter(|_| {
                self.picking
                    .is_none()
            });
        let tags = used
            .iter()
            .copied()
            .filter(|tag| shows(tag))
            .map(|tag| {
                // around what is typed, spaces are no part of a name
                let is_taken = move |new: &str| {
                    let new = new.trim();
                    new.is_empty() || all(pipeline).contains(new)
                };
                let name = look::name("tag", tag, is_taken, Message::Rename.with(tag.to_owned()));
                let uses = carriers(pipeline)
                    .filter(|carrier| of(pipeline, carrier).contains(tag))
                    .count();
                let hue = color(&pipeline.ui, tag);
                let line = row![
                    look::dot(hue, dot::M, true),
                    help.of(
                        name,
                        "The name of the tag. Type another and press Enter to rename it wherever \
                         it is carried; the border is orange until then, and red while the name \
                         is empty or another tag's. Right-click the line to pick its colour.",
                    ),
                    help.of(
                        look::aside(uses.to_string()),
                        "How many names and nodes carry the tag."
                    ),
                    help.of(
                        cross(Message::Remove(tag.to_owned())),
                        "Takes the tag off everything that carries it, which is the end of it.",
                    ),
                ]
                .spacing(spacing::S)
                .align_y(Center);
                painted(painting, Mark::Tag(tag.to_owned()), hue, line)
            })
            .collect::<Vec<_>>();
        // under them and a line, the categories of blocks, each in the colour that stands for it
        let divider = (!tags.is_empty()).then(|| rule::horizontal(stroke::THIN).boxed());
        let categories = catalog
            .categories
            .iter()
            .filter(|category| shows(category))
            .map(|category| {
                let hue = self::category(&pipeline.ui, catalog, category);
                let line = row![
                    look::dot(hue, dot::M, true),
                    text(*category),
                    space::horizontal()
                ]
                .spacing(spacing::S)
                .align_y(Center);
                let mark = Mark::Category((*category).to_owned());
                help.of(
                    painted(painting, mark, hue, line),
                    "A category of blocks, in the colour that stands for it. Right-click it to \
                     pick another.",
                )
            });
        column(tags)
            .extend(divider)
            .extend(categories)
            .spacing(spacing::S)
            .boxed()
    }

    /// The picker of the tags of `carrier`, if it is the one that is open.
    pub fn picker<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        carrier: &Carrier,
    ) -> Option<Element<'a, Message>> {
        let picking = self
            .picking
            .as_ref()
            .filter(|picking| picking.carrier == *carrier)?;
        let carried = of(pipeline, carrier);
        let picked = picking
            .picked
            .iter()
            .map(String::as_str);
        let tags = all(pipeline)
            .into_iter()
            .chain(picked)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|tag| {
                let hue = color(&pipeline.ui, tag);
                let is_carried = carried.contains(tag);
                let line = row![
                    look::ticked(Some(hue), tag, is_carried, move |_| {
                        Message::Toggle(tag.to_owned())
                    }),
                    space::horizontal()
                ];
                let painting = self
                    .painting
                    .as_ref();
                painted(painting, Mark::Tag(tag.to_owned()), hue, line)
            });
        let new = look::field("new tag", &picking.typed, font::SANS, Tone::Given)
            .on_input(Message::Type)
            .on_submit(Message::Add)
            .width(Fill);
        let picker = container(
            column(tags)
                .push(new.boxed())
                .spacing(spacing::S),
        )
        .padding(spacing::M)
        .width(extent::M)
        .style(theme::container::menu);
        Some(picker.boxed())
    }
}

/// The tags that `carrier` carries in `pipeline`: a name in its `ui`, and a node in its own.
pub fn of<'a>(
    pipeline: &'a Pipeline,
    carrier: &Carrier,
) -> BTreeSet<&'a str> {
    let list = match carrier {
        Carrier::Name(name) => &pipeline.ui[TAGS][name.as_str()],
        Carrier::Node(id) => pipeline
            .nodes
            .iter()
            .find(|node| node.id == *id)
            .map_or(&Value::Null, |node| &node.ui[TAGS]),
    };
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// The tags that the node of this id carries through the names it uses, and not itself.
pub fn through<'a>(
    pipeline: &'a Pipeline,
    id: &str,
) -> BTreeSet<&'a str> {
    let own = of(pipeline, &Carrier::Node(id.to_owned()));
    pipeline
        .used_by(id)
        .into_iter()
        .flat_map(|name| of(pipeline, &Carrier::Name(name.to_owned())))
        .filter(|tag| !own.contains(tag))
        .collect()
}

/// Every tag in use: each that a name or a node of `pipeline` carries.
pub fn all(pipeline: &Pipeline) -> BTreeSet<&str> {
    carriers(pipeline)
        .flat_map(|carrier| of(pipeline, &carrier))
        .collect()
}

/// All that may carry a tag: what `pipeline` names that its `ui` lists, and its nodes.
fn carriers(pipeline: &Pipeline) -> impl Iterator<Item = Carrier> {
    let names = pipeline.ui[TAGS]
        .as_object()
        .into_iter()
        .flat_map(|named| named.keys())
        .filter(|name| {
            pipeline
                .kind_of(name)
                .is_some()
        })
        .cloned()
        .map(Carrier::Name);
    let nodes = pipeline
        .nodes
        .iter()
        .map(|node| Carrier::Node(node.id.clone()));
    names.chain(nodes)
}

/// The colour of a tag as `ui` has it, or grey if it has none that reads.
pub fn color(
    ui: &Value,
    tag: &str,
) -> Color {
    read(&ui[COLORS][TAGS][tag]).unwrap_or(hue::NEUTRAL)
}

/// The colour of a category of blocks: its own in `ui`, or else the hue at its place from the last.
pub fn category(
    ui: &Value,
    catalog: &Catalog,
    name: &str,
) -> Color {
    // the first tags take the first hues, and the categories the others
    let place = catalog
        .categories
        .iter()
        .position(|category| *category == name)
        .unwrap_or_default();
    let last = hue::PALETTE.len() - 1;
    read(&ui[COLORS][CATEGORIES][name]).unwrap_or(hue::PALETTE[last - place % hue::PALETTE.len()])
}

/// The colour that a `ui` writes as `written`, if that reads as one.
fn read(written: &Value) -> Option<Color> {
    written
        .as_str()?
        .parse()
        .ok()
}

/// Has `carrier` carry `tag`, or no longer.
fn carry(
    pipeline: &mut Pipeline,
    carrier: &Carrier,
    tag: &str,
    is_carried: bool,
) {
    retag(pipeline, std::slice::from_ref(carrier), |carried| {
        match is_carried {
            true => carried.insert(tag.to_owned()),
            false => carried.remove(tag),
        };
    });
}

/// Calls the tag `old` `new` wherever it is carried, if `new` is a name that no tag has.
fn rename(
    pipeline: &mut Pipeline,
    old: &str,
    new: &str,
) -> bool {
    if new.is_empty() || all(pipeline).contains(new) {
        return false;
    }
    let carriers: Vec<Carrier> = carriers(pipeline).collect();
    retag(pipeline, &carriers, |carried| {
        if carried.remove(old) {
            carried.insert(new.to_owned());
        }
    });
    let colors = object(&mut pipeline.ui, &[COLORS, TAGS]);
    if let Some(color) = colors.shift_remove(old) {
        colors.insert(new.to_owned(), color);
    }
    true
}

/// Has `change` done to the tags of each of `carriers`, which holds no list once it has none.
fn retag(
    pipeline: &mut Pipeline,
    carriers: &[Carrier],
    change: impl Fn(&mut BTreeSet<String>),
) {
    for carrier in carriers {
        let mut tags: BTreeSet<String> = of(pipeline, carrier)
            .into_iter()
            .map(str::to_owned)
            .collect();
        change(&mut tags);
        // a name has its list under it in the `ui` of the pipeline, and a node in its own
        let (held, key) = match carrier {
            Carrier::Name(name) => (object(&mut pipeline.ui, &[TAGS]), name.as_str()),
            Carrier::Node(id) => {
                let node = pipeline
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == *id);
                let Some(node) = node else {
                    continue;
                };
                (object(&mut node.ui, &[]), TAGS)
            }
        };
        match tags.is_empty() {
            true => held.shift_remove(key),
            false => held.insert(
                key.to_owned(),
                tags.into_iter()
                    .collect(),
            ),
        };
    }
}

/// What was named `old` carries its tags as `new`, or lets go of them if it is gone.
pub fn renamed(
    pipeline: &mut Pipeline,
    old: &str,
    new: Option<&str>,
) {
    let named = pipeline
        .ui
        .get_mut(TAGS)
        .and_then(Value::as_object_mut);
    let Some(named) = named else {
        return;
    };
    if let (Some(carried), Some(new)) = (named.shift_remove(old), new) {
        named.insert(new.to_owned(), carried);
    }
}

/// Lets go of what nothing uses, for the file to hold no more than is in use.
pub fn tidy(pipeline: &mut Pipeline) {
    let gone: Vec<String> = pipeline.ui[TAGS]
        .as_object()
        .into_iter()
        .flat_map(|named| named.keys())
        .filter(|name| {
            pipeline
                .kind_of(name)
                .is_none()
        })
        .cloned()
        .collect();
    for name in gone {
        renamed(pipeline, &name, None);
    }
    let used: BTreeSet<String> = all(pipeline)
        .into_iter()
        .map(str::to_owned)
        .collect();
    object(&mut pipeline.ui, &[COLORS, TAGS]).retain(|tag, _| used.contains(tag));
    // nor does it hold what that leaves empty, up to the `ui` itself
    let is_held = |_: &String, held: &mut Value| match held {
        Value::Object(held) => !held.is_empty(),
        Value::Array(held) => !held.is_empty(),
        _ => true,
    };
    object(&mut pipeline.ui, &[COLORS]).retain(is_held);
    let ui = object(&mut pipeline.ui, &[]);
    ui.retain(is_held);
    if ui.is_empty() {
        pipeline.ui = Value::Null;
    }
}

/// `content`, which stands for `mark` in its `hue`: a right press lays the palette open under it.
fn painted<'a>(
    painting: Option<&Mark>,
    mark: Mark,
    hue: Color,
    content: impl Widget<Message> + 'a,
) -> Element<'a, Message> {
    let is_open = painting == Some(&mark);
    // what lies under it makes room for the palette, which so covers nothing
    let palette = is_open.then(|| {
        let hues = hue::PALETTE
            .into_iter()
            .map(|picked| {
                let swatch = space()
                    .width(dot::L)
                    .height(dot::L);
                let is_picked = picked.into_rgba8() == hue.into_rgba8();
                button(swatch)
                    .padding(0)
                    .style(theme::button::swatch(picked, is_picked))
                    .on_press(Message::Painted(mark.clone(), picked))
                    .boxed()
            });
        row(hues)
            .spacing(spacing::XS)
            .wrap()
    });
    // a second right press closes it again
    let toggle = Message::Paint((!is_open).then_some(mark));
    let content = mouse_area(content).on_right_press(toggle);
    match palette {
        Some(palette) => column![content, palette]
            .spacing(spacing::XS)
            .boxed(),
        None => content.boxed(),
    }
}

/// A small dot for each of `own` tags, and a hollow one for each that comes `through` a name.
pub fn dots<'a, Message: 'a>(
    ui: &Value,
    own: &BTreeSet<&str>,
    through: &BTreeSet<&str>,
) -> Option<Element<'a, Message>> {
    let tags = || {
        own.iter()
            .chain(through)
    };
    let dots = tags().map(|tag| look::dot(color(ui, tag), dot::S, own.contains(tag)));
    // they say their names when hovered
    let names: Vec<&str> = tags()
        .copied()
        .collect();
    (!names.is_empty()).then(|| look::told(row(dots).spacing(spacing::XXS), Some(names.join(", "))))
}

/// `field` with `dots` at the end of it, which is as wide with them as without.
pub fn dotted<'a, Message: 'a>(
    field: impl Widget<Message> + 'a,
    dots: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let dots = dots.map(|dots| {
        container(dots)
            .align_x(Right)
            .center_y(Fill)
            .width(Fill)
            .padding(padding::right(theme::FIELD.right))
    });
    stack![field, dots].boxed()
}

/// The hue of the palette that the fewest tags in use have, and of those the first.
fn fresh(pipeline: &Pipeline) -> Color {
    let used = all(pipeline);
    let having = |hue: &Color| {
        used.iter()
            .filter(|tag| color(&pipeline.ui, tag).into_rgba8() == hue.into_rgba8())
            .count()
    };
    hue::PALETTE
        .into_iter()
        .min_by_key(having)
        .expect("the palette has hues")
}

/// A colour as a `ui` writes it: `#rrggbb`.
fn hex(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
#[path = "../../tests/unit/marks.rs"]
mod tests;
