//! The drawer of expressions: each under its name, to edit there and to use by that name in a card.

use std::collections::BTreeMap;

use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Center, Fill, Function, Widget};
use polaroid_iced::leave;
use serde_json::{Map, Value};

use super::form::{self, Expressions};
use super::{Help, NOTE, TITLE, cross, name_field, toggle};
use crate::{Pipeline, methods};

/// What the drawer shows of the expressions of a pipeline, by their names.
#[derive(Default)]
pub struct Drawer {
    sections: BTreeMap<String, Section>,
}

/// What the drawer shows of an expression that the pipeline does not hold; closed at first.
#[derive(Default)]
struct Section {
    is_open: bool,
    form: form::State,
    /// The name as it is being typed, until its field is left.
    name: Option<String>,
}

/// What is done to the expression of a name.
#[derive(Debug, Clone)]
pub enum Message {
    /// An empty expression is added, under a name that is free.
    New,
    /// Its editor is laid open, or closed if it is open.
    Toggle(String),
    Name(String, String),
    Rename(String),
    /// The field of its name lost the focus.
    Leave(String),
    Form(String, form::Message),
    Delete(String),
}

/// What an update did to the pipeline.
pub enum Change {
    Nothing,
    /// The pipeline is another, but no node computes anything else.
    Pipeline,
    /// The expression of this name is another or gone, for every node that uses it.
    Expression(String),
}

impl Drawer {
    /// Does what `message` asks, to the drawer or to the expressions of `pipeline`.
    pub fn update(
        &mut self,
        message: Message,
        pipeline: &mut Pipeline,
    ) -> Change {
        let expressions = &mut pipeline.expressions;
        match message {
            Message::New => {
                let name = (1..)
                    .map(|n| format!("expression_{n}"))
                    .find(|name| !expressions.contains_key(name))
                    .expect("some number is free");
                expressions.insert(name.clone(), Value::Null);
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
                let Some(expression) = expressions.get_mut(&name) else {
                    return Change::Nothing;
                };
                // the form may only show its value another way
                let is_changed = self
                    .section(name.clone())
                    .form
                    .update(message, expression);
                match is_changed {
                    true => Change::Expression(name),
                    false => Change::Nothing,
                }
            }
            Message::Delete(name) => {
                self.sections
                    .remove(&name);
                match expressions.shift_remove(&name) {
                    Some(_) => Change::Expression(name),
                    None => Change::Nothing,
                }
            }
        }
    }

    /// What is shown of the expression of a name.
    fn section(
        &mut self,
        name: String,
    ) -> &mut Section {
        self.sections
            .entry(name)
            .or_default()
    }

    /// The expressions each under its name; `columns` are those they may name.
    pub fn view<'a>(
        &self,
        defs: &'a Value,
        expressions: &'a Map<String, Value>,
        columns: &[String],
        help: Help,
    ) -> impl Widget<Message> + use<'a> {
        let closed = Section::default();
        let sections = expressions
            .iter()
            .map(|(name, expression)| {
                let section = self
                    .sections
                    .get(name)
                    .unwrap_or(&closed);
                let typed = section
                    .name
                    .as_deref();
                let head = row![
                    help.of(
                        toggle(section.is_open, Message::Toggle(name.clone())),
                        "Lays the expression open to edit it, or closes it to its text.",
                    ),
                    help.of(
                        leave(
                            name_field("name", typed, name, |name| {
                                expressions.contains_key(name)
                            })
                            .on_input(Message::Name.with(name.clone()))
                            .on_submit(Message::Rename(name.clone()))
                            .size(TITLE)
                            .padding(4),
                            Message::Leave(name.clone()),
                        ),
                        "The name that cards pick the expression by. Type another and press \
                         Enter to rename it, wherever it is used; the border is orange until \
                         then, and red while the name is empty or another's. Leaving the field \
                         shows the name again.",
                    ),
                    help.of(
                        cross(Message::Delete(name.clone())),
                        "Deletes the expression; the cards that used it are left without one.",
                    ),
                ]
                .spacing(6)
                .align_y(Center);
                // open it is edited, and closed it is written out
                let body = match section.is_open {
                    true => {
                        let schema = &defs["Expr"];
                        let chained = Expressions::Chained(columns);
                        form::view(&section.form, defs, schema, expression, chained, 0.0, help)
                            .map(Message::Form.with(name.clone()))
                            .boxed()
                    }
                    false => text(methods::text(expression))
                        .size(NOTE)
                        .style(text::secondary)
                        .boxed(),
                };
                column![head, body]
                    .spacing(6)
                    .boxed()
            });
        let head = row![
            "Expressions",
            space::horizontal(),
            help.of(
                button("New").on_press(Message::New),
                "Adds an empty expression, under a name that is free.",
            ),
        ]
        .align_y(Center);
        let sections = column(sections)
            .spacing(14)
            .width(Fill);
        container(
            column![
                head,
                // the scrollbar beside the expressions, not over them
                scrollable(sections).spacing(4),
            ]
            .spacing(10),
        )
        .padding(10)
        .width(Fill)
        .height(Fill)
    }
}
