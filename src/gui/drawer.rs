//! The drawer of what a pipeline names: its expressions and values, each edited under its name.

use std::collections::BTreeMap;
use std::rc::Rc;

use iced::widget::{column, container, row, scrollable, space, text};
use iced::{Center, Element, Fill, Function, Widget};
use polaroid_iced::leave;
use polaroid_iced::theme::{self, Tone, font, size, spacing};
use serde_json::{Map, Value};

use super::form::{self, Expressions, Form};
use super::look::{self, Help, add, cross, name_field, toggle};
use crate::{Kind, Pipeline, Registry, methods};

/// What the drawer shows of what a pipeline names, by the names.
#[derive(Default)]
pub struct Drawer {
    sections: BTreeMap<String, Section>,
}

/// What the drawer shows of an expression or a value that the pipeline does not hold.
#[derive(Default)]
struct Section {
    is_open: bool,
    form: form::State,
    /// The name as it is being typed, until its field is left.
    name: Option<String>,
    /// It as it is edited, if the pipeline does not take that yet.
    draft: Option<form::Draft>,
}

/// What is done to the expression or the value of a name.
#[derive(Debug, Clone)]
pub enum Message {
    /// An empty expression or value is added, under a name that is free.
    New(Kind),
    /// Its editor is laid open, or closed if it is open.
    Toggle(String),
    Name(String, String),
    Rename(String),
    /// The field of its name lost the focus.
    Leave(String),
    Form(String, form::Message),
    /// It holds this value at this pointer, as a card that names it moved a number of it.
    Put(String, String, Value),
    Delete(String),
}

/// What an update did to the pipeline.
pub enum Change {
    Nothing,
    /// The pipeline is another, but no node computes anything else.
    Pipeline,
    /// What is named this is another or gone, for every node that uses it.
    Named(String),
}

impl Drawer {
    /// Does what `message` asks, to the drawer or to what `pipeline` names.
    pub fn update(
        &mut self,
        message: Message,
        pipeline: &mut Pipeline,
        registry: &Registry,
    ) -> Change {
        match message {
            Message::New(kind) => {
                let (named, stem) = match kind {
                    Kind::Value => (&pipeline.values, "value"),
                    _ => (&pipeline.expressions, "expression"),
                };
                let name = (1..)
                    .map(|n| format!("{stem}_{n}"))
                    .find(|name| !named.contains_key(name))
                    .expect("some number is free");
                holding(pipeline, kind == Kind::Value).insert(name.clone(), Value::Null);
                self.section(name)
                    .is_open = true;
                Change::Pipeline
            }
            Message::Toggle(name) => {
                let section = self.section(name);
                section.is_open = !section.is_open;
                Change::Nothing
            }
            Message::Name(name, typed) => {
                self.section(name)
                    .name = Some(typed);
                Change::Nothing
            }
            Message::Rename(old) => {
                let typed = self
                    .section(old.clone())
                    .name
                    .clone();
                // one that is refused stays typed
                let Some(new) = typed.filter(|new| pipeline.rename_named(&old, new)) else {
                    return Change::Nothing;
                };
                // what is shown of it goes along to its new name
                if let Some(mut section) = self
                    .sections
                    .remove(&old)
                {
                    section.name = None;
                    self.sections
                        .insert(new, section);
                }
                Change::Pipeline
            }
            Message::Leave(name) => {
                self.section(name)
                    .name = None;
                Change::Nothing
            }
            Message::Form(name, message) => {
                let is_value = pipeline
                    .values
                    .contains_key(&name);
                let Some(stored) = holding(pipeline, is_value)
                    .get(&name)
                    .cloned()
                else {
                    return Change::Nothing;
                };
                let section = self.section(name.clone());
                let check = check(pipeline, registry, &name);
                let draft = section
                    .draft
                    .as_ref();
                let change = section
                    .form
                    .update(message, &stored, draft, &*check);
                drop(check);
                match change {
                    form::Change::Nothing => Change::Nothing,
                    // until the pipeline takes it, the nodes that use it have it as it was
                    form::Change::Draft(draft) => {
                        section.draft = Some(draft);
                        Change::Nothing
                    }
                    form::Change::Value(value) => {
                        section.draft = None;
                        if value == stored {
                            return Change::Nothing;
                        }
                        holding(pipeline, is_value).insert(name.clone(), value);
                        Change::Named(name)
                    }
                    form::Change::Named(name, at, value) => {
                        self.update(Message::Put(name, at, value), pipeline, registry)
                    }
                }
            }
            Message::Put(name, at, value) => {
                let is_value = pipeline
                    .values
                    .contains_key(&name);
                let Some(named) = holding(pipeline, is_value).get_mut(&name) else {
                    return Change::Nothing;
                };
                form::put(named, &at, Some(value));
                Change::Named(name)
            }
            Message::Delete(name) => {
                self.sections
                    .remove(&name);
                let is_value = pipeline
                    .values
                    .contains_key(&name);
                match holding(pipeline, is_value).shift_remove(&name) {
                    Some(_) => Change::Named(name),
                    None => Change::Nothing,
                }
            }
        }
    }

    /// What is shown of the expression or the value of a name.
    fn section(
        &mut self,
        name: String,
    ) -> &mut Section {
        self.sections
            .entry(name)
            .or_default()
    }

    /// The expressions and the values each under its name; `columns` are those they may name.
    pub fn view<'a>(
        &'a self,
        defs: &'a Value,
        pipeline: &'a Pipeline,
        registry: &'a Registry,
        columns: &[String],
        help: Help,
    ) -> impl Widget<Message> + use<'a> {
        let closed = form::State::default();
        let is_taken = |name: &str| {
            pipeline
                .expressions
                .contains_key(name)
                || pipeline
                    .values
                    .contains_key(name)
        };
        let section = |kind: Kind, name: &'a String, stored: &'a Value| {
            let section = self
                .sections
                .get(name);
            let is_open = section.is_some_and(|section| section.is_open);
            let typed = section.and_then(|section| {
                section
                    .name
                    .as_deref()
            });
            let head = row![
                help.of(
                    toggle(is_open, Message::Toggle(name.clone())),
                    "Lays it open to edit it, or closes it to what it is written as.",
                ),
                help.of(
                    leave(
                        name_field("name", typed, name, is_taken)
                            .on_input(Message::Name.with(name.clone()))
                            .on_submit(Message::Rename(name.clone())),
                        Message::Leave(name.clone()),
                    ),
                    "The name that cards and expressions use it by. Type another and press Enter \
                     to rename it, wherever it is used; the border is orange until then, and red \
                     while the name is empty or another's. Leaving the field shows the name again.",
                ),
                help.of(
                    cross(Message::Delete(name.clone())),
                    "Deletes it; what used it is left without one.",
                ),
            ]
            .spacing(spacing::XS)
            .align_y(Center);
            // what a call of an expression that has holes gives, or why it is no sound one
            let takes = (kind == Kind::Expression)
                .then(|| pipeline.holes(name))
                .and_then(|holes| match holes {
                    Ok(holes) if holes.is_empty() => None,
                    Ok(holes) => {
                        let holes: Vec<String> = holes
                            .iter()
                            .map(|(hole, taken)| format!("{hole}: {taken}"))
                            .collect();
                        Some(look::aside(format!("takes {}", holes.join(", "))))
                    }
                    Err(e) => Some(look::note(form::line(&e), Tone::Refused)),
                });
            // open it is edited, and closed it is written out
            let draft = section.and_then(|section| {
                section
                    .draft
                    .as_ref()
            });
            let (value, held) = form::Draft::shown(draft, stored);
            let (schema, expressions, written) = match kind {
                Kind::Expression => (&defs["Expr"], Expressions::Chained, methods::text(value)),
                _ => (&form::ANY, Expressions::Named, value.to_string()),
            };
            let body: Element<'a, Message> = match is_open {
                true => Form {
                    state: section.map_or(&closed, |section| &section.form),
                    defs,
                    pipeline,
                    columns,
                    expressions,
                    check: &check(pipeline, registry, name),
                    value,
                    held,
                    label_width: 0.0,
                    help,
                }
                .view(schema)
                .map(Message::Form.with(name.clone()))
                .boxed(),
                false => look::aside(written)
                    .font(font::MONO)
                    .boxed(),
            };
            column![head, takes, body]
                .spacing(spacing::XS)
                .boxed()
        };
        let group = |title: &'a str, kind: Kind, named: &'a Map<String, Value>, says: &str| {
            let head = row![
                text(title).size(size::HEADING),
                space::horizontal(),
                help.of(add(Message::New(kind)), says),
            ]
            .align_y(Center);
            let sections = named
                .iter()
                .map(|(name, stored)| section(kind, name, stored));
            column![
                head,
                column(sections).spacing(spacing::L)
            ]
            .spacing(spacing::M)
            .width(Fill)
        };
        let named = column![
            group(
                "Expressions",
                Kind::Expression,
                &pipeline.expressions,
                "Adds an empty expression, under a name that is free.",
            ),
            group(
                "Values",
                Kind::Value,
                &pipeline.values,
                "Adds an empty value, under a name that is free: a number, a text, a flag, a \
                 list or a table, for expressions and cards to name.",
            ),
        ]
        .spacing(spacing::XL);
        // the scrollbar beside what is named, not over it
        let bar = scrollable::Scrollbar::new()
            .width(theme::BAR)
            .scroller_width(theme::BAR)
            .spacing(spacing::XS);
        let named = scrollable(named)
            .direction(scrollable::Direction::Vertical(bar))
            .style(theme::scrollable::thin);
        container(named)
            .padding(spacing::M)
            .style(theme::container::sheet)
            .width(Fill)
            .height(Fill)
    }
}

/// The expressions of `pipeline`, or its values.
fn holding(
    pipeline: &mut Pipeline,
    is_value: bool,
) -> &mut Map<String, Value> {
    match is_value {
        true => &mut pipeline.values,
        false => &mut pipeline.expressions,
    }
}

/// Whether `pipeline` takes `name` as it is edited: sound alone, and taken by each of its users.
fn check<'a>(
    pipeline: &'a Pipeline,
    registry: &'a Registry,
    name: &'a str,
) -> form::Check<'a> {
    Rc::new(move |edited| {
        let mut tried = pipeline.clone();
        let is_value = tried
            .values
            .contains_key(name);
        holding(&mut tried, is_value).insert(name.to_owned(), edited.clone());
        if !is_value {
            tried
                .holes(name)
                .map_err(|e| form::line(&e))?;
        }
        let users = tried.users(name);
        tried
            .nodes
            .iter()
            .filter(|node| users.contains(&node.id.as_str()))
            .try_for_each(|node| {
                registry
                    .resolved(&node.block, &node.args, &tried)
                    .map(drop)
                    .map_err(|e| format!("{}: {}", node.id, form::line(&e)))
            })
    })
}
