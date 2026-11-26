//! A form for a JSON value, laid out by its JSON Schema; a null is a hole to be filled.

mod chain;
mod named;
mod table;

use std::cell::OnceCell;
use std::rc::Rc;
use std::sync::LazyLock;

use iced::widget::{column, row};
use iced::{Center, Element, Function, Widget};
use polaroid_iced::theme::{Tone, font, spacing};
use serde_json::{Value, json};

use super::look::{
    Help,
    add,
    aside,
    clear,
    field,
    fitted,
    label_width,
    labelled,
    note,
    pick,
    scrubbed,
    small,
    tick,
    told,
};
use crate::schema::{blank, default, json_type, nudged, optional, read, resolve, variants};
use crate::{Pipeline, methods, reference};

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
    /// Put the number that this text writes at a JSON pointer, or none if it writes none.
    Type(String, String),
    /// Take what is at a JSON pointer out of its list or table.
    Drop(String),
    /// What the chain of an expression in it asks for.
    Chain(chain::Message),
    /// Put a value at a JSON pointer in what is named this in the pipeline.
    Name(String, String, Value),
}

/// Whether the owner of a value takes it as it is edited, or why not in a line: the core says.
pub type Check<'a> = Rc<dyn Fn(&Value) -> Result<(), String> + 'a>;

/// A value as it is edited that its owner does not take yet.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// The value that it is an edit of, which it holds for no longer than that stays.
    of: Value,
    value: Value,
    why: String,
}

impl Draft {
    /// What a form shows of `value`: it as `draft` has it edited, and why that is not taken.
    pub fn shown<'a>(
        draft: Option<&'a Self>,
        value: &'a Value,
    ) -> (&'a Value, Option<&'a str>) {
        match draft.filter(|draft| draft.of == *value) {
            Some(draft) => (&draft.value, Some(&draft.why)),
            None => (value, None),
        }
    }
}

/// What an update of a form leaves its owner to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Nothing,
    /// The value is this now, and no draft of it is left.
    Value(Value),
    /// The value stays, and this edit of it waits until it is taken.
    Draft(Draft),
    /// What is named this in the pipeline holds this value at this pointer.
    Named(String, String, Value),
}

impl State {
    /// Does what `message` asks to `value` as `draft` has it edited; `check` says if it is taken.
    pub fn update(
        &mut self,
        message: Message,
        value: &Value,
        draft: Option<&Draft>,
        check: &dyn Fn(&Value) -> Result<(), String>,
    ) -> Change {
        // a number is being typed for as long as nothing else is done
        self.typing = None;
        let (shown, _) = Draft::shown(draft, value);
        let mut edited = shown.clone();
        match message {
            Message::Name(name, at, new) => return Change::Named(name, at, new),
            Message::Put(at, new) => put(&mut edited, &at, Some(new)),
            Message::Type(at, typed) => {
                let new = read(&typed).map_or(Value::Null, Value::from);
                put(&mut edited, &at, Some(new));
                self.typing = Some((at, typed));
            }
            Message::Drop(at) => {
                put(&mut edited, &at, None);
                // the later elements move down, away from what the chains under the list show
                let list = at
                    .rsplit_once('/')
                    .map_or("", |(list, _)| list);
                self.chain
                    .relocate(list, None, "");
            }
            Message::Chain(message) => self
                .chain
                .update(message, &mut edited),
        }
        // the form may only show its value another way
        if edited == *shown {
            return Change::Nothing;
        }
        // its owner takes it once the core does, or if the core takes what it has no more
        match check(&edited) {
            Err(why) if edited != *value && check(value).is_ok() => Change::Draft(Draft {
                of: value.clone(),
                value: edited,
                why,
            }),
            _ => Change::Value(edited),
        }
    }
}

/// How a form shows the expressions in its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expressions {
    /// Each is picked by its name, and is not edited here.
    Named,
    /// Each is edited here as a chain of calls, which may have holes.
    Chained,
}

/// A name that a field may hold, as it is written and as the JSON it is.
type Offer = Rc<(String, Value)>;

/// The names that the fields of a form may hold, once a field has listed them.
pub type Offers = OnceCell<Vec<Offer>>;

/// A form as it is laid out: what every part of it is laid out with.
#[derive(Clone, Copy)]
pub struct Form<'a, 'c> {
    pub state: &'c State,
    pub defs: &'a Value,
    /// The pipeline, whose expressions and values a field may name.
    pub pipeline: &'a Pipeline,
    /// Those names, which are listed once for all its fields.
    pub offers: &'c Offers,
    /// The columns that are there to name: none until a Check found them.
    pub columns: &'c [String],
    pub expressions: Expressions,
    /// What says whether the whole value is taken, as with a name put in it.
    pub check: &'c Check<'a>,
    /// The whole value that it shows, and why that is not taken if it is a draft.
    pub value: &'a Value,
    pub held: Option<&'a str>,
    /// The least width of the column of the names of the arguments.
    pub label_width: f32,
    pub help: Help,
}

/// Why there are no columns to pick from.
const UNKNOWN: &str = "The columns are known once a Check found them: Check, then pick one.";

/// The kinds of plain JSON that a value of any is picked as.
const KINDS: [&str; 5] = [
    "number", "text", "flag", "list", "table",
];

/// How far the arguments of a call are set in from it, and a call from the root of its chain.
const INDENT: f32 = spacing::L;

/// The schemas of a text, a number, a whole number and a flag, and that of any JSON.
static TEXT: LazyLock<Value> = LazyLock::new(|| json!({ "type": "string" }));
static NUMBER: LazyLock<Value> = LazyLock::new(|| json!({ "type": "number" }));
static WHOLE: LazyLock<Value> = LazyLock::new(|| json!({ "type": "integer" }));
static FLAG: LazyLock<Value> = LazyLock::new(|| json!({ "type": "boolean" }));
pub static ANY: Value = Value::Null;

/// How `value` stands, by the default of its schema and whether that takes a null.
fn tone_of(
    value: &Value,
    default: Option<&Value>,
    is_nullable: bool,
) -> Tone {
    match value {
        _ if default == Some(value) => Tone::Default,
        // nothing where something has to be, as a null may be what is meant
        Value::Null if !is_nullable => Tone::Refused,
        _ => Tone::Given,
    }
}

impl<'a> Form<'a, '_> {
    /// The form for its value, by `schema`, over why that is not taken if no field says it.
    pub fn view(
        self,
        schema: &'a Value,
    ) -> impl Widget<Message> + use<'a> {
        let fields = resolve(self.defs, schema)["properties"].as_object();
        // a field that it names in backticks says it
        let is_said = |why: &str| {
            fields
                .into_iter()
                .flatten()
                .any(|(name, _)| says(why, name))
        };
        let waits = self
            .held
            .filter(|why| !is_said(why))
            .map(|why| {
                note(
                    format!("Not applied until it is whole: {why}"),
                    Tone::Unsaved,
                )
            });
        column![
            self.value(schema, self.value, String::new()),
            waits
        ]
        .spacing(spacing::XS)
    }

    /// The form for the value at the pointer `at`: a name, an expression, or by its schema.
    fn value(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        self.placed(schema, value, at, false)
    }

    /// As [`Self::value`]; any JSON `in_row` of a list or table is taken out there, not cleared.
    fn placed(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
        in_row: bool,
    ) -> Element<'a, Message> {
        // a default beside a `$ref` is the field's own
        let default = schema.get("default");
        let schema = resolve(self.defs, schema);
        let default = default.or(schema.get("default"));
        let inner = optional(schema).map(|inner| resolve(self.defs, inner));
        let shown = inner.unwrap_or(schema);
        // any JSON may be none, and a field left out is its default, shown in grey
        let is_free = is_any(shown);
        let is_nullable = inner.is_some() || is_free;
        let value = match (value, default) {
            (Value::Null, Some(default)) if !is_nullable => default,
            _ => value,
        };
        let is_named = reference(value).is_some();
        let can_clear = is_named || is_nullable && !(is_free && in_row);
        let clear = (can_clear && !value.is_null()).then(|| {
            let clear = clear(Message::Put(at.clone(), Value::Null));
            self.help
                .of(clear, "Clears the value, to give another or leave it out.")
        });
        // why the whole is not taken is said at the field that it names
        let why = self
            .held
            .filter(|why| !at.is_empty() && says(why, &at[1..]));
        let tone = match why {
            Some(_) => Tone::Refused,
            None => tone_of(value, default, is_nullable),
        };

        // a schema that resolves to the one definition of an expression is that very value
        let is_expression = std::ptr::eq(shown, &self.defs["Expr"]);
        let is_column = std::ptr::eq(shown, &self.defs["ColumnName"]);
        let named = |placeholder| self.names(shown, value, &at, placeholder, tone);
        let form = match self.expressions {
            _ if is_named => self.named(shown, value, at.clone(), tone),
            Expressions::Chained if is_expression => self.chain(value, at.clone()),
            Expressions::Named if is_expression => {
                // one that has no name is written as it is, until a name takes its place
                let written =
                    (!value.is_null()).then(|| aside(methods::text(value)).font(font::MONO));
                row![named("expression…"), written]
                    .spacing(spacing::S)
                    .align_y(Center)
                    .boxed()
            }
            // the whole of what a form is for is not a name, unless it is any JSON
            _ if at.is_empty() && !is_free => self.plain(shown, value, at.clone(), tone),
            _ => {
                let field = match is_column {
                    true => self.column(value, at.clone(), tone),
                    false => self.plain(shown, value, at.clone(), tone),
                };
                // what can be cleared, or emptied as a list or a table, is named once it is
                let is_given = match value {
                    Value::Null => false,
                    Value::Array(items) => !items.is_empty(),
                    Value::Object(fields) => !fields.is_empty(),
                    _ => is_nullable,
                };
                let named = (!is_given)
                    .then(|| named("@…"))
                    .flatten();
                row![field, named]
                    .spacing(spacing::XS)
                    .align_y(Center)
                    .boxed()
            }
        };
        // the same widgets whatever it holds, so that what has the focus keeps it
        row![
            form,
            clear,
            why.map(|why| note(why, Tone::Refused))
        ]
        .spacing(spacing::XS)
        .align_y(Center)
        .boxed()
    }

    /// The name of a column at the pointer `at`, to pick another of the columns that are read by.
    fn column(
        self,
        value: &Value,
        at: String,
        tone: Tone,
    ) -> Element<'a, Message> {
        // one that is not among them is shown all the same
        let name = value
            .as_str()
            .map(str::to_owned);
        let columns = self
            .columns
            .to_vec();
        let pick = pick(name, columns, String::clone, "column…", font::SANS, tone)
            .on_select(move |name: String| Message::Put(at.clone(), name.into()));
        // none to pick from until a Check finds them, which it says
        match self
            .columns
            .is_empty()
        {
            true => told(pick, Some(UNKNOWN.to_owned())),
            false => pick.stepped(),
        }
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
                return self
                    .help
                    .of(small("set", put(blank)), "Gives it a value, to fill in.");
            }
        }
        let variants = variants(schema);
        if !variants.is_empty() {
            return self.variant(variants, value, at, tone);
        }

        // any JSON is laid out as what it is, and a number being typed stays one
        let kind = match (json_type(schema), value) {
            ("", _)
                if self
                    .typing(&at)
                    .is_some() =>
            {
                "number"
            }
            ("", Value::Bool(_)) => "boolean",
            ("", Value::Number(_)) => "number",
            ("", Value::String(_)) => "string",
            ("", Value::Array(_)) => "array",
            ("", Value::Object(_)) => "object",
            (kind, _) => kind,
        };
        match kind {
            "object" if schema["properties"].is_object() => self.fields(schema, value, &at),
            "object" => self.table(&schema["additionalProperties"], value, &at),
            "array" => match schema["prefixItems"].as_array() {
                Some(items) => row(items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| self.value(item, &value[i], format!("{at}/{i}"))))
                .spacing(spacing::S)
                .boxed(),
                None => self.list(&schema["items"], value, &at),
            },
            // none yet is an empty box, which a press ticks
            "boolean" => tick(
                value
                    .as_bool()
                    .unwrap_or_default(),
                tone,
                move |on| put(on.into()),
            ),
            "integer" | "number" => self.typed(schema, value, at, tone),
            "string" => field(
                "",
                value
                    .as_str()
                    .unwrap_or_default(),
                font::SANS,
                tone,
            )
            .on_input(move |typed| put(typed.into()))
            .boxed(),
            // none of any JSON yet: its kind is picked, and it starts empty
            _ => pick(
                None,
                KINDS.to_vec(),
                ToString::to_string,
                "value…",
                font::SANS,
                tone,
            )
            .on_select(move |kind| {
                put(match kind {
                    "number" => json!(0),
                    "text" => json!(""),
                    "flag" => json!(false),
                    "list" => json!([]),
                    _ => json!({}),
                })
            })
            .boxed(),
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
        let worded = |(name, _): &(&str, Option<&Value>)| worded(name);
        let pick = pick(selected, variants, worded, "pick…", font::SANS, tone)
            .on_select(move |(name, payload)| {
                put(match payload {
                    Some(payload) => json!({ name: default(defs, payload) }),
                    None => name.into(),
                })
            })
            .stepped();

        match selected {
            Some((name, Some(payload))) => column![
                pick,
                self.value(payload, &value[name], format!("{at}/{name}"))
            ]
            .spacing(spacing::XS)
            .boxed(),
            _ => pick,
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
        let wide = label_width(
            fields
                .clone()
                .map(|(name, _)| name.as_str()),
        )
        .max(self.label_width);
        let nested = Form {
            label_width: 0.0,
            ..self
        };
        column(fields.map(|(name, field)| {
            let described = field["description"].as_str();
            let field = nested.value(field, &value[name], format!("{at}/{name}"));
            labelled(name, wide, described, field)
        }))
        .spacing(spacing::XS)
        .boxed()
    }

    /// The elements of a list of any length, each to take away, over a button to add one.
    fn list(
        self,
        items: &'a Value,
        value: &'a Value,
        at: &str,
    ) -> Element<'a, Message> {
        if let Some(grid) = self.grid(items, value, at) {
            return grid;
        }
        let elements = value
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let end = format!("{at}/{}", elements.len());
        let more = Message::Put(end, default(self.defs, items));
        let lines = elements
            .iter()
            .enumerate()
            .map(|(i, element)| {
                let at = format!("{at}/{i}");
                row![
                    self.help.of(
                        clear(Message::Drop(at.clone())),
                        "Takes this one out of the list."
                    ),
                    self.placed(items, element, at, true)
                ]
                .spacing(spacing::XS)
                .boxed()
            });
        column(lines)
            .push(
                self.help
                    .of(add(more), "Adds one to the list."),
            )
            .spacing(spacing::XS)
            .boxed()
    }

    /// The text of the number that is being typed at the pointer `at`, if one is.
    fn typing(
        self,
        at: &str,
    ) -> Option<String> {
        self.state
            .typing
            .as_ref()
            .filter(|(pointer, _)| pointer == at)
            .map(|(_, typed)| typed.clone())
    }

    /// A field for a number: as it is being typed there, or else as the number that it is.
    fn typed(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
        tone: Tone,
    ) -> Element<'a, Message> {
        let typing = self.typing(&at);
        // text that writes no number is none, and shown as one that is missing
        let tone = match &typing {
            Some(typed) if !typed.is_empty() && read(typed).is_none() => Tone::Refused,
            _ => tone,
        };
        let typed = typing.unwrap_or_else(|| match value {
            // none yet is an empty field, to type one in
            Value::Null => String::new(),
            number => number.to_string(),
        });
        // as wide as its text, beside a handle that moves the number as a slider would
        let field = fitted("", typed, font::SANS, tone).on_input(Message::Type.with(at.clone()));
        let schema = stepped(schema, value);
        let moved = move |steps| Message::Put(at.clone(), nudged(schema, value, steps));
        scrubbed(field, Some(moved), self.help)
    }
}

/// Whether `schema`, with its `$ref` followed, takes any JSON.
fn is_any(schema: &Value) -> bool {
    json_type(schema).is_empty() && variants(schema).is_empty()
}

/// The schema that a number is moved by: that of its field, or of any JSON what the number is.
fn stepped<'a>(
    schema: &'a Value,
    number: &Value,
) -> &'a Value {
    match (json_type(schema), number.is_f64()) {
        ("integer" | "number", _) => schema,
        (_, true) => &*NUMBER,
        (_, false) => &*WHOLE,
    }
}

/// The name of a variant in lower case words, however it is written: `TopRight` as `top right`.
fn worded(name: &str) -> String {
    let mut worded = String::new();
    for (i, letter) in name
        .chars()
        .enumerate()
    {
        if letter.is_uppercase() && i > 0 {
            worded.push(' ');
        }
        worded.extend(letter.to_lowercase());
    }
    worded
}

/// A key as a part of a JSON pointer.
fn escaped(key: &str) -> String {
    key.replace('~', "~0")
        .replace('/', "~1")
}

/// Whether `why` names the field `name`, as the core does in backticks.
fn says(
    why: &str,
    name: &str,
) -> bool {
    why.contains(&format!("`{name}`"))
}

/// Why something is refused, in the first line that says it.
pub fn line(why: &impl std::fmt::Display) -> String {
    why.to_string()
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Puts `new` at the pointer `at`, or takes out what is there; what holds it is made if missing.
pub fn put(
    root: &mut Value,
    at: &str,
    new: Option<Value>,
) {
    let Some((parent, key)) = at.rsplit_once('/') else {
        *root = new.unwrap_or_default();
        return;
    };
    let key = key
        .replace("~1", "/")
        .replace("~0", "~");
    let is_missing = root
        .pointer(parent)
        .is_none_or(Value::is_null);
    if is_missing && new.is_some() {
        let empty = match key.parse::<usize>() {
            Ok(_) => json!([]),
            Err(_) => json!({}),
        };
        put(root, parent, Some(empty));
    }

    match (root.pointer_mut(parent), new) {
        (Some(Value::Object(fields)), Some(new)) => {
            fields.insert(key, new);
        }
        (Some(Value::Object(fields)), None) => {
            fields.shift_remove(&key);
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

#[cfg(test)]
#[path = "../../tests/unit/form.rs"]
mod tests;
