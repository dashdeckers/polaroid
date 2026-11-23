//! A form for a JSON value, laid out by its JSON Schema; a null is a hole to be filled.

mod chain;

use std::collections::BTreeSet;

use iced::advanced::graphics::text::Paragraph;
use iced::advanced::text::{Paragraph as _, Text};
use iced::alignment::Vertical;
use iced::widget::{
    button,
    checkbox,
    column,
    container,
    pick_list,
    row,
    text,
    text_input,
    tooltip,
};
use iced::{Center, Element, Fill, Font, Function, Size, Theme, Widget};
use serde_json::{Map, Value, json};

use crate::methods::{self, Method};
use crate::schema::{blank, default, json_type, number, optional, resolve, variants};
use crate::{marked, marker};

/// What a form shows that its value does not hold.
#[derive(Debug, Default)]
pub struct State {
    /// The calls whose arguments are laid out, by the JSON pointers to their nodes.
    expanded: BTreeSet<String>,
    /// The namespace whose methods are offered, and the pointer to the expression to call on.
    namespace: Option<(String, &'static str)>,
    /// The pointer to the number being typed, and its text, which may be no number yet.
    typing: Option<(String, String)>,
}

/// What a form asks for.
#[derive(Debug, Clone)]
pub enum Message {
    /// Put a value at a JSON pointer.
    Put(String, Value),
    /// Put the number that this text writes at a JSON pointer, or none if it writes none yet.
    Type(String, String),
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
        // a number is being typed for as long as nothing else is done
        self.typing = None;
        match message {
            Message::Put(at, new) => put(value, &at, Some(new)),
            Message::Type(at, typed) => {
                let new = number(&typed).map_or(Value::Null, Value::from);
                let is_changed = value
                    .pointer(&at)
                    .is_none_or(|old| *old != new);
                put(value, &at, Some(new));
                self.typing = Some((at, typed));
                return is_changed;
            }
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
///
/// With `named`, an expression is picked from those by its name, and is not edited here.
/// The names of its arguments are in a column of the width `names` at the least.
pub fn view<'a>(
    state: &State,
    defs: &'a Value,
    schema: &'a Value,
    value: &'a Value,
    columns: Vec<String>,
    named: Option<&'a Map<String, Value>>,
    names: f32,
) -> impl Widget<Message> + use<'a> {
    let form = Form {
        state,
        defs,
        columns: &columns,
        named,
        names,
    };
    form.value(schema, value, String::new())
}

/// A form as it is laid out: what every part of it is laid out with.
#[derive(Clone, Copy)]
struct Form<'a, 'c> {
    state: &'c State,
    defs: &'a Value,
    columns: &'c [String],
    named: Option<&'a Map<String, Value>>,
    /// The least width of the column of the names of the arguments.
    names: f32,
}

/// The size of the text of an argument.
const SIZE: f32 = 13.0;
/// The height of a line that holds a field.
const LINE: f32 = 26.0;

/// How a value stands to what its schema asks for, which tells how it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Given,
    /// It is the default, and shown in grey.
    Default,
    /// There is none where there has to be one, and it is shown in red.
    Missing,
}

impl Tone {
    /// The tone of `value`, by the default of its schema and whether that takes a null.
    fn of(
        value: &Value,
        default: Option<&Value>,
        is_nullable: bool,
    ) -> Self {
        match value {
            _ if default == Some(value) => Self::Default,
            // nothing where something has to be, as a null may be what is meant
            Value::Null if !is_nullable => Self::Missing,
            _ => Self::Given,
        }
    }
}

/// The style of a field to type in, by the tone of its value.
fn typed_in(tone: Tone) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme, status| {
        let palette = theme.palette();
        let mut style = text_input::default(theme, status);
        match tone {
            Tone::Given => {}
            Tone::Default => style.value = style.placeholder,
            Tone::Missing => {
                style.border.color = palette
                    .danger
                    .base
                    .color
            }
        }
        style
    }
}

/// The style of a box to tick, by the tone of whether it is ticked.
fn ticked(
    tone: Tone,
    is_ticked: bool,
) -> impl Fn(&Theme, checkbox::Status) -> checkbox::Style {
    move |theme, status| {
        let palette = theme.palette();
        let mut style = checkbox::primary(theme, status);
        let grey = palette
            .background
            .strong
            .color;
        match tone {
            Tone::Given => {}
            Tone::Default => {
                style.border.color = grey;
                if is_ticked {
                    style.background = grey.into();
                }
            }
            Tone::Missing => {
                style.border.color = palette
                    .danger
                    .base
                    .color
            }
        }
        style
    }
}

/// The style of a list to pick from, by the tone of what is picked.
pub fn picked(tone: Tone) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |theme, status| {
        let palette = theme.palette();
        let mut style = pick_list::default(theme, status);
        match tone {
            Tone::Given => {}
            Tone::Default => style.text_color = style.placeholder_color,
            Tone::Missing => {
                style.border.color = palette
                    .danger
                    .base
                    .color
            }
        }
        style
    }
}

/// The width of a column that holds the widest of the names of some arguments.
pub fn names<'a>(names: impl IntoIterator<Item = &'a str>) -> f32 {
    names
        .into_iter()
        .map(|name| width_of(name, SIZE))
        .fold(0.0, f32::max)
}

/// The width of a line of text of this size.
pub fn width_of(
    content: &str,
    size: f32,
) -> f32 {
    let line = Text {
        content,
        bounds: Size::INFINITE,
        size: size.into(),
        line_height: text::LineHeight::default(),
        font: Font::DEFAULT,
        align_x: text::Alignment::Default,
        align_y: Vertical::Top,
        shaping: text::Shaping::default(),
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::None,
        hint_factor: None,
    };
    Paragraph::with_text(line).min_width()
}

/// An argument on a line: its name in a column of the width `names`, then its value.
///
/// Its name tells what it is for when hovered.
pub fn labelled<'a, Message: 'a>(
    name: &'a str,
    names: f32,
    described: Option<&'a str>,
    value: impl Widget<Message> + 'a,
) -> Element<'a, Message> {
    let name = text(name)
        .size(SIZE)
        .wrapping(text::Wrapping::None)
        .ellipsis(text::Ellipsis::End);
    // level with the first line of a value of many, as a list is
    let name = container(name)
        .width(names)
        .height(LINE)
        .align_y(Center);
    let name = match described {
        Some(described) => {
            let described = container(text(described).size(12))
                .width(280)
                .padding(6)
                .style(container::bordered_box);
            tooltip(name, described)
                .position(tooltip::Position::Top)
                .boxed()
        }
        None => name.boxed(),
    };
    row![name, value]
        .spacing(8)
        .boxed()
}

/// The expression at the pointer `at` as the name of one of `named`, to pick another by.
fn name<'a>(
    named: &'a Map<String, Value>,
    value: &'a Value,
    at: String,
    tone: Tone,
) -> Element<'a, Message> {
    let name = marked(value);
    let names: Vec<&str> = named
        .keys()
        .map(String::as_str)
        .collect();
    let written = match name.and_then(|name| named.get(name)) {
        Some(expression) => methods::text(expression),
        // one that has no name is written as it is, until a name takes its place
        None if name.is_none() && !value.is_null() => methods::text(value),
        None => String::new(),
    };
    row![
        pick_list(name, names, ToString::to_string)
            .on_select(move |name: &str| Message::Put(at.clone(), marker(name)))
            .placeholder("expression…")
            .text_size(SIZE)
            .padding(4)
            .style(picked(tone)),
        // in what room is left, cut short
        text(written)
            .size(12)
            .width(Fill)
            .wrapping(text::Wrapping::None)
            .ellipsis(text::Ellipsis::End)
            .style(text::secondary),
    ]
    .spacing(8)
    .align_y(Center)
    .boxed()
}

impl<'a> Form<'a, '_> {
    /// The form for the value at the pointer `at`: an expression as a chain, a nullable to clear.
    fn value(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        // a default beside a `$ref` is the field's own
        let default = schema.get("default");
        let schema = resolve(self.defs, schema);
        let default = default.or(schema.get("default"));
        let inner = optional(schema).map(|inner| resolve(self.defs, inner));
        let clear = (inner.is_some() && !value.is_null())
            .then(|| small("×", Message::Put(at.clone(), Value::Null)));
        let tone = Tone::of(value, default, inner.is_some());

        let shown = inner.unwrap_or(schema);
        let is_expression = std::ptr::eq(shown, &self.defs["Expr"]);
        let form = match (is_expression, self.named) {
            (true, Some(named)) => name(named, value, at, tone),
            (true, None) => self.chain(value, at),
            (false, _) => self.plain(shown, value, at, tone),
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
        tone: Tone,
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
                .placeholder("pick…")
                .text_size(SIZE)
                .padding(4)
                .style(picked(tone));

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
            "object" => {
                let fields = schema["properties"]
                    .as_object()
                    .into_iter()
                    .flatten();
                // as wide as the widest of the names, and what is nested starts its own column
                let wide = names(
                    fields
                        .clone()
                        .map(|(name, _)| name.as_str()),
                )
                .max(self.names);
                let nested = Form { names: 0.0, ..self };
                column(fields.map(|(name, field)| {
                    let described = field["description"].as_str();
                    let field = nested.value(field, &value[name], format!("{at}/{name}"));
                    labelled(name, wide, described, field)
                }))
                .spacing(5)
                .boxed()
            }
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
            "boolean" => {
                // none yet is an empty box, which a press ticks
                let is_ticked = value
                    .as_bool()
                    .unwrap_or_default();
                checkbox(is_ticked)
                    .on_toggle(move |on| put(on.into()))
                    .style(ticked(tone, is_ticked))
                    .boxed()
            }
            "integer" | "number" => {
                // as it is being typed here, or else as the number that it is
                let typing = self
                    .state
                    .typing
                    .as_ref()
                    .filter(|(pointer, _)| *pointer == at)
                    .map(|(_, typed)| typed.clone());
                // text that writes no number is none, and shown as one that is missing
                let tone = match &typing {
                    Some(typed) if !typed.is_empty() && number(typed).is_none() => Tone::Missing,
                    _ => tone,
                };
                let typed = typing.unwrap_or_else(|| match value {
                    // none yet is an empty field, to type one in
                    Value::Null => String::new(),
                    number => number.to_string(),
                });
                text_input("", typed)
                    .on_input(Message::Type.with(at))
                    .width(Fill.max(100))
                    .size(SIZE)
                    .padding(4)
                    .style(typed_in(tone))
                    .boxed()
            }
            "string" => text_input(
                "",
                value
                    .as_str()
                    .unwrap_or_default(),
            )
            .on_input(move |typed| put(typed.into()))
            .size(SIZE)
            .padding(4)
            .style(typed_in(tone))
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
