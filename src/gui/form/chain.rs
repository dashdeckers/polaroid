//! An expression laid out as its chain of calls: the root, then one call to a line.

use std::iter::successors;

use iced::widget::{PickList, column, container, mouse_area, pick_list, row, text_input};
use iced::{Center, Element, Fill, Font, Function, Widget, padding};
use serde_json::Value;

use super::{Expressions, Form, Message, Tone, small};
use crate::methods::{self, Method};
use crate::schema;

/// How far a call is set in from its root, and arguments from their call.
const INDENT: f32 = 20.0;

/// Arguments longer than this are not written while folded away.
const WRITTEN: usize = 32;

impl<'a> Form<'a, '_> {
    /// The expression `value` at the pointer `at` as its chain; a hole offers the functions.
    pub(super) fn chain(
        self,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        // the nodes from the last call back to the root, each with the Method it is a call of
        let read = |node: &'a Value, pointer: String| (methods::read(node), node, pointer);
        let links: Vec<_> = successors(Some(read(value, at.clone())), |(method, node, pointer)| {
            let receiver = method
                .as_ref()?
                .receiver
                .as_ref()?;
            Some(read(
                node.pointer(receiver)?,
                format!("{pointer}{receiver}"),
            ))
        })
        .collect();

        let lines = links
            .into_iter()
            .rev()
            .map(|(method, node, pointer)| match method {
                Some(method) => self.call(method, node, pointer),
                None if node.is_null() => pick(None, choices(None), label, "pl.…")
                    .on_select(Message::Replace.with(pointer))
                    .boxed(),
                None => self.plain(&self.defs["Expr"], node, pointer, Tone::Given),
            });
        let add = (!value.is_null()).then(|| self.add(at));
        let lines = lines
            .chain(add)
            .enumerate()
            .map(|(i, line)| match i {
                0 => line,
                _ => container(line)
                    .padding(padding::left(INDENT))
                    .boxed(),
            });
        column(lines)
            .spacing(5)
            .boxed()
    }

    /// One call of a chain: its name, which picks another in its place, and its arguments.
    fn call(
        self,
        method: &'static Method,
        node: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        // the namespace of a method; it may be replaced by those that are called the same way
        let called = method
            .receiver
            .as_ref()
            .map(|_| method.namespace);
        let before = match called {
            None => String::new(),
            Some("") => ".".to_owned(),
            Some(namespace) => format!(".{namespace}."),
        };
        let name = pick(Some(method), choices(called), label, "")
            .on_select(Message::Replace.with(at.clone()));
        let remove = small("×", Message::Remove(at.clone()));
        let toggle =
            |label: &str| mouse_area(code(label)).on_double_click(Message::Toggle(at.clone()));
        let line = |after: Element<'a, Message>| {
            row![
                code(before),
                name,
                after,
                remove
            ]
            .spacing(2)
            .align_y(Center)
        };

        if let Some(inline) = self.inline(method, node, &at) {
            let after = row![code("("), inline, code(")")].align_y(Center);
            return line(after.boxed()).boxed();
        }
        if method.inputs.len() + method.options.len() == 0 {
            return line(code("()").boxed()).boxed();
        }
        if !self
            .state
            .expanded
            .contains(&at)
        {
            let written = methods::arguments(method, node);
            let is_short = written
                .chars()
                .count()
                <= WRITTEN;
            let after = match is_short {
                true => toggle(&format!("({written})")),
                false => toggle("(…)"),
            };
            return line(after.boxed()).boxed();
        }

        let expr = &self.defs["Expr"];
        let arguments = method
            .inputs
            .iter()
            .chain(&method.options)
            .map(|arg| {
                let value = node
                    .pointer(&arg.at)
                    .unwrap_or(&Value::Null);
                row![
                    code(format!("{}=", arg.name)),
                    self.value(
                        schema::pointer(self.defs, expr, &arg.at),
                        value,
                        format!("{at}{}", arg.at)
                    )
                ]
                .spacing(5)
            });
        column![
            line(toggle("(").boxed()),
            container(column(arguments).spacing(5)).padding(padding::left(INDENT)),
            toggle(")"),
        ]
        .spacing(5)
        .boxed()
    }

    /// The argument of a `col` or a plain `lit`, to edit in place: a column or a value.
    fn inline(
        self,
        method: &Method,
        node: &'a Value,
        at: &str,
    ) -> Option<Element<'a, Message>> {
        let option = method
            .options
            .first()?;
        let value = node.pointer(&option.at)?;
        let put = Message::Put.with(format!("{at}{}", option.at));

        match method.kind.as_str() {
            "/Column" => {
                let name = value
                    .as_str()
                    .unwrap_or_default();
                let columns = match self.expressions {
                    Expressions::Chained(columns) => columns,
                    Expressions::Named(_) => &[],
                };
                if columns.is_empty() {
                    return Some(
                        text_input("column", name)
                            .on_input(move |name| put(name.into()))
                            .width(Fill.max(150))
                            .boxed(),
                    );
                }
                // it shows a name that is none of the columns, as one named before they were known
                let selected = columns
                    .iter()
                    .find(|column| *column == name)
                    .cloned();
                let placeholder = match name {
                    "" => "column…",
                    name => name,
                };
                Some(
                    pick_list(selected, columns.to_vec(), String::clone)
                        .on_select(move |name| put(name.into()))
                        .placeholder(placeholder.to_owned())
                        .boxed(),
                )
            }
            "/Literal" => Some(
                text_input("value", methods::typed(node)?)
                    .on_input(move |typed| put(methods::literal_of(&typed)))
                    .width(Fill.max(150))
                    .boxed(),
            ),
            _ => None,
        }
    }

    /// The line that adds a call to the chain at `at`: its namespaces, then their methods.
    fn add(
        self,
        at: String,
    ) -> Element<'a, Message> {
        let narrowed = match &self.state.namespace {
            Some((pointer, namespace)) if *pointer == at => *namespace,
            _ => {
                return row![
                    code("."),
                    small("+", Message::Narrow(at, ""))
                ]
                .align_y(Center)
                .boxed();
            }
        };

        let mut namespaces: Vec<&'static str> = Vec::new();
        for method in methods::methods() {
            let is_new = method
                .receiver
                .is_some()
                && !method
                    .namespace
                    .is_empty()
                && !namespaces.contains(&method.namespace);
            if is_new {
                namespaces.push(method.namespace);
            }
        }
        // back to the methods of the expression itself
        if !narrowed.is_empty() {
            namespaces.insert(0, "");
        }
        let selected = (!narrowed.is_empty()).then_some(narrowed);
        let named = |namespace: &&str| match *namespace {
            "" => "(none)".to_owned(),
            namespace => namespace.to_owned(),
        };
        let namespace = pick(selected, namespaces, named, "namespace")
            .on_select(Message::Narrow.with(at.clone()));
        let method =
            pick(None, choices(Some(narrowed)), label, "method").on_select(Message::Call.with(at));

        row![
            code("."),
            namespace,
            code("."),
            method
        ]
        .spacing(2)
        .align_y(Center)
        .boxed()
    }
}

/// The functions, or the methods of `namespace`, the most picked first.
fn choices(namespace: Option<&str>) -> Vec<&'static Method> {
    let mut choices: Vec<_> = methods::methods()
        .iter()
        .filter(|method| {
            method
                .receiver
                .as_ref()
                .map(|_| method.namespace)
                == namespace
        })
        .collect();
    choices.sort_by_cached_key(|method| {
        let label = label(method);
        (
            !["pl.col", "pl.lit", "alias"].contains(&label.as_str()),
            label,
        )
    });
    choices
}

/// What a Method is picked by: its name, or the full name of a function.
fn label(method: &&Method) -> String {
    match (&method.receiver, method.namespace) {
        (Some(_), _) => method
            .name
            .to_owned(),
        (None, "") => format!("pl.{}", method.name),
        (None, namespace) => format!("{namespace}.{}", method.name),
    }
}

/// A pick list of code, as wide as what it shows: `selected`, or else `placeholder`.
fn pick<'a, T: Clone + PartialEq + 'a>(
    selected: Option<T>,
    options: Vec<T>,
    label: impl Fn(&T) -> String + 'a,
    placeholder: &str,
) -> PickList<'a, T, Vec<T>, T, Message> {
    let text = selected
        .as_ref()
        .map_or_else(|| placeholder.to_owned(), &label);
    // about nine to a letter, then the padding and the arrow
    let width = text.chars().count() as f32 * 9.0 + 36.0;
    pick_list(selected, options, label)
        .placeholder(placeholder)
        .padding([2, 6])
        .width(width)
        .font(Font::MONOSPACE)
}

/// Text that reads as code.
fn code<'a>(text: impl Into<String>) -> iced::widget::Text<'a> {
    iced::widget::text(text.into()).font(Font::MONOSPACE)
}
