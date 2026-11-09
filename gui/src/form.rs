//! A form for any JSON value from its JSON Schema, which is what the catalog
//! says of a block's args, Polars' expressions included.
//!
//! An enum is a chip naming its variant, its payload's scalars beside it and
//! its nested values indented below, so an expression reads as its tree. A
//! picked chip can be replaced, wrapped in another variant, or cleared to a
//! hole; a hole offers every variant. Every edit replaces the value at a JSON
//! pointer.

use iced::widget::{
    Column,
    Row,
    Text,
    button,
    checkbox,
    column,
    container,
    pick_list,
    row,
    rule,
    text,
    text_input,
    tooltip,
};
use iced::{Center, Element, Font, Padding, Shrink, Theme};
use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub enum Message {
    /// The value at this pointer, replaced.
    Edit(String, Value),
    /// The chip picked, at this pointer.
    Focus(Option<String>),
}

/// `value`, laid out by `schema`, whose `$ref`s point into `defs`.
pub fn view<'a>(
    value: &'a Value,
    schema: &'a Value,
    defs: &'a Value,
    focus: Option<&'a str>,
) -> Element<'a, Message> {
    let form = Form { defs, focus };
    let schema = form.resolve(schema);
    let (head, body) = match schema.get("properties") {
        // a block's args: a labelled row per field
        Some(_) => form.fields(value, schema, String::new(), false),
        None => form.view(value, schema, String::new()),
    };
    column![
        Row::with_children(head).spacing(6),
        body
    ]
    .spacing(4)
    .into()
}

/// `schema`'s default value, with a hole wherever a choice must be made.
pub fn default(
    schema: &Value,
    defs: &Value,
) -> Value {
    Form { defs, focus: None }.default(schema)
}

/// Sets the value at `pointer`, if there is one.
pub fn apply(
    root: &mut Value,
    pointer: &str,
    value: Value,
) {
    if let Some(slot) = root.pointer_mut(pointer) {
        *slot = value;
    }
}

#[derive(Clone, Copy)]
struct Form<'a> {
    defs: &'a Value,
    focus: Option<&'a str>,
}

/// The widgets beside a value's label, and what goes indented below it.
type Parts<'a> = (Vec<Element<'a, Message>>, Option<Element<'a, Message>>);

impl<'a> Form<'a> {
    fn resolve(
        self,
        schema: &'a Value,
    ) -> &'a Value {
        match schema
            .get("$ref")
            .and_then(Value::as_str)
        {
            Some(path) => self.resolve(&self.defs[path.trim_start_matches("#/$defs/")]),
            None => schema,
        }
    }

    fn view(
        self,
        value: &'a Value,
        schema: &'a Value,
        at: String,
    ) -> Parts<'a> {
        let schema = self.resolve(schema);
        let inner = optional(schema);
        let clearable = inner.is_some() || nullable(schema);
        let present = inner.unwrap_or(schema);

        if clearable && value.is_null() {
            let set = match inner {
                Some(inner) => self.default(inner),
                None => blank(kind(schema)),
            };
            // an optional enum shows its hole, to be filled directly
            if !set.is_null() {
                return (
                    vec![small_button(
                        "+ set",
                        Message::Edit(at, set),
                    )],
                    None,
                );
            }
        }
        let (mut head, body) = self.present(value, present, at.clone());
        if clearable && !value.is_null() {
            head.push(small_button("×", Message::Edit(at, Value::Null)));
        }
        (head, body)
    }

    fn present(
        self,
        value: &'a Value,
        schema: &'a Value,
        at: String,
    ) -> Parts<'a> {
        let schema = self.resolve(schema);
        if schema
            .get("oneOf")
            .is_some()
        {
            return self.choice(value, schema, at);
        }
        if let Some(options) = schema
            .get("enum")
            .and_then(Value::as_array)
        {
            let picker = picker(
                options
                    .iter()
                    .filter_map(Value::as_str),
                value,
                at,
            );
            return (vec![picker], None);
        }

        match kind(schema) {
            "object" => self.fields(value, schema, at, true),
            "array" => match schema
                .get("prefixItems")
                .and_then(Value::as_array)
            {
                Some(items) => self.tuple(value, items, at),
                None => (vec![], Some(self.list(value, &schema["items"], at))),
            },
            "boolean" => {
                let on = value
                    .as_bool()
                    .unwrap_or(false);
                let check =
                    checkbox(on).on_toggle(move |on| Message::Edit(at.clone(), Value::Bool(on)));
                (vec![check.into()], None)
            }
            "integer" | "number" => {
                let typed = match value {
                    Value::String(typed) => typed.clone(),
                    other => other.to_string(),
                };
                let input = text_input("0", &typed)
                    .on_input(move |typed| Message::Edit(at.clone(), number(typed)))
                    .size(13)
                    .width(90);
                (vec![input.into()], None)
            }
            "string" => {
                let input = text_input(
                    "",
                    value
                        .as_str()
                        .unwrap_or_default(),
                )
                .on_input(move |typed| Message::Edit(at.clone(), Value::String(typed)))
                .size(13)
                .width(150);
                (vec![input.into()], None)
            }
            // a shape the form does not know: edit it as JSON
            _ => (vec![code(value.to_string())], None),
        }
    }

    /// An enum: its variant's chip and payload, or a hole.
    fn choice(
        self,
        value: &'a Value,
        schema: &'a Value,
        at: String,
    ) -> Parts<'a> {
        let variants: Vec<&'a Value> = schema["oneOf"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| self.resolve(v))
            .collect();
        let names: Vec<String> = variants
            .iter()
            .flat_map(|v| names(v))
            .collect();

        // names alone, no payloads: a plain picker
        if variants
            .iter()
            .all(|v| {
                v.get("properties")
                    .is_none()
            })
        {
            return (
                vec![picker(
                    names
                        .iter()
                        .map(String::as_str),
                    value,
                    at,
                )],
                None,
            );
        }

        let (name, payload) = match value {
            Value::String(name) => (name.as_str(), None),
            Value::Object(map) if map.len() == 1 => {
                let (name, payload) = map
                    .iter()
                    .next()
                    .expect("one entry");
                let schema = variants
                    .iter()
                    .find_map(|v| {
                        v.get("properties")?
                            .get(name)
                    });
                (name.as_str(), schema.map(|schema| (payload, schema)))
            }
            _ => {
                let fill =
                    move |name: String| Message::Edit(at.clone(), self.variant(&variants, &name));
                let hole = pick_list(names, None::<String>, fill)
                    .placeholder("+ pick")
                    .text_size(13)
                    .style(|theme: &Theme, status| pick_list::Style {
                        placeholder_color: theme
                            .extended_palette()
                            .warning
                            .strong
                            .color,
                        ..pick_list::default(theme, status)
                    });
                return (vec![hole.into()], None);
            }
        };

        let focused = self.focus == Some(at.as_str());
        let chip = button(text(name.to_owned()).size(13))
            .padding([2, 8])
            .style(match focused {
                true => button::primary,
                false => button::secondary,
            })
            .on_press(Message::Focus((!focused).then(|| at.clone())));
        let mut head: Vec<Element<'a, Message>> = vec![chip.into()];

        // a picked chip's tools, on a line of their own below it
        let tools = focused.then(|| {
            let replace = {
                let (variants, at) = (variants.clone(), at.clone());
                move |name: String| Message::Edit(at.clone(), self.variant(&variants, &name))
            };
            let wrap = {
                let (variants, at) = (variants.clone(), at.clone());
                move |name: String| {
                    // the new variant, with this value in its first hole for it
                    let mut wrapped = self.variant(&variants, &name);
                    self.fill(&mut wrapped, schema, schema, &mut Some(value.clone()));
                    Message::Edit(at.clone(), wrapped)
                }
            };
            row![
                pick_list(names.clone(), None::<String>, replace)
                    .placeholder("replace")
                    .text_size(12)
                    .width(150),
                pick_list(names, None::<String>, wrap)
                    .placeholder("wrap in")
                    .text_size(12)
                    .width(150),
                small_button("clear", Message::Edit(at.clone(), Value::Null)),
            ]
            .spacing(6)
        });

        let body = match payload {
            None => None,
            Some((payload, schema)) => {
                let (more, body) = self.view(payload, schema, format!("{at}/{name}"));
                head.extend(more);
                body
            }
        };
        let body = match (tools, body) {
            (None, body) => body,
            (Some(tools), None) => Some(tools.into()),
            (Some(tools), Some(body)) => Some(
                column![tools, body]
                    .spacing(4)
                    .into(),
            ),
        };
        (head, body)
    }

    /// An object's fields: when `inline`, its scalars beside its chip, and
    /// the rest each on a labelled row below.
    fn fields(
        self,
        value: &'a Value,
        schema: &'a Value,
        at: String,
        inline: bool,
    ) -> Parts<'a> {
        let mut beside = vec![];
        let mut rows = Column::new().spacing(4);
        for (name, field) in schema
            .get("properties")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let Some(current) = value.get(name) else {
                // an optional field left out: offer to add it
                let mut added = value.clone();
                added[name] = self.default(field);
                rows = rows.push(small_button(
                    format!("+ {name}"),
                    Message::Edit(at.clone(), added),
                ));
                continue;
            };
            let (head, body) = self.view(current, field, format!("{at}/{name}"));
            if inline && self.scalar(field) {
                beside.push(label(name).into());
                beside.extend(head);
            } else {
                let description = self
                    .resolve(field)
                    .get("description")
                    .and_then(Value::as_str);
                rows = rows.push(layout(Some(described(name, description)), head, body));
            }
        }
        (beside, Some(rows.into()))
    }

    /// Whether values of `schema` are edited in one widget, so they fit
    /// beside a chip rather than on a row of their own.
    fn scalar(
        self,
        schema: &'a Value,
    ) -> bool {
        let schema = self.resolve(schema);
        let schema = optional(schema)
            .map(|inner| self.resolve(inner))
            .unwrap_or(schema);
        match schema
            .get("oneOf")
            .and_then(Value::as_array)
        {
            Some(variants) => variants
                .iter()
                .all(|v| {
                    self.resolve(v)
                        .get("properties")
                        .is_none()
                }),
            None => {
                schema
                    .get("enum")
                    .is_some()
                    || matches!(kind(schema), "string" | "integer" | "number" | "boolean")
            }
        }
    }

    /// A tuple: its scalars beside, the rest below by position.
    fn tuple(
        self,
        value: &'a Value,
        items: &'a [Value],
        at: String,
    ) -> Parts<'a> {
        let mut beside = vec![];
        let mut rows = Column::new().spacing(4);
        for (i, item) in items
            .iter()
            .enumerate()
        {
            let (head, body) = self.view(&value[i], item, format!("{at}/{i}"));
            match self.scalar(item) {
                true => beside.extend(head),
                false => rows = rows.push(layout(Some(label(i.to_string()).into()), head, body)),
            }
        }
        (beside, Some(rows.into()))
    }

    /// A list: a row per element, each removable, and one to add another.
    fn list(
        self,
        value: &'a Value,
        items: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        let elements = value
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut rows = Column::new().spacing(4);
        for (i, element) in elements
            .iter()
            .enumerate()
        {
            let (mut head, body) = self.view(element, items, format!("{at}/{i}"));
            let mut rest = elements.to_vec();
            rest.remove(i);
            head.push(small_button(
                "×",
                Message::Edit(at.clone(), Value::Array(rest)),
            ));
            rows = rows.push(layout(Some(label(i.to_string()).into()), head, body));
        }
        let mut more = elements.to_vec();
        more.push(self.default(items));
        rows.push(small_button("+ add", Message::Edit(at, Value::Array(more))))
            .into()
    }

    fn default(
        self,
        schema: &Value,
    ) -> Value {
        let schema = match schema
            .get("$ref")
            .and_then(Value::as_str)
        {
            Some(path) => &self.defs[path.trim_start_matches("#/$defs/")],
            None => schema,
        };
        if let Some(value) = schema
            .get("default")
            .or_else(|| schema.get("const"))
        {
            return value.clone();
        }
        if optional(schema).is_some() || nullable(schema) {
            return Value::Null;
        }
        if let Some(variants) = schema
            .get("oneOf")
            .and_then(Value::as_array)
        {
            // the first name, if names are all there is; else a hole
            let units = variants
                .iter()
                .all(|v| {
                    v.get("properties")
                        .is_none()
                });
            return match units {
                true => variants
                    .iter()
                    .flat_map(names)
                    .next()
                    .map_or(Value::Null, Value::String),
                false => Value::Null,
            };
        }
        if let Some(first) = schema
            .get("enum")
            .and_then(|options| options.get(0))
        {
            return first.clone();
        }
        match kind(schema) {
            "object" => Value::Object(
                schema
                    .get("required")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|name| (name.to_owned(), self.default(&schema["properties"][name])))
                    .collect(),
            ),
            "array" => match schema
                .get("prefixItems")
                .and_then(Value::as_array)
            {
                Some(items) => Value::Array(
                    items
                        .iter()
                        .map(|item| self.default(item))
                        .collect(),
                ),
                None => Value::Array(vec![]),
            },
            other => blank(other),
        }
    }

    /// Variant `name`, its payload defaulted.
    fn variant(
        self,
        variants: &[&Value],
        name: &str,
    ) -> Value {
        variants
            .iter()
            .find_map(|v| {
                match v
                    .get("properties")
                    .and_then(|p| p.get(name))
                {
                    Some(payload) => Some(Value::Object(Map::from_iter([(
                        name.to_owned(),
                        self.default(payload),
                    )]))),
                    None => names(v)
                        .iter()
                        .any(|n| n == name)
                        .then(|| Value::String(name.to_owned())),
                }
            })
            .unwrap_or(Value::Null)
    }

    /// Puts `with` into the first hole of `value`, whose schema is `schema`,
    /// that takes the enum `target`.
    fn fill(
        self,
        value: &mut Value,
        schema: &'a Value,
        target: &Value,
        with: &mut Option<Value>,
    ) {
        let schema = self.resolve(schema);
        let schema = optional(schema)
            .map(|inner| self.resolve(inner))
            .unwrap_or(schema);
        if with.is_none() {
            return;
        }
        if value.is_null() {
            if std::ptr::eq(schema, target) {
                *value = with
                    .take()
                    .expect("checked above");
            }
            return;
        }
        let variants = schema
            .get("oneOf")
            .and_then(Value::as_array);
        match (variants, value) {
            (Some(variants), Value::Object(map)) => {
                for (name, payload) in map {
                    if let Some(schema) = variants
                        .iter()
                        .find_map(|v| {
                            self.resolve(v)
                                .get("properties")?
                                .get(name)
                        })
                    {
                        self.fill(payload, schema, target, with);
                    }
                }
            }
            (None, Value::Object(map)) => {
                for (name, field) in map {
                    if let Some(schema) = schema
                        .get("properties")
                        .and_then(|p| p.get(name))
                    {
                        self.fill(field, schema, target, with);
                    }
                }
            }
            (None, Value::Array(elements)) => {
                for (i, element) in elements
                    .iter_mut()
                    .enumerate()
                {
                    let item = schema
                        .get("prefixItems")
                        .and_then(|items| items.get(i))
                        .unwrap_or(&schema["items"]);
                    self.fill(element, item, target, with);
                }
            }
            _ => {}
        }
    }
}

/// A labelled row, its body indented below behind a rule.
fn layout<'a>(
    label: Option<Element<'a, Message>>,
    head: Vec<Element<'a, Message>>,
    body: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let line = Row::new()
        .spacing(6)
        .align_y(Center)
        .push(label)
        .extend(head);
    match body {
        None => line.into(),
        Some(body) => column![
            line,
            // as tall as the body, which the rule then spans
            row![rule::vertical(1), body]
                .spacing(10)
                .padding(Padding::ZERO.left(12))
                .height(Shrink),
        ]
        .spacing(4)
        .into(),
    }
}

/// A field's label, its doc comment shown on hover.
fn described<'a>(
    name: &'a str,
    description: Option<&'a str>,
) -> Element<'a, Message> {
    let name = label(name).width(80);
    match description {
        None => name.into(),
        Some(description) => tooltip(
            name,
            container(text(description).size(12))
                .padding(6)
                .max_width(360)
                .style(container::rounded_box),
            tooltip::Position::Bottom,
        )
        .into(),
    }
}

fn label<'a>(name: impl text::IntoFragment<'a>) -> Text<'a> {
    text(name)
        .size(12)
        .style(|theme: &Theme| text::Style {
            color: Some(
                theme
                    .extended_palette()
                    .background
                    .strongest
                    .color,
            ),
        })
}

/// A picker of `options`, `value` among them.
fn picker<'a, 'b>(
    options: impl Iterator<Item = &'b str>,
    value: &Value,
    at: String,
) -> Element<'a, Message> {
    let options: Vec<String> = options
        .map(str::to_owned)
        .collect();
    let picked = value
        .as_str()
        .map(str::to_owned);
    pick_list(options, picked, move |o| {
        Message::Edit(at.clone(), Value::String(o))
    })
    .text_size(13)
    .into()
}

fn small_button<'a>(
    content: impl Into<String>,
    message: Message,
) -> Element<'a, Message> {
    button(text(content.into()).size(12))
        .padding([1, 6])
        .style(button::text)
        .on_press(message)
        .into()
}

fn code<'a>(content: String) -> Element<'a, Message> {
    text(content)
        .size(12)
        .font(Font::MONOSPACE)
        .into()
}

/// The non-null schema of an `Option`.
fn optional(schema: &Value) -> Option<&Value> {
    let options = schema
        .get("anyOf")?
        .as_array()?;
    let is_null = |o: &Value| o.get("type") == Some(&Value::from("null"));
    if !options
        .iter()
        .any(is_null)
    {
        return None;
    }
    options
        .iter()
        .find(|o| !is_null(o))
}

/// Whether `type` lists `null` beside another type.
fn nullable(schema: &Value) -> bool {
    schema
        .get("type")
        .and_then(Value::as_array)
        .is_some_and(|types| types.contains(&Value::from("null")))
}

/// The JSON type, `null` aside.
fn kind(schema: &Value) -> &str {
    match schema.get("type") {
        Some(Value::String(kind)) => kind,
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|kind| *kind != "null")
            .unwrap_or("null"),
        _ => "",
    }
}

/// The empty value of a JSON type.
fn blank(kind: &str) -> Value {
    match kind {
        "string" => Value::String(String::new()),
        "integer" | "number" => Value::from(0),
        "boolean" => Value::Bool(false),
        "array" => Value::Array(vec![]),
        "object" => Value::Object(Map::new()),
        _ => Value::Null,
    }
}

/// The names a variant goes by.
fn names(variant: &Value) -> Vec<String> {
    if let Some(name) = variant
        .get("const")
        .and_then(Value::as_str)
    {
        return vec![name.to_owned()];
    }
    match (variant.get("enum"), variant.get("properties")) {
        (Some(Value::Array(names)), _) => names
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        (_, Some(Value::Object(properties))) => properties
            .keys()
            .cloned()
            .collect(),
        _ => vec![],
    }
}

/// What was typed in a number field: the number, or the text while it is
/// not one yet, such as `1.` or `-`, so typing is never fought.
fn number(typed: String) -> Value {
    serde_json::from_str::<serde_json::Number>(&typed)
        .ok()
        .filter(|number| number.to_string() == typed)
        .map_or(Value::String(typed), Value::Number)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn catalog() -> Value {
        polaroid_core::Registry::with_polars().catalog()
    }

    #[test]
    fn a_number_is_kept_as_typed_until_it_reads_back_the_same() {
        assert_eq!(number("1.5".into()), json!(1.5));
        assert_eq!(number("-3".into()), json!(-3));
        assert_eq!(number("1.".into()), json!("1."));
        assert_eq!(number("-".into()), json!("-"));
    }

    #[test]
    fn a_default_leaves_a_hole_where_an_expression_goes() {
        let catalog = catalog();
        let defs = &catalog["$defs"];
        assert_eq!(
            default(&catalog["blocks"]["filter"]["args"], defs),
            json!({"predicates": []})
        );
        assert_eq!(
            default(&catalog["blocks"]["join"]["args"], defs),
            json!({"left_on": [], "right_on": [], "how": "Inner"})
        );
        assert_eq!(default(&json!({"$ref": "#/$defs/Expr"}), defs), Value::Null);
    }

    #[test]
    fn wrapping_puts_the_old_value_in_the_new_variants_hole() {
        let catalog = catalog();
        let form = Form {
            defs: &catalog["$defs"],
            focus: None,
        };
        let expr = form.resolve(&catalog["$defs"]["Expr"]);
        let variants: Vec<&Value> = expr["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| form.resolve(v))
            .collect();

        let mut wrapped = form.variant(&variants, "Alias");
        form.fill(&mut wrapped, expr, expr, &mut Some(json!({"Column": "q"})));
        assert_eq!(wrapped, json!({"Alias": [{"Column": "q"}, ""]}));

        // what the form builds is what Polars reads
        wrapped["Alias"][1] = json!("rev");
        let parsed: polaroid_core::polars::prelude::Expr = serde_json::from_value(wrapped).unwrap();
        assert_eq!(parsed.to_string(), r#"col("q").alias("rev")"#);
    }
}
