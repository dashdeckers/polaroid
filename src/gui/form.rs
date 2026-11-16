//! A form for a JSON value, laid out by its JSON Schema; a null is a hole to be filled.

mod chain;
mod edit;

use std::collections::BTreeSet;

use iced::widget::{button, checkbox, column, pick_list, row, text, text_input};
use iced::{Center, Element, Fill, Widget};
use serde_json::{Value, json};

pub use self::edit::Edit;
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
    Edit(Edit),
    /// Lay out the arguments of the call at this pointer, or fold them away.
    Toggle(String),
    /// Offer the methods of this namespace to call on the expression at this pointer.
    Narrow(String, &'static str),
    /// Call the Method of this index on the expression at this pointer.
    Call(String, usize),
    /// Put a call of the Method of this index at this pointer, on the receiver that is there.
    Replace(String, usize),
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
        let methods = methods::methods();
        // the pointer to the receiver of the call at `node`, from the node
        let receiver = |node: &Value| {
            methods::read(methods, node).and_then(|method| {
                method
                    .receiver
                    .clone()
            })
        };

        match message {
            Message::Edit(edit) => edit.apply(value),
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
            Message::Call(at, index) => {
                let (Some(node), Some(method)) = (value.pointer_mut(&at), methods.get(index))
                else {
                    return false;
                };
                *node = method.call(node.take());

                // what was here is the receiver now
                let inner = method
                    .receiver
                    .as_deref()
                    .unwrap_or_default();
                self.relocate(&at, Some(&at), &format!("{at}{inner}"));
                self.expand(at, method);
            }
            Message::Replace(at, index) => {
                let (Some(node), Some(method)) = (value.pointer_mut(&at), methods.get(index))
                else {
                    return false;
                };
                let old = receiver(node);
                let kept = old
                    .as_deref()
                    .and_then(|old| node.pointer_mut(old))
                    .map(Value::take)
                    .unwrap_or_default();
                *node = method.call(kept);

                // the receiver stays, if the new call has one, and the arguments go
                let from = old
                    .filter(|_| {
                        method
                            .receiver
                            .is_some()
                    })
                    .map(|old| format!("{at}{old}"));
                let inner = method
                    .receiver
                    .as_deref()
                    .unwrap_or_default();
                self.relocate(&at, from.as_deref(), &format!("{at}{inner}"));
                self.expand(at, method);
            }
            Message::Remove(at) => {
                let Some(node) = value.pointer_mut(&at) else {
                    return false;
                };
                let old = receiver(node);
                *node = old
                    .as_deref()
                    .and_then(|old| node.pointer_mut(old))
                    .map(Value::take)
                    .unwrap_or_default();

                let from = old.map(|old| format!("{at}{old}"));
                self.relocate(&at, from.as_deref(), &at);
            }
        }
        true
    }

    /// Follows a change to the node at `at`: what was at `from` is at `to`, the rest is gone.
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
                if below(&pointer, at).is_none() {
                    return Some(pointer);
                }
                let rest = below(&pointer, from?)?;
                Some(format!("{to}{rest}"))
            })
            .collect();
        self.namespace = None;
    }

    /// Lays out the arguments of a new call of `method` at `at`, to be filled next.
    fn expand(
        &mut self,
        at: String,
        method: &Method,
    ) {
        let has_arguments = !(method
            .inputs
            .is_empty()
            && method
                .options
                .is_empty());
        if has_arguments {
            self.expanded
                .insert(at);
        }
    }
}

/// The form for `value`, by `schema`; `columns` are those the value may name.
pub fn view<'a>(
    state: &'a State,
    defs: &'a Value,
    schema: &'a Value,
    value: &'a Value,
    columns: Vec<String>,
) -> impl Widget<Message> {
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
    state: &'a State,
    defs: &'a Value,
    columns: &'c [String],
}

impl<'a> Form<'a, '_> {
    /// The form for the value at the JSON pointer `at`, which may be null if the schema says so.
    fn value(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        let schema = resolve(self.defs, schema);
        let Some(inner) = optional(schema) else {
            return self.present(schema, value, at);
        };

        let null = Message::Edit(Edit(at.clone(), Some(Value::Null)));
        let clear = (!value.is_null()).then(|| small("×", null));
        row![
            self.present(resolve(self.defs, inner), value, at),
            clear
        ]
        .spacing(5)
        .align_y(Center)
        .boxed()
    }

    /// The form for a value that is there, or the hole it leaves; an expression is a chain.
    fn present(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        match std::ptr::eq(schema, &self.defs["Expr"]) {
            true => self.chain(value, at),
            false => self.plain(schema, value, at),
        }
    }

    /// The form for a value by nothing but its schema.
    fn plain(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        let defs = self.defs;
        let put = {
            let at = at.clone();
            move |value: Value| Message::Edit(Edit(at.clone(), Some(value)))
        };

        if value.is_null() {
            let blank = blank(defs, schema);
            if !blank.is_null() {
                return small("set", put(blank)).boxed();
            }
        }

        let variants = variants(schema);
        if !variants.is_empty() {
            // a variant is its name, or its name around its payload
            let name = value
                .as_str()
                .or_else(|| {
                    let (name, _) = value
                        .as_object()?
                        .iter()
                        .next()?;
                    Some(name.as_str())
                });
            let selected = variants
                .iter()
                .copied()
                .find(|(variant, _)| Some(*variant) == name);
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
                    let add = Edit(
                        format!("{at}/{}", elements.len()),
                        Some(default(defs, items)),
                    );
                    column(
                        elements
                            .iter()
                            .enumerate()
                            .map(|(i, element)| {
                                let at = format!("{at}/{i}");
                                row![
                                    small("×", Message::Edit(Edit(at.clone(), None))),
                                    self.value(items, element, at)
                                ]
                                .spacing(5)
                                .boxed()
                            }),
                    )
                    .push(small("+", Message::Edit(add)).boxed())
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
                    .on_input(move |typed| put(number(typed)))
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

/// What was typed for a number: the number, or the text while it reads as none, as `1.` or `-`.
fn number(typed: String) -> Value {
    match typed.parse::<serde_json::Number>() {
        Ok(number) if number.to_string() == typed => number.into(),
        _ => typed.into(),
    }
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
