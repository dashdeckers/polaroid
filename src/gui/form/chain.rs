//! An expression laid out as its chain of calls: the root, then one call to a line.

use std::collections::BTreeSet;
use std::iter::successors;

use iced::widget::{column, container, mouse_area, row, space};
use iced::{Center, Element, Fill, Function, Widget, padding};
use serde_json::{Value, json};

use super::look::{Face, GAP, INDENT, REMOVE, Tone, code, field, fitted, pick, remove, scrubbed};
use super::{self as form, Form, PLAIN, put, small, told};
use crate::methods::{self, Method};
use crate::{reference, schema};

/// What is picked the most, and so offered first.
const FIRST: [&str; 3] = ["pl.col", "pl.lit", "alias"];

/// What the methods that are in no namespace are offered under.
const BASE: &str = "base";

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
    /// Does what `message` asks to the expressions in `value`.
    pub fn update(
        &mut self,
        message: Message,
        value: &mut Value,
    ) {
        match message {
            Message::Toggle(at) => {
                if !self
                    .expanded
                    .remove(&at)
                {
                    self.expanded
                        .insert(at);
                }
            }
            Message::Narrow(at, namespace) => self.namespace = Some((at, namespace)),
            Message::Call(at, method) => {
                // an input that was left out is given from nothing
                if value
                    .pointer(&at)
                    .is_none()
                {
                    put(value, &at, Some(Value::Null));
                }
                let Some(node) = value.pointer_mut(&at) else {
                    return;
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
            }
            // another call in its place is a call on what it leaves; the same again is no change
            Message::Replace(at, method) => {
                let is_same = value
                    .pointer(&at)
                    .and_then(methods::read)
                    == Some(method);
                if !is_same {
                    self.update(Message::Remove(at.clone()), value);
                    self.update(Message::Call(at, method), value);
                }
            }
            Message::Remove(at) => {
                let Some(node) = value.pointer_mut(&at) else {
                    return;
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
    /// The expression `value` at the pointer `at` as its chain; a hole offers what may fill it.
    pub(super) fn chain(
        self,
        value: &'a Value,
        at: String,
    ) -> Element<'a, form::Message> {
        let expr = &self.defs["Expr"];
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

        // each line, after what takes it away if it is a call or a name
        let lines = links
            .into_iter()
            .rev()
            .map(|(method, node, pointer)| match method {
                Some(method) => {
                    let remove = self.help.of(
                        remove(asked(Message::Remove(pointer.clone()))),
                        "Takes this call out of the chain, and leaves what it was called on.",
                    );
                    (Some(remove), self.call(method, node, pointer))
                }
                // a hole offers the functions, and the names of the pipeline beside them
                None if node.is_null() => {
                    let named = self.names(expr, node, &pointer, "@…", Tone::Given);
                    let function = code_pick(None, choices(None), label, "pl.…")
                        .on_select(move |method| asked(Message::Replace(pointer.clone(), method)));
                    let hole = row![function, named]
                        .spacing(GAP)
                        .align_y(Center)
                        .boxed();
                    (None, hole)
                }
                None if reference(node).is_some() => {
                    let remove = self.help.of(
                        remove(form::Message::Put(pointer.clone(), Value::Null)),
                        "Takes this name out of the chain, and leaves a hole.",
                    );
                    (Some(remove), self.named(expr, node, pointer, Tone::Given))
                }
                None => (None, self.plain(expr, node, pointer, Tone::Given)),
            });
        let add = (!value.is_null()).then(|| (None, self.add(at)));
        let lines = lines
            .chain(add)
            .enumerate()
            .map(|(i, (remove, line))| {
                let indent = match i {
                    0 => 0.0,
                    _ => INDENT,
                };
                row![
                    container(remove).width(REMOVE),
                    container(line).padding(padding::left(indent)),
                ]
                .boxed()
            });
        column(lines)
            .spacing(GAP)
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
        let name = code_pick(Some(method), choices(called), label, "").on_select({
            let at = at.clone();
            move |method| asked(Message::Replace(at.clone(), method))
        });
        // the brackets and all the room to their right lay the arguments out, or fold them away
        let toggle = |label: &str| {
            let area = row![
                code(label),
                space::horizontal()
            ]
            .align_y(Center);
            let area = mouse_area(area).on_double_click(asked(Message::Toggle(at.clone())));
            self.help.of(
                area,
                "Double-click to lay the arguments out, or to fold them away.",
            )
        };
        // the name tells what Polars says the method does, and each argument what it is for
        let says = method
            .doc
            .map(|doc| doc.says.to_owned());
        let name = told(name, says);
        let mut name = Some(name);
        let mut line = |after: Element<'a, form::Message>| {
            row![
                code(before.clone()),
                name.take(),
                after
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
                let inside = format!("{at}{}", arg.at);
                let given = node.pointer(&arg.at);
                let says = method
                    .says_of(&arg.name)
                    .map(str::to_owned);
                // one that may be left out is taken out again where it is given
                let leave = (arg.is_optional && given.is_some()).then(|| {
                    self.help.of(
                        small("×", form::Message::Drop(inside.clone())),
                        "Leaves this argument out again.",
                    )
                });
                row![
                    told(code(format!("{}=", arg.name)), says),
                    self.value(
                        schema::pointer(self.defs, expr, &arg.at),
                        given.unwrap_or(&Value::Null),
                        inside
                    ),
                    leave
                ]
                .spacing(GAP)
            });
        column![
            line(toggle("(").boxed()),
            container(column(arguments).spacing(GAP)).padding(padding::left(INDENT)),
            toggle(")"),
        ]
        .spacing(GAP)
        .boxed()
    }

    /// The argument of a `col` or a plain `lit`, to edit in place: a column or a value, or a name.
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
        let at = format!("{at}{}", option.at);
        let put = || form::Message::Put.with(at.clone());
        let named = |schema: &'a Value| self.names(schema, value, &at, "@…", Tone::Given);
        let beside = |edited: Element<'a, form::Message>, schema| {
            row![edited, named(schema)]
                .spacing(GAP)
                .align_y(Center)
                .boxed()
        };

        match method.kind.as_str() {
            "/Column" | "/Literal" if reference(value).is_some() => {
                Some(self.named(&PLAIN[4], value, at.clone(), Tone::Given))
            }
            "/Column" => {
                let name = value
                    .as_str()
                    .unwrap_or_default();
                if self
                    .columns
                    .is_empty()
                {
                    let typed = field("column", name, Face::Code, Tone::Given)
                        .on_input({
                            let put = put();
                            move |name| put(name.into())
                        })
                        .width(Fill.max(150));
                    return Some(beside(typed.boxed(), &PLAIN[0]));
                }
                // it shows a name that is none of the columns, as one named before they were known
                let selected = self
                    .columns
                    .iter()
                    .find(|column| *column == name)
                    .cloned();
                let placeholder = match name {
                    "" => "column…",
                    name => name,
                };
                let columns = self
                    .columns
                    .to_vec();
                let picked = code_pick(selected, columns, String::clone, placeholder)
                    .on_select({
                        let put = put();
                        move |name| put(name.into())
                    })
                    .stepped();
                Some(beside(picked, &PLAIN[0]))
            }
            "/Literal" => {
                let typed = methods::typed(node)?;
                let number = schema::number(&typed);
                // the field keeps its place, and so its focus, while what is typed is no number
                let typed_in = fitted("value", typed, Face::Code, Tone::Given).on_input({
                    let put = put();
                    move |typed| put(methods::literal_of(&typed))
                });
                // a number has a handle that moves it, and it stays whole if it was
                let moved = number.map(|number| {
                    let kind = match number.is_f64() {
                        true => "number",
                        false => "integer",
                    };
                    let put = put();
                    move |steps| {
                        let moved = schema::nudged(&json!({ "type": kind }), &json!(number), steps);
                        put(methods::literal_of(&moved.to_string()))
                    }
                });
                Some(beside(scrubbed(typed_in, moved, self.help), &PLAIN[4]))
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
                    self.help.of(
                        small("+", asked(Message::Narrow(at, ""))),
                        "Calls a method on the expression: pick its namespace, then the method.",
                    )
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
        // the methods of the expression itself come first, under a name of their own
        namespaces.insert(0, "");
        let named = |namespace: &&str| match *namespace {
            "" => BASE.to_owned(),
            namespace => namespace.to_owned(),
        };
        let namespace = code_pick(Some(narrowed), namespaces, named, "namespace").on_select({
            let at = at.clone();
            move |namespace| asked(Message::Narrow(at.clone(), namespace))
        });
        let method = code_pick(None, choices(Some(narrowed)), label, "method")
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

/// A pick list of code, which shows `selected`, or else `placeholder`.
fn code_pick<'a, T: Clone + PartialEq + 'a>(
    selected: Option<T>,
    options: Vec<T>,
    label: impl Fn(&T) -> String + 'a,
    placeholder: &str,
) -> polaroid_iced::PickList<'a, T, form::Message> {
    pick(
        selected,
        options,
        label,
        placeholder,
        Face::Code,
        Tone::Given,
    )
}
