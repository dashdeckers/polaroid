//! A form for a JSON value, laid out by its JSON Schema; a null is a hole to be filled.

mod chain;

use std::collections::BTreeSet;

use iced::widget::{button, checkbox, column, pick_list, row, text, text_input};
use iced::{Center, Element, Fill, Function, Widget};
use serde_json::{Number, Value, json};

use crate::methods::{self, Method};
use crate::schema::{blank, default, json_type, optional, resolve, variants};

/// What a form shows that its value does not hold.
#[derive(Debug, Default)]
pub struct State {
    /// The calls whose arguments are laid out, by the JSON pointers to their nodes.
    expanded: BTreeSet<String>,
    /// The namespace whose methods are offered, and the pointer to the expression to call on.
    namespace: Option<(String, &'static str)>,
}

/// What a form asks for.
#[derive(Debug, Clone)]
pub enum Message {
    /// Put a value at a JSON pointer.
    Put(String, Value),
    /// Take the array element at a JSON pointer away.
    Drop(String),
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
    /// Does what `message` asks to `value`, and returns whether it changed.
    pub fn update(
        &mut self,
        message: Message,
        value: &mut Value,
    ) -> bool {
        match message {
            Message::Put(at, new) => put(value, &at, Some(new)),
            Message::Drop(at) => {
                put(value, &at, None);
                // the later elements move down, away from the marks under the array
                let array = at
                    .rsplit_once('/')
                    .map_or("", |(array, _)| array);
                self.relocate(array, None, "");
            }
            Message::Toggle(at) => {
                if !self
                    .expanded
                    .remove(&at)
                {
                    self.expanded
                        .insert(at);
                }
                return false;
            }
            Message::Narrow(at, namespace) => {
                self.namespace = Some((at, namespace));
                return false;
            }
            Message::Call(at, method) => {
                let Some(node) = value.pointer_mut(&at) else {
                    return false;
                };
                *node = method.call(node.take());

                // what was here is the receiver now, if the call takes one
                let inner = method
                    .receiver
                    .as_deref();
                self.relocate(&at, inner.and(Some("")), inner.unwrap_or_default());
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
                return !is_same
                    && self.update(Message::Remove(at.clone()), value)
                    && self.update(Message::Call(at, method), value);
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
            }
        }
        true
    }

    /// Follows a change to the node at `at`: what was at `from` below it is at `to`, the rest gone.
    fn relocate(
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

/// The form for `value`, by `schema`; `columns` are those the value may name.
pub fn view<'a>(
    state: &State,
    defs: &'a Value,
    schema: &'a Value,
    value: &'a Value,
    columns: Vec<String>,
) -> impl Widget<Message> + use<'a> {
    let form = Form {
        state,
        defs,
        columns: &columns,
    };
    form.value(schema, value, String::new())
}

/// A form as it is laid out: what every part of it is laid out with.
#[derive(Clone, Copy)]
struct Form<'a, 'c> {
    state: &'c State,
    defs: &'a Value,
    columns: &'c [String],
}

impl<'a> Form<'a, '_> {
    /// The form for the value at the pointer `at`: an expression as a chain, a nullable to clear.
    fn value(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        let schema = resolve(self.defs, schema);
        let inner = optional(schema).map(|inner| resolve(self.defs, inner));
        let clear = (inner.is_some() && !value.is_null())
            .then(|| small("×", Message::Put(at.clone(), Value::Null)));

        let shown = inner.unwrap_or(schema);
        let form = match std::ptr::eq(shown, &self.defs["Expr"]) {
            true => self.chain(value, at),
            false => self.plain(shown, value, at),
        };
        if inner.is_none() {
            return form;
        }
        row![form, clear]
            .spacing(5)
            .align_y(Center)
            .boxed()
    }

    /// The form for a value by nothing but its schema, or the hole it leaves.
    fn plain(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        let defs = self.defs;
        let put = Message::Put.with(at.clone());

        if value.is_null() {
            let blank = blank(defs, schema);
            if !blank.is_null() {
                return small("set", put(blank)).boxed();
            }
        }

        let variants = variants(schema);
        if !variants.is_empty() {
            // a variant is its name, or its name around its payload
            let selected = variants
                .iter()
                .copied()
                .find(|(name, payload)| match payload {
                    Some(_) => value
                        .get(name)
                        .is_some(),
                    None => value == name,
                });
            let pick = pick_list(selected, variants, |(name, _)| name.to_string())
                .on_select(move |(name, payload)| {
                    put(match payload {
                        Some(payload) => json!({ name: default(defs, payload) }),
                        None => name.into(),
                    })
                })
                .placeholder("pick…");

            return match selected {
                Some((name, Some(payload))) => column![
                    pick,
                    self.value(payload, &value[name], format!("{at}/{name}"))
                ]
                .spacing(5)
                .boxed(),
                _ => pick.boxed(),
            };
        }

        match json_type(schema) {
            "object" => column(
                schema["properties"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(name, field)| {
                        row![
                            text(name),
                            self.value(field, &value[name], format!("{at}/{name}"))
                        ]
                        .spacing(10)
                    }),
            )
            .spacing(5)
            .boxed(),
            "array" => match schema["prefixItems"].as_array() {
                Some(items) => row(items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| self.value(item, &value[i], format!("{at}/{i}"))))
                .spacing(10)
                .boxed(),
                None => {
                    let items = &schema["items"];
                    let elements = value
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let add =
                        Message::Put(format!("{at}/{}", elements.len()), default(defs, items));
                    column(
                        elements
                            .iter()
                            .enumerate()
                            .map(|(i, element)| {
                                let at = format!("{at}/{i}");
                                row![
                                    small("×", Message::Drop(at.clone())),
                                    self.value(items, element, at)
                                ]
                                .spacing(5)
                                .boxed()
                            }),
                    )
                    .push(small("+", add).boxed())
                    .spacing(5)
                    .boxed()
                }
            },
            "boolean" => checkbox(
                value
                    .as_bool()
                    .unwrap_or_default(),
            )
            .on_toggle(move |on| put(on.into()))
            .boxed(),
            "integer" | "number" => {
                let typed = match value {
                    Value::String(typed) => typed.clone(),
                    number => number.to_string(),
                };
                text_input("", typed)
                    .on_input(move |typed| put(number(&typed).map_or(typed.into(), Value::from)))
                    .width(Fill.max(100))
                    .boxed()
            }
            "string" => text_input(
                "",
                value
                    .as_str()
                    .unwrap_or_default(),
            )
            .on_input(move |typed| put(typed.into()))
            .boxed(),
            // a shape the form does not know
            _ => text(value.to_string()).boxed(),
        }
    }
}

/// Puts `new` at the pointer `at`, or removes the element there; a missing field or end is added.
fn put(
    root: &mut Value,
    at: &str,
    new: Option<Value>,
) {
    let Some((parent, key)) = at.rsplit_once('/') else {
        *root = new.unwrap_or_default();
        return;
    };

    match (root.pointer_mut(parent), new) {
        (Some(Value::Object(fields)), Some(new)) => {
            fields.insert(key.to_owned(), new);
        }
        (Some(Value::Array(items)), new) => {
            if let Ok(index) = key.parse::<usize>()
                && index <= items.len()
            {
                // none, one or both of an element out and a value in
                let end = items
                    .len()
                    .min(index + 1);
                items.splice(index..end, new);
            }
        }
        _ => {}
    }
}

/// The number that `typed` writes exactly; `1.` or `-` is none yet.
fn number(typed: &str) -> Option<Number> {
    typed
        .parse::<Number>()
        .ok()
        .filter(|number| number.to_string() == typed)
}

fn small(
    label: &str,
    message: Message,
) -> impl Widget<Message> {
    button(label)
        .padding([5, 8])
        .style(button::text)
        .on_press(message)
}

#[cfg(test)]
#[path = "../../tests/unit/form.rs"]
mod tests;
