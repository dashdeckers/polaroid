//! The drawer of expressions: each under its name, to edit there and to use by that name in a card.

use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Center, Fill, Widget};
use serde_json::{Map, Value};

use super::form;
use crate::{Pipeline, methods};

/// What the drawer shows of each expression of a pipeline, in their order.
#[derive(Default)]
pub struct Drawer {
    sections: Vec<Section>,
}

/// What the drawer shows of an expression that the pipeline does not hold.
#[derive(Default)]
struct Section {
    is_open: bool,
    form: form::State,
    /// The name as it is being typed, until Enter renames the expression.
    name: Option<String>,
}

/// What is done to the expression of an index.
#[derive(Debug, Clone)]
pub enum Message {
    /// An empty expression is added, under a name that is free.
    New,
    /// Its editor is laid open, or closed if it is open.
    Toggle(usize),
    Name(usize, String),
    Rename(usize),
    Form(usize, form::Message),
    Delete(usize),
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
    /// A drawer of the expressions of `pipeline`, all closed.
    pub fn new(pipeline: &Pipeline) -> Self {
        let sections = pipeline
            .expressions
            .iter()
            .map(|_| Section::default())
            .collect();
        Self { sections }
    }

    /// Does what `message` asks, to the drawer or to the expressions of `pipeline`.
    pub fn update(
        &mut self,
        message: Message,
        pipeline: &mut Pipeline,
    ) -> Change {
        let expressions = &mut pipeline.expressions;
        let name_of = |expressions: &Map<String, Value>, index: usize| {
            expressions
                .keys()
                .nth(index)
                .cloned()
        };
        match message {
            Message::New => {
                let name = (1..)
                    .map(|n| format!("expression_{n}"))
                    .find(|name| !expressions.contains_key(name))
                    .expect("some number is free");
                expressions.insert(name, Value::Null);
                self.sections
                    .push(Section {
                        is_open: true,
                        ..Section::default()
                    });
                Change::Pipeline
            }
            Message::Toggle(index) => {
                if let Some(section) = self
                    .sections
                    .get_mut(index)
                {
                    section.is_open = !section.is_open;
                }
                Change::Nothing
            }
            Message::Name(index, name) => {
                if let Some(section) = self
                    .sections
                    .get_mut(index)
                {
                    section.name = Some(name);
                }
                Change::Nothing
            }
            Message::Rename(index) => {
                let typed = self
                    .sections
                    .get_mut(index)
                    .and_then(|section| section.name.take());
                let (Some(old), Some(new)) = (name_of(expressions, index), typed) else {
                    return Change::Nothing;
                };
                // it keeps its place, as the sections are in the order of the expressions
                match pipeline.rename_expression(&old, &new) {
                    true => Change::Pipeline,
                    false => Change::Nothing,
                }
            }
            Message::Form(index, message) => {
                let (Some(section), Some((name, expression))) = (
                    self.sections
                        .get_mut(index),
                    expressions
                        .iter_mut()
                        .nth(index),
                ) else {
                    return Change::Nothing;
                };
                // the form may only show its value another way
                match section
                    .form
                    .update(message, expression)
                {
                    true => Change::Expression(name.clone()),
                    false => Change::Nothing,
                }
            }
            Message::Delete(index) => {
                let Some(name) = name_of(expressions, index) else {
                    return Change::Nothing;
                };
                expressions.shift_remove(&name);
                self.sections
                    .remove(index);
                Change::Expression(name)
            }
        }
    }

    /// The expressions each under its name; `columns` are those they may name.
    pub fn view<'a>(
        &self,
        defs: &'a Value,
        expressions: &'a Map<String, Value>,
        columns: &[String],
    ) -> impl Widget<Message> + use<'a> {
        let sections = self
            .sections
            .iter()
            .zip(expressions)
            .enumerate()
            .map(|(index, (section, (name, expression)))| {
                let toggle = match section.is_open {
                    true => "−",
                    false => "+",
                };
                let name = section
                    .name
                    .clone()
                    .unwrap_or_else(|| name.clone());
                let head = row![
                    button(toggle)
                        .style(button::text)
                        .padding([0, 6])
                        .on_press(Message::Toggle(index)),
                    text_input("name", name)
                        .on_input(move |name| Message::Name(index, name))
                        .on_submit(Message::Rename(index))
                        .size(14)
                        .padding(4),
                    button("×")
                        .style(button::danger)
                        .padding([0, 6])
                        .on_press(Message::Delete(index)),
                ]
                .spacing(6)
                .align_y(Center);
                // open it is edited, and closed it is written out
                let body = match section.is_open {
                    true => {
                        let schema = &defs["Expr"];
                        let columns = columns.to_vec();
                        form::view(&section.form, defs, schema, expression, columns, None, 0.0)
                            .map(move |message| Message::Form(index, message))
                            .boxed()
                    }
                    false => text(methods::text(expression))
                        .size(12)
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
            button("New").on_press(Message::New),
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
