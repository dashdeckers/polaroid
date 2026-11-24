//! An expression laid out as its chain of calls: the root, then one call to a line.

use std::collections::BTreeSet;
use std::iter::successors;

use iced::widget::{PickList, column, container, mouse_area, pick_list, row, text_input};
use iced::{Center, Element, Fill, Font, Function, Widget, padding};
use polaroid_iced::{scrub, width_of};
use serde_json::{Value, json};

use super::{self as form, Form, Tone, small};
use crate::methods::{self, Method};
use crate::schema;

/// What is picked the most, and so offered first.
const FIRST: [&str; 3] = ["pl.col", "pl.lit", "alias"];

/// The size of text that reads as code.
const CODE: f32 = 16.0;

/// How far a call is set in from its root, and arguments from their call.
const INDENT: f32 = 20.0;

/// Arguments longer than this are not written while folded away.
const WRITTEN: usize = 32;

/// What a chain shows that its expression does not hold.
#[derive(Debug, Default)]
pub struct State {
    /// The calls whose arguments are laid out, by the JSON pointers to their nodes.
    pub(super) expanded: BTreeSet<String>,
    /// The namespace whose methods are offered, and the pointer to the expression to call on.
    namespace: Option<(String, &'static str)>,
}

/// What a chain asks for.
#[derive(Debug, Clone)]
pub enum Message {
    /// Lay out the arguments of the call at this pointer, or fold them away.
    Toggle(String),
    /// Offer the methods of this namespace to call on the expression at this pointer.
    Narrow(String, &'static str),
    /// Call this Method on the expression at this pointer.
    Call(String, &'static Method),
    /// Put a call of this Method at this pointer, on the receiver that is there.
    Replace(String, &'static Method),
    /// Take the call at this pointer away and leave its receiver, or a hole.
    Remove(String),
}

impl State {
    /// Does what `message` asks to the expressions in `value`, and returns whether it changed.
    pub fn update(
        &mut self,
        message: Message,
        value: &mut Value,
    ) -> bool {
        match message {
            Message::Toggle(at) => {
                if !self
                    .expanded
                    .remove(&at)
                {
                    self.expanded
                        .insert(at);
                }
                false
            }
            Message::Narrow(at, namespace) => {
                self.namespace = Some((at, namespace));
                false
            }
            Message::Call(at, method) => {
                let Some(node) = value.pointer_mut(&at) else {
                    return false;
                };
                *node = method.call(node.take());

                // what was here is the receiver now, if the call takes one
                match method
                    .receiver
                    .as_deref()
                {
                    Some(receiver) => self.relocate(&at, Some(""), receiver),
                    None => self.relocate(&at, None, ""),
                }
                if method.inputs.len() + method.options.len() > 0 {
                    self.expanded
                        .insert(at);
                }
                true
            }
            // another call in its place is a call on what it leaves; the same again is no change
            Message::Replace(at, method) => {
                let is_same = value
                    .pointer(&at)
                    .and_then(methods::read)
                    == Some(method);
                if is_same {
                    return false;
                }
                self.update(Message::Remove(at.clone()), value);
                self.update(Message::Call(at, method), value)
            }
            Message::Remove(at) => {
                let Some(node) = value.pointer_mut(&at) else {
                    return false;
                };
                let old = methods::read(node).and_then(|method| {
                    method
                        .receiver
                        .as_deref()
                });
                *node = old
                    .and_then(|old| node.pointer_mut(old))
                    .map(Value::take)
                    .unwrap_or_default();
                self.relocate(&at, old, "");
                true
            }
        }
    }

    /// Follows a change to the node at `at`: what was at `from` below it is at `to`, the rest gone.
    pub(super) fn relocate(
        &mut self,
        at: &str,
        from: Option<&str>,
        to: &str,
    ) {
        // what is left of `pointer` below `root`, if it is there or below
        fn below<'a>(
            pointer: &'a str,
            root: &str,
        ) -> Option<&'a str> {
            pointer
                .strip_prefix(root)
                .filter(|rest| rest.is_empty() || rest.starts_with('/'))
        }

        self.expanded = std::mem::take(&mut self.expanded)
            .into_iter()
            .filter_map(|pointer| {
                let Some(rest) = below(&pointer, at) else {
                    return Some(pointer);
                };
                let rest = below(rest, from?)?;
                Some(format!("{at}{to}{rest}"))
            })
            .collect();
        self.namespace = None;
    }
}

impl<'a> Form<'a, '_> {
    /// The expression `value` at the pointer `at` as its chain; a hole offers the functions.
    pub(super) fn chain(
        self,
        value: &'a Value,
        at: String,
    ) -> Element<'a, form::Message> {
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
                    .on_select(move |method| asked(Message::Replace(pointer.clone(), method)))
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
    ) -> Element<'a, form::Message> {
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
        let name = pick(Some(method), choices(called), label, "").on_select({
            let at = at.clone();
            move |method| asked(Message::Replace(at.clone(), method))
        });
        let remove = small("×", asked(Message::Remove(at.clone())));
        let toggle = |label: &str| {
            mouse_area(code(label)).on_double_click(asked(Message::Toggle(at.clone())))
        };
        let line = |after: Element<'a, form::Message>| {
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
            .chain
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
    ) -> Option<Element<'a, form::Message>> {
        let option = method
            .options
            .first()?;
        let value = node.pointer(&option.at)?;
        let put = || form::Message::Put.with(format!("{at}{}", option.at));

        match method.kind.as_str() {
            "/Column" => {
                let name = value
                    .as_str()
                    .unwrap_or_default();
                let columns = self
                    .expressions
                    .columns();
                if columns.is_empty() {
                    return Some(
                        text_input("column", name)
                            .on_input({
                                let put = put();
                                move |name| put(name.into())
                            })
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
                        .on_select({
                            let put = put();
                            move |name| put(name.into())
                        })
                        .placeholder(placeholder.to_owned())
                        .boxed(),
                )
            }
            "/Literal" => {
                let typed = methods::typed(node)?;
                let number = schema::number(&typed);
                let field = text_input("value", typed)
                    .on_input({
                        let put = put();
                        move |typed| put(methods::literal_of(&typed))
                    })
                    .width(Fill.max(150));
                // a drag across a number moves it, and it stays whole if it was
                let Some(number) = number else {
                    return Some(field.boxed());
                };
                let kind = match number.is_f64() {
                    true => "number",
                    false => "integer",
                };
                let put = put();
                let on_scrub = move |steps| {
                    let moved = schema::nudged(&json!({ "type": kind }), &json!(number), steps);
                    put(methods::literal_of(&moved.to_string()))
                };
                Some(
                    scrub(field)
                        .on_scrub(on_scrub)
                        .boxed(),
                )
            }
            _ => None,
        }
    }

    /// The line that adds a call to the chain at `at`: its namespaces, then their methods.
    fn add(
        self,
        at: String,
    ) -> Element<'a, form::Message> {
        let narrowed = match &self
            .state
            .chain
            .namespace
        {
            Some((pointer, namespace)) if *pointer == at => *namespace,
            _ => {
                return row![
                    code("."),
                    small("+", asked(Message::Narrow(at, "")))
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
        let namespace = pick(selected, namespaces, named, "namespace").on_select({
            let at = at.clone();
            move |namespace| asked(Message::Narrow(at.clone(), namespace))
        });
        let method = pick(None, choices(Some(narrowed)), label, "method")
            .on_select(move |method| asked(Message::Call(at.clone(), method)));

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

/// What a chain asks for, as the form that it is part of passes it on.
fn asked(message: Message) -> form::Message {
    form::Message::Chain(message)
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
        (!FIRST.contains(&label.as_str()), label)
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
) -> PickList<'a, T, Vec<T>, T, form::Message> {
    let text = selected
        .as_ref()
        .map_or_else(|| placeholder.to_owned(), &label);
    // beside its text, the padding and the arrow
    let width = width_of(&text, CODE, Font::MONOSPACE) + 36.0;
    pick_list(selected, options, label)
        .placeholder(placeholder)
        .padding([2, 6])
        .width(width)
        .text_size(CODE)
        .font(Font::MONOSPACE)
}

/// Text that reads as code.
fn code<'a>(text: impl Into<String>) -> iced::widget::Text<'a> {
    iced::widget::text(text.into())
        .size(CODE)
        .font(Font::MONOSPACE)
}
