//! A form for a JSON value, laid out by its JSON Schema; a null is a hole to be filled.

mod chain;

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
use iced::{Center, Element, Fill, Font, Function, Theme, Widget};
use polaroid_iced::{scrub, width_of};
use serde_json::{Map, Value, json};

use super::{NOTE, TEXT};
use crate::schema::{blank, default, json_type, nudged, number, optional, resolve, variants};
use crate::{marked, marker, methods};

/// What a form shows that its value does not hold.
#[derive(Debug, Default)]
pub struct State {
    /// What the chains of the expressions in it show.
    chain: chain::State,
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
    /// What the chain of an expression in it asks for.
    Chain(chain::Message),
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
                // the later elements move down, away from what the chains under the array show
                let array = at
                    .rsplit_once('/')
                    .map_or("", |(array, _)| array);
                self.chain
                    .relocate(array, None, "");
            }
            Message::Chain(message) => {
                return self
                    .chain
                    .update(message, value);
            }
        }
        true
    }
}

/// How a form shows the expressions in its value.
#[derive(Clone, Copy)]
pub enum Expressions<'a, 'c> {
    /// Each is picked by its name from these, and is not edited here; these columns are read.
    Named(&'a Map<String, Value>, &'c [String]),
    /// Each is edited here as a chain of calls, which may name these columns.
    Chained(&'c [String]),
}

impl<'c> Expressions<'_, 'c> {
    /// The columns that are there to name.
    fn columns(self) -> &'c [String] {
        match self {
            Self::Named(_, columns) | Self::Chained(columns) => columns,
        }
    }
}

/// The form for `value`, by `schema`.
///
/// The names of its arguments are in a column of the width `names` at the least.
pub fn view<'a>(
    state: &State,
    defs: &'a Value,
    schema: &'a Value,
    value: &'a Value,
    expressions: Expressions<'a, '_>,
    names: f32,
) -> impl Widget<Message> + use<'a> {
    let form = Form {
        state,
        defs,
        expressions,
        names,
    };
    form.value(schema, value, String::new())
}

/// A form as it is laid out: what every part of it is laid out with.
#[derive(Clone, Copy)]
struct Form<'a, 'c> {
    state: &'c State,
    defs: &'a Value,
    expressions: Expressions<'a, 'c>,
    /// The least width of the column of the names of the arguments.
    names: f32,
}

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
        .map(|name| width_of(name, TEXT, Font::DEFAULT))
        .fold(0.0, f32::max)
}

/// A description as its paragraphs, each on one line for the text to wrap as it fits.
fn flowed(described: &str) -> String {
    described
        .split("\n\n")
        .map(|paragraph| paragraph.replace('\n', " "))
        .collect::<Vec<_>>()
        .join("\n\n")
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
        .size(TEXT)
        .wrapping(text::Wrapping::None)
        .ellipsis(text::Ellipsis::End);
    // level with the first line of a value of many, as a list is
    let name = container(name)
        .width(names)
        .height(LINE)
        .align_y(Center);
    let name = match described {
        Some(described) => {
            let described = container(text(flowed(described)).size(NOTE))
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
            .text_size(TEXT)
            .padding(4)
            .style(picked(tone)),
        // in what room is left, cut short
        text(written)
            .size(NOTE)
            .width(Fill)
            .wrapping(text::Wrapping::None)
            .ellipsis(text::Ellipsis::End)
            .style(text::secondary),
    ]
    .spacing(8)
    .align_y(Center)
    .boxed()
}

/// The name of a column at the pointer `at`, to pick another of `columns` by.
fn column_of<'a>(
    columns: &[String],
    value: &Value,
    at: String,
    tone: Tone,
) -> Element<'a, Message> {
    // one that is not among them is shown all the same
    let name = value
        .as_str()
        .map(str::to_owned);
    pick_list(name, columns.to_vec(), String::clone)
        .on_select(move |name: String| Message::Put(at.clone(), name.into()))
        .placeholder("column…")
        .text_size(TEXT)
        .padding(4)
        .style(picked(tone))
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
        // a schema that resolves to the one definition of an expression is that very value
        let is_expression = std::ptr::eq(shown, &self.defs["Expr"]);
        // and to that of the name of a column one of the columns, where they are known
        let columns = self
            .expressions
            .columns();
        let is_column = std::ptr::eq(shown, &self.defs["ColumnName"]) && !columns.is_empty();
        let form = match (is_expression, self.expressions) {
            (true, Expressions::Named(named, _)) => name(named, value, at, tone),
            (true, Expressions::Chained(_)) => self.chain(value, at),
            (false, _) if is_column => column_of(columns, value, at, tone),
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
        let put = Message::Put.with(at.clone());

        if value.is_null() {
            let blank = blank(self.defs, schema);
            if !blank.is_null() {
                return small("set", put(blank)).boxed();
            }
        }
        let variants = variants(schema);
        if !variants.is_empty() {
            return self.variant(variants, value, at, tone);
        }

        match json_type(schema) {
            "object" => self.fields(schema, value, &at),
            "array" => match schema["prefixItems"].as_array() {
                Some(items) => row(items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| self.value(item, &value[i], format!("{at}/{i}"))))
                .spacing(10)
                .boxed(),
                None => self.list(&schema["items"], value, &at),
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
            "integer" | "number" => self.typed(schema, value, at, tone),
            "string" => text_input(
                "",
                value
                    .as_str()
                    .unwrap_or_default(),
            )
            .on_input(move |typed| put(typed.into()))
            .size(TEXT)
            .padding(4)
            .style(typed_in(tone))
            .boxed(),
            // a shape the form does not know
            _ => text(value.to_string()).boxed(),
        }
    }

    /// A list to pick one of `variants` from, over the form for what the picked one holds.
    fn variant(
        self,
        variants: Vec<(&'a str, Option<&'a Value>)>,
        value: &'a Value,
        at: String,
        tone: Tone,
    ) -> Element<'a, Message> {
        let defs = self.defs;
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
        let put = Message::Put.with(at.clone());
        let pick = pick_list(selected, variants, |(name, _)| name.to_string())
            .on_select(move |(name, payload)| {
                put(match payload {
                    Some(payload) => json!({ name: default(defs, payload) }),
                    None => name.into(),
                })
            })
            .placeholder("pick…")
            .text_size(TEXT)
            .padding(4)
            .style(picked(tone));

        match selected {
            Some((name, Some(payload))) => column![
                pick,
                self.value(payload, &value[name], format!("{at}/{name}"))
            ]
            .spacing(5)
            .boxed(),
            _ => pick.boxed(),
        }
    }

    /// The fields of an object, each on a line under its name.
    fn fields(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: &str,
    ) -> Element<'a, Message> {
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

    /// The elements of an array of any length, each to take away, over a button to add one.
    fn list(
        self,
        items: &'a Value,
        value: &'a Value,
        at: &str,
    ) -> Element<'a, Message> {
        let elements = value
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let end = format!("{at}/{}", elements.len());
        let add = Message::Put(end, default(self.defs, items));
        let lines = elements
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
            });
        column(lines)
            .push(small("+", add).boxed())
            .spacing(5)
            .boxed()
    }

    /// A field for a number: as it is being typed there, or else as the number that it is.
    ///
    /// A drag across it moves the number, as a slider would.
    fn typed(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
        tone: Tone,
    ) -> Element<'a, Message> {
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
        let field = text_input("", typed)
            .on_input(Message::Type.with(at.clone()))
            .width(Fill.max(100))
            .size(TEXT)
            .padding(4)
            .style(typed_in(tone));
        scrub(field)
            .on_scrub(move |steps| Message::Put(at.clone(), nudged(schema, value, steps)))
            .boxed()
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
