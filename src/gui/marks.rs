//! Marks: the tags that what a pipeline names carries, and their colours, kept in its `ui`.

use std::collections::BTreeSet;

use iced::widget::{column, container, row};
use iced::{Color, Element, Fill, Widget};
use polaroid_iced::theme::{self, Tone, dot, extent, font, hue, spacing};
use serde_json::Value;

use super::look;
use super::ui::object;
use crate::Pipeline;

/// Where a `ui` keeps the tags of each name, and the colours of the tags.
const TAGS: &str = "tags";
const COLORS: &str = "colors";

/// What of the tags is under way: the picker that is open.
#[derive(Default)]
pub struct Marks {
    picking: Option<Picking>,
}

/// What a picker of tags is open for, and what it shows that the pipeline does not hold.
#[derive(Default)]
struct Picking {
    name: String,
    /// The name of a new tag as it is typed.
    typed: String,
    /// The tags that were picked in it, which it lists though nothing carries them any more.
    picked: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The picker of the tags of what is named this opens.
    Pick(String),
    Close,
    /// What the picker is open for carries this tag, or no longer if it did.
    Toggle(String),
    /// The name of a new tag as it is typed.
    Type(String),
    /// What the picker is open for carries the tag that was typed, which is made if it is new.
    Add,
}

impl Marks {
    /// Does what `message` asks; true if that changed the pipeline.
    pub fn update(
        &mut self,
        message: Message,
        pipeline: &mut Pipeline,
    ) -> bool {
        match (message, &mut self.picking) {
            (Message::Pick(name), _) => {
                self.picking = Some(Picking {
                    name,
                    ..Picking::default()
                });
            }
            (Message::Close, _) => self.picking = None,
            (Message::Type(typed), Some(picking)) => picking.typed = typed,
            (Message::Toggle(tag), Some(picking)) => {
                let is_carried = of(pipeline, &picking.name).contains(tag.as_str());
                carry(pipeline, &picking.name, &tag, !is_carried);
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
                carry(pipeline, &picking.name, &tag, true);
                return true;
            }
            // nothing is picked in a picker that is not open
            (_, None) => {}
        }
        false
    }

    /// The picker of the tags of what is named `name`, if it is the one that is open.
    pub fn picker<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        name: &str,
    ) -> Option<Element<'a, Message>> {
        let picking = self
            .picking
            .as_ref()
            .filter(|picking| picking.name == name)?;
        let carried = of(pipeline, name);
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
                look::ticked(Some(hue), tag, is_carried, move |_| {
                    Message::Toggle(tag.to_owned())
                })
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

/// The tags that what `pipeline` names `name` carries.
pub fn of<'a>(
    pipeline: &'a Pipeline,
    name: &str,
) -> BTreeSet<&'a str> {
    tags(&pipeline.ui[TAGS][name]).collect()
}

/// Every tag in use: each that something `pipeline` names carries.
pub fn all(pipeline: &Pipeline) -> BTreeSet<&str> {
    carried(pipeline)
        .flat_map(tags)
        .collect()
}

/// The list of the tags of each thing that `pipeline` names.
fn carried(pipeline: &Pipeline) -> impl Iterator<Item = &Value> {
    pipeline.ui[TAGS]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(name, _)| {
            pipeline
                .kind_of(name)
                .is_some()
        })
        .map(|(_, carried)| carried)
}

/// The tags that a list of them in a `ui` holds.
fn tags(list: &Value) -> impl Iterator<Item = &str> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

/// The colour of a tag as `ui` has it, or grey if it has none that reads.
pub fn color(
    ui: &Value,
    tag: &str,
) -> Color {
    ui[COLORS][TAGS][tag]
        .as_str()
        .and_then(|written| written.parse().ok())
        .unwrap_or(hue::NEUTRAL)
}

/// Has what is named `name` carry `tag`, or no longer.
fn carry(
    pipeline: &mut Pipeline,
    name: &str,
    tag: &str,
    is_carried: bool,
) {
    let mut carried: BTreeSet<String> = of(pipeline, name)
        .into_iter()
        .map(str::to_owned)
        .collect();
    match is_carried {
        true => carried.insert(tag.to_owned()),
        false => carried.remove(tag),
    };
    let named = object(&mut pipeline.ui, &[TAGS]);
    match carried.is_empty() {
        true => named.shift_remove(name),
        false => named.insert(
            name.to_owned(),
            carried
                .into_iter()
                .collect(),
        ),
    };
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

/// A small dot for each of `tags`, which say their names when hovered; nothing for none.
pub fn dots<'a, Message: 'a>(
    ui: &Value,
    tags: &BTreeSet<&str>,
) -> Option<Element<'a, Message>> {
    let dots = tags
        .iter()
        .map(|tag| look::dot(color(ui, tag), dot::S));
    let names: Vec<&str> = tags
        .iter()
        .copied()
        .collect();
    (!tags.is_empty()).then(|| look::told(row(dots).spacing(spacing::XXS), Some(names.join(", "))))
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
