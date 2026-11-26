//! Names: what stands for a named expression or value in JSON, and what takes its place in a build.

use std::fmt;
use std::sync::LazyLock;

use polars::prelude::*;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Number, Value};

use crate::engine::{Pipeline, guard};
use crate::registry::Entry;
use crate::{Registry, schema};

/// What a reference names: an expression, a value, or an argument of the function it stands in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Expression,
    Value,
    Argument,
}

impl Kind {
    /// The key that a reference of this kind holds its name under.
    fn key(self) -> &'static str {
        match self {
            Self::Expression => "$expr",
            Self::Value => "$value",
            Self::Argument => "$arg",
        }
    }

    /// The kind whose key `fields` hold, if they hold one.
    fn of(fields: &Map<String, Value>) -> Option<Self> {
        [
            Self::Expression,
            Self::Value,
            Self::Argument,
        ]
        .into_iter()
        .find(|kind| fields.contains_key(kind.key()))
    }
}

/// A reference as JSON writes it: `{"$expr": name}`, `{"$value": name}` or `{"$arg": name}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference<'a> {
    pub kind: Kind,
    pub name: &'a str,
    /// The arguments of a call of an expression that has holes, each by the name of its hole.
    pub with: Option<&'a Map<String, Value>>,
    /// The JSON pointer to the part of a value or an argument that is meant: empty for all of it.
    pub at: &'a str,
    /// What of a table is meant in place of the table: `keys` or `values`.
    pub part: Option<&'a str>,
}

impl<'a> Reference<'a> {
    /// A reference to all of what is called `name`.
    pub fn to(
        kind: Kind,
        name: &'a str,
    ) -> Self {
        Self {
            kind,
            name,
            with: None,
            at: "",
            part: None,
        }
    }
}

impl From<&Reference<'_>> for Value {
    /// The reference as JSON writes it, which `reference` reads again.
    fn from(reference: &Reference) -> Self {
        let mut fields = Map::new();
        let key = reference.kind.key();
        fields.insert(
            key.to_owned(),
            reference
                .name
                .into(),
        );
        if let Some(with) = reference.with {
            fields.insert("with".to_owned(), with.clone().into());
        }
        if !reference
            .at
            .is_empty()
        {
            fields.insert("at".to_owned(), reference.at.into());
        }
        if let Some(part) = reference.part {
            fields.insert("part".to_owned(), part.into());
        }
        Value::Object(fields)
    }
}

impl fmt::Display for Reference<'_> {
    /// As it is written in an expression, without arguments: `@name`, `@name.keys`, `$hole.field`.
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let sign = match self.kind {
            Kind::Argument => '$',
            Kind::Expression | Kind::Value => '@',
        };
        write!(f, "{sign}{}", self.name)?;
        for step in steps(self.at) {
            match step.parse::<usize>() {
                Ok(index) => write!(f, "[{index}]")?,
                Err(_) => write!(f, ".{step}")?,
            }
        }
        self.part
            .map_or(Ok(()), |part| write!(f, ".{part}"))
    }
}

/// The keys and places that the JSON pointer `at` steps through.
fn steps(at: &str) -> impl Iterator<Item = String> + '_ {
    at.split('/')
        .skip(1)
        .map(|step| {
            step.replace("~1", "/")
                .replace("~0", "~")
        })
}

/// The reference that `value` is, if it is one: with anything its kind does not take, it is none.
pub fn reference(value: &Value) -> Option<Reference<'_>> {
    let fields = value.as_object()?;
    let kind = Kind::of(fields)?;
    let taken: &[&str] = match kind {
        Kind::Expression => &["with"],
        Kind::Value => &["at", "part"],
        Kind::Argument => &["at"],
    };
    let text = |key: &str| {
        fields
            .get(key)
            .and_then(Value::as_str)
    };
    let is_reference = fields
        .iter()
        .all(|(key, field)| match key.as_str() {
            "with" => taken.contains(&"with") && field.is_object(),
            key => (key == kind.key() || taken.contains(&key)) && field.is_string(),
        });
    Some(Reference {
        kind,
        name: text(kind.key()).filter(|_| is_reference)?,
        with: fields
            .get("with")
            .and_then(Value::as_object),
        at: text("at").unwrap_or_default(),
        part: text("part"),
    })
}

/// What a hole of a function takes, by where it stands in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Taken {
    Expression,
    /// A name, as that of a column, or another text.
    Text,
    Number,
    Flag,
    /// Any JSON: it stands under a literal, has a part of it meant, or is passed on to a call.
    Any,
}

impl Taken {
    /// What the place at the JSON pointer `at` of an expression of Polars takes.
    fn at(at: &str) -> Self {
        /// The definitions of the schema of an expression of Polars.
        static DEFS: LazyLock<Value> = LazyLock::new(|| {
            let mut generator = SchemaSettings::draft2020_12().into_generator();
            generator.subschema_for::<Expr>();
            serde_json::to_value(generator.definitions()).unwrap_or_default()
        });
        let expression = &DEFS["Expr"];
        let taken = schema::pointer(&DEFS, expression, at);
        match schema::json_type(taken) {
            _ if std::ptr::eq(taken, expression) => Self::Expression,
            "string" => Self::Text,
            "number" | "integer" => Self::Number,
            "boolean" => Self::Flag,
            _ => Self::Any,
        }
    }

    /// Whether what is `given` is of this kind: an expression if Polars reads it as one.
    fn takes(
        self,
        given: &Value,
    ) -> bool {
        match self {
            Self::Expression => serde_json::from_value::<Expr>(given.clone()).is_ok(),
            Self::Text => given.is_string(),
            Self::Number => given.is_number(),
            Self::Flag => given.is_boolean(),
            Self::Any => true,
        }
    }
}

impl fmt::Display for Taken {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(match self {
            Self::Expression => "an expression",
            Self::Text => "text",
            Self::Number => "a number",
            Self::Flag => "true or false",
            Self::Any => "anything",
        })
    }
}

/// The holes of the function `body` in the order they first stand, each with what it takes.
fn holes(body: &Value) -> PolarsResult<Vec<(&str, Taken)>> {
    let mut holes: Vec<(&str, Taken)> = Vec::new();
    let mut places = vec![(body, String::new(), true)];
    while let Some((value, at, is_placed)) = places.pop() {
        match (reference(value), value) {
            (Some(hole), _) if hole.kind == Kind::Argument => {
                // a hole of which a part is meant is a table or a list, wherever it stands
                let taken = match is_placed && hole.at.is_empty() {
                    true => Taken::at(&at),
                    false => Taken::Any,
                };
                match holes
                    .iter_mut()
                    .find(|(name, _)| *name == hole.name)
                {
                    None => holes.push((hole.name, taken)),
                    Some((_, before)) if *before == Taken::Any => *before = taken,
                    Some((_, before)) if taken == Taken::Any || *before == taken => {}
                    Some((name, before)) => polars_bail!(
                        ComputeError: "`${name}` stands where {before} is taken, and where {taken}"
                    ),
                }
            }
            // an argument of a call is an expression of its own, or a hole that is passed on
            (Some(call), _) => places.extend(
                call.with
                    .into_iter()
                    .flatten()
                    .rev()
                    .map(|(_, given)| (given, String::new(), reference(given).is_none())),
            ),
            (None, Value::Array(items)) => places.extend(
                items
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(i, item)| (item, format!("{at}/{i}"), is_placed)),
            ),
            (None, Value::Object(fields)) => places.extend(
                fields
                    .iter()
                    .rev()
                    .map(|(key, field)| (field, format!("{at}/{key}"), is_placed)),
            ),
            (None, _) => {}
        }
    }
    Ok(holes)
}

/// What stands for the expression of a name: in the args of a Node, or in another expression.
pub fn marker(name: &str) -> Value {
    Value::from(&Reference::to(Kind::Expression, name))
}

/// The name of the expression that `value` stands for, if it is a marker.
pub fn marked(value: &Value) -> Option<&str> {
    reference(value)
        .filter(|reference| reference.kind == Kind::Expression)
        .map(|reference| reference.name)
}

impl Pipeline {
    /// Calls the expression or value `old` `new`, in its place and where it is named, if it can.
    pub fn rename_named(
        &mut self,
        old: &str,
        new: &str,
    ) -> bool {
        let is_taken = self
            .named(new)
            .is_some();
        let named = match self
            .expressions
            .contains_key(old)
        {
            true => &mut self.expressions,
            false => &mut self.values,
        };
        // an empty or taken name would have its users take another
        if !named.contains_key(old) || new.is_empty() || is_taken {
            return false;
        }
        *named = std::mem::take(named)
            .into_iter()
            .map(|(name, value)| match name == old {
                true => (new.to_owned(), value),
                false => (name, value),
            })
            .collect();
        let naming = self
            .nodes
            .iter_mut()
            .map(|node| &mut node.args)
            .chain(
                self.expressions
                    .values_mut(),
            )
            .chain(
                self.values
                    .values_mut(),
            );
        for value in naming {
            rename(value, old, new);
        }
        true
    }

    /// The ids of the Nodes whose args use what is named `name`, themselves or through others.
    pub fn users(
        &self,
        name: &str,
    ) -> Vec<&str> {
        self.nodes
            .iter()
            .filter(|node| uses(&node.args, name, self, &mut Vec::new()))
            .map(|node| node.id.as_str())
            .collect()
    }

    /// The expressions and values that use what is named `name`, themselves or through others.
    pub fn dependents(
        &self,
        name: &str,
    ) -> Vec<&str> {
        self.expressions
            .iter()
            .chain(&self.values)
            .filter(|(other, named)| *other != name && uses(named, name, self, &mut Vec::new()))
            .map(|(other, _)| other.as_str())
            .collect()
    }

    /// The holes of the expression `name`, each with what it takes, or why it is no sound one.
    pub fn holes(
        &self,
        name: &str,
    ) -> PolarsResult<Vec<(&str, Taken)>> {
        let body = self
            .expressions
            .get(name)
            .ok_or_else(|| polars_err!(ComputeError: "no expression is called '{name}'"))?;
        let holes = holes(body)?;
        // all that it names is there, and what it calls is given what that takes
        resolve(
            &mut body.clone(),
            self,
            Scope::Alone,
            &mut vec![name.to_owned()],
        )?;
        Ok(holes)
    }

    /// The expression or the value of a name: they share their names.
    fn named(
        &self,
        name: &str,
    ) -> Option<&Value> {
        self.expressions
            .get(name)
            .or_else(|| {
                self.values
                    .get(name)
            })
    }
}

/// Why args are not taken, and the JSON pointer to the place in them that it is about, if one.
#[derive(Debug)]
pub struct Refusal {
    pub at: Option<String>,
    pub why: PolarsError,
}

impl From<PolarsError> for Refusal {
    /// A refusal of the args as a whole.
    fn from(why: PolarsError) -> Self {
        Self { at: None, why }
    }
}

impl From<Refusal> for PolarsError {
    fn from(refusal: Refusal) -> Self {
        refusal.why
    }
}

impl Registry {
    /// `args` as a node of `block` reads them, each name replaced, or why and where they are not.
    pub fn resolved(
        &self,
        block: &str,
        args: &Value,
        pipeline: &Pipeline,
    ) -> Result<Value, Refusal> {
        let entry = self
            .get(block)
            .ok_or_else(|| polars_err!(ComputeError: "no block is called '{block}'"))?;
        let args = resolved(entry, args.clone(), pipeline)?;
        entry.parse(&args)?;
        Ok(args)
    }
}

/// `args` of a node of `entry`, each reference replaced by what it names.
pub(crate) fn resolved(
    entry: &Entry,
    mut args: Value,
    pipeline: &Pipeline,
) -> Result<Value, Refusal> {
    if has_reference(&args) {
        let (schema, defs) = entry.schema();
        replace(&mut args, "", schema, defs, pipeline)?;
    }
    Ok(args)
}

/// What stands in the place of the holes that are met.
#[derive(Clone, Copy)]
enum Scope<'a> {
    /// Nothing: a hole stands in no function, which is an error.
    Outside,
    /// Themselves: a function is looked at alone.
    Alone,
    /// What a call gives for each.
    Call(&'a Map<String, Value>),
}

/// Replaces each reference in `value` by what it names; one named again in `chain` names itself.
fn resolve(
    value: &mut Value,
    pipeline: &Pipeline,
    scope: Scope,
    chain: &mut Vec<String>,
) -> PolarsResult<()> {
    // what is plain under a literal is the literal of Polars that it is; a hole left open stays
    if let Some(named) = lifted(value).map(|named| named.to_string()) {
        let plain = &mut value["Literal"];
        resolve(plain, pipeline, scope, chain)?;
        if reference(plain).is_none() {
            *value = serde_json::to_value(literal(plain, &named)?)
                .map_err(|e| polars_err!(ComputeError: "`{named}`: {e}"))?;
        }
        return Ok(());
    }
    let Some(reference) = reference(value) else {
        // what holds the key of a reference and is none would pass for plain JSON
        let key = value
            .as_object()
            .and_then(Kind::of)
            .map(Kind::key);
        if let Some(key) = key {
            polars_bail!(ComputeError: "{value} holds `{key}`, and is no reference");
        }
        for part in parts(value) {
            resolve(part, pipeline, scope, chain)?;
        }
        return Ok(());
    };
    let (kind, name) = (
        reference.kind,
        reference
            .name
            .to_owned(),
    );
    if kind == Kind::Argument {
        let given = match scope {
            Scope::Outside => {
                polars_bail!(ComputeError: "`{reference}` is a hole, and stands in no function")
            }
            Scope::Alone => return Ok(()),
            // a call that does not give every hole is refused before
            Scope::Call(given) => given
                .get(&name)
                .unwrap_or(&Value::Null),
        };
        // a hole of a function looked at alone is passed on as it is, without parts
        *value = match crate::reference(given) {
            Some(_) => given.clone(),
            None => meant(given.clone(), &reference)?,
        };
        return Ok(());
    }
    if let Some(at) = chain
        .iter()
        .position(|named| *named == name)
    {
        // `a` uses `b`, which uses `a`
        let links: Vec<String> = chain[at + 1..]
            .iter()
            .chain([&name])
            .map(|link| format!("`{link}`"))
            .collect();
        polars_bail!(ComputeError: "`{}` uses {}", chain[at], links.join(", which uses "));
    }
    let is_value = kind == Kind::Value;
    let named = match is_value {
        true => pipeline
            .values
            .get(&name)
            .ok_or_else(|| polars_err!(ComputeError: "no value is called '{name}'")),
        false => pipeline
            .expressions
            .get(&name)
            .ok_or_else(|| polars_err!(ComputeError: "no expression is called '{name}'")),
    };
    let mut named = named?.clone();
    // the arguments of a call are resolved where the call stands, and then stand for its holes
    let mut given = reference
        .with
        .cloned()
        .unwrap_or_default();
    for argument in given.values_mut() {
        resolve(argument, pipeline, scope, chain)?;
    }
    let scope = match is_value {
        true => Scope::Outside,
        false => {
            called(&name, &named, &given)?;
            Scope::Call(&given)
        }
    };
    chain.push(name.clone());
    resolve(&mut named, pipeline, scope, chain)?;
    chain.pop();
    *value = match is_value {
        true => meant(named, &reference)?,
        // an expression whose holes are left open is not yet anything
        false if has_reference(&named) => named,
        false => match serde_json::from_value::<Expr>(named.clone()) {
            Ok(_) => named,
            Err(e) => polars_bail!(ComputeError: "`{name}` is no expression: {e}"),
        },
    };
    Ok(())
}

/// Fails unless a call of the function `name` gives each hole of its `body` what that takes.
fn called(
    name: &str,
    body: &Value,
    given: &Map<String, Value>,
) -> PolarsResult<()> {
    let holes = holes(body)?;
    for (hole, taken) in &holes {
        let given = given
            .get(*hole)
            .ok_or_else(
                || polars_err!(ComputeError: "`{name}` takes `{hole}`, which is not given"),
            )?;
        // what still holds a hole, of a function looked at alone, is not yet anything
        let is_taken = has_reference(given) || taken.takes(given);
        let said = match given {
            Value::Number(number) => format!("the number {number}"),
            Value::String(text) => format!("the text {text:?}"),
            Value::Array(_) => "a list".to_owned(),
            Value::Object(_) => "a table".to_owned(),
            plain => plain.to_string(),
        };
        polars_ensure!(
            is_taken,
            ComputeError: "`{name}`: `{hole}` is {said}, where {taken} is taken"
        );
    }
    let unknown = given
        .keys()
        .find(|key| {
            !holes
                .iter()
                .any(|(hole, _)| hole == key)
        });
    match unknown {
        Some(unknown) => polars_bail!(ComputeError: "`{name}` takes no `{unknown}`"),
        None => Ok(()),
    }
}

/// The reference to what is plain that `value` holds as a literal: `{"Literal": {"$value": name}}`.
fn lifted(value: &Value) -> Option<Reference<'_>> {
    let fields = value
        .as_object()
        .filter(|fields| fields.len() == 1)?;
    reference(fields.get("Literal")?).filter(|reference| reference.kind != Kind::Expression)
}

/// The part of what is `named` that `reference` means: all of it, or what its pointer leads to.
fn meant(
    named: Value,
    reference: &Reference,
) -> PolarsResult<Value> {
    let whole = Reference {
        at: "",
        part: None,
        ..reference.clone()
    };
    let root = whole.to_string();
    let mut found = &named;
    for step in steps(reference.at) {
        let part = match found {
            Value::Object(fields) => fields.get(&step),
            Value::Array(rows) => match step.parse::<usize>() {
                Ok(index) => rows.get(index),
                Err(_) => {
                    polars_bail!(ComputeError: "`{root}` is a list: name a row, as `{root}[0]`")
                }
            },
            _ => None,
        };
        found = part.ok_or_else(|| polars_err!(ComputeError: "there is no `{reference}`"))?;
    }
    match (reference.part, found) {
        (None, _) => Ok(found.clone()),
        (Some("keys"), Value::Object(fields)) => Ok(fields
            .keys()
            .cloned()
            .collect()),
        (Some("values"), Value::Object(fields)) => Ok(fields
            .values()
            .cloned()
            .collect()),
        (Some("keys" | "values"), _) => {
            polars_bail!(ComputeError: "there is no `{reference}`: a table alone has those")
        }
        (Some(part), _) => {
            polars_bail!(ComputeError: "`{root}` has its `keys` and its `values`, but no `{part}`")
        }
    }
}

/// The literal of Polars that `plain` is: a number, a text, a truth, nothing, or a list of them.
fn literal(
    plain: &Value,
    named: &str,
) -> PolarsResult<Expr> {
    let scalar = |plain: &Value, is_float: bool| match plain {
        Value::Null => Some(AnyValue::Null),
        Value::Bool(truth) => Some(AnyValue::Boolean(*truth)),
        Value::Number(number) if is_float => number
            .as_f64()
            .map(AnyValue::Float64),
        Value::Number(number) => number
            .as_i64()
            .map(AnyValue::Int64),
        Value::String(text) => Some(AnyValue::StringOwned(text.into())),
        _ => None,
    };
    Ok(match plain {
        Value::Object(_) if serde_json::from_value::<Expr>(plain.clone()).is_ok() => {
            polars_bail!(ComputeError: "`{named}` is an expression, which is no literal")
        }
        Value::Object(_) => polars_bail!(
            ComputeError: "`{named}` is a table, which is no literal: name its keys or its values"
        ),
        Value::Array(items) => {
            // whole numbers among numbers that are not whole are numbers like them
            let is_float = items
                .iter()
                .any(Value::is_f64);
            let items: Option<Vec<AnyValue>> = items
                .iter()
                .map(|item| scalar(item, is_float))
                .collect();
            let series = items
                .and_then(|items| Series::from_any_values(PlSmallStr::EMPTY, &items, true).ok());
            lit(series.ok_or_else(|| {
                polars_err!(ComputeError: "`{named}` is no list of one kind, which a literal takes")
            })?)
        }
        Value::String(text) => lit(text.as_str()),
        Value::Bool(truth) => lit(*truth),
        Value::Null => lit(NULL),
        Value::Number(number) => match number.as_i64() {
            Some(whole) => lit(whole),
            None => lit(number
                .as_f64()
                .unwrap_or(f64::NAN)),
        },
    })
}

/// Replaces each outermost reference in `value`, at `at` in the args: a refusal is said there.
fn replace(
    value: &mut Value,
    at: &str,
    schema: &Value,
    defs: &Value,
    pipeline: &Pipeline,
) -> Result<(), Refusal> {
    let is_named = lifted(value).is_some()
        || value
            .as_object()
            .is_some_and(|fields| Kind::of(fields).is_some());
    if !is_named {
        match value {
            Value::Array(items) => {
                for (i, item) in items
                    .iter_mut()
                    .enumerate()
                {
                    replace(item, &format!("{at}/{i}"), schema, defs, pipeline)?;
                }
            }
            Value::Object(fields) => {
                for (key, field) in fields {
                    replace(field, &format!("{at}/{key}"), schema, defs, pipeline)?;
                }
            }
            _ => {}
        }
        return Ok(());
    }
    let here = |why| Refusal {
        at: Some(at.to_owned()),
        why,
    };
    let name = marked(value).map(str::to_owned);
    resolve(value, pipeline, Scope::Outside, &mut Vec::new()).map_err(here)?;
    // where `schema` takes a number, an expression that was named stands as the number it is
    let Some(name) = name else {
        return Ok(());
    };
    let taken = schema::pointer(defs, schema, at);
    let taken = schema::optional(taken).map_or(taken, |some| schema::resolve(defs, some));
    let is_whole = match schema::json_type(taken) {
        "integer" => true,
        "number" => false,
        _ => return Ok(()),
    };
    let field = at.trim_start_matches('/');
    let number = worked_out(value).ok_or_else(|| {
        here(polars_err!(ComputeError: "`{name}` is no number, which `{field}` takes"))
    })?;
    // a whole number worked out of others may come as a float, which is whole if it stays itself
    *value = match (is_whole, number.as_f64()) {
        (true, Some(float)) if number.is_f64() => {
            let whole = float as i64;
            if whole as f64 != float {
                let why = polars_err!(
                    ComputeError: "`{name}` is no whole number, which `{field}` takes"
                );
                return Err(here(why));
            }
            Value::from(whole)
        }
        _ => Value::Number(number),
    };
    Ok(())
}

/// The one number that `expression` comes to as Polars works it out of no rows, if it is one.
fn worked_out(expression: &Value) -> Option<Number> {
    let expression: Expr = serde_json::from_value(expression.clone()).ok()?;
    // the count of the rows is a number of a frame, not of its own
    if (&expression)
        .into_iter()
        .any(|part| matches!(part, Expr::Len))
    {
        return None;
    }
    let frame = guard(|| {
        DataFrame::empty()
            .lazy()
            .select([expression.alias("number")])
            .collect()
    })
    .ok()?;
    let number = frame
        .column("number")
        .ok()?;
    if number.len() != 1 {
        return None;
    }
    match number.dtype() {
        dtype if dtype.is_integer() => Some(
            number
                .cast(&DataType::Int64)
                .ok()?
                .i64()
                .ok()?
                .get(0)?
                .into(),
        ),
        dtype if dtype.is_float() => Number::from_f64(
            number
                .cast(&DataType::Float64)
                .ok()?
                .f64()
                .ok()?
                .get(0)?,
        ),
        _ => None,
    }
}

/// The values that `value` holds, if it is a list or an object.
fn parts(value: &mut Value) -> Box<dyn Iterator<Item = &mut Value> + '_> {
    match value {
        Value::Array(items) => Box::new(items.iter_mut()),
        Value::Object(fields) => Box::new(fields.values_mut()),
        _ => Box::new(std::iter::empty()),
    }
}

/// Whether `value` holds the key of a reference anywhere.
fn has_reference(value: &Value) -> bool {
    match value {
        Value::Array(items) => items
            .iter()
            .any(has_reference),
        Value::Object(fields) => {
            Kind::of(fields).is_some()
                || fields
                    .values()
                    .any(has_reference)
        }
        _ => false,
    }
}

/// Whether `value` names `name`, itself or through what it names, each of them `seen` once.
fn uses<'a>(
    value: &'a Value,
    name: &str,
    pipeline: &'a Pipeline,
    seen: &mut Vec<&'a str>,
) -> bool {
    let through = reference(value).is_some_and(|reference| {
        reference.kind != Kind::Argument
            && (reference.name == name
                || !seen.contains(&reference.name) && {
                    seen.push(reference.name);
                    pipeline
                        .named(reference.name)
                        .is_some_and(|named| uses(named, name, pipeline, seen))
                })
    });
    // and in what it holds, as the arguments of a call
    through
        || match value {
            Value::Array(items) => items
                .iter()
                .any(|item| uses(item, name, pipeline, seen)),
            Value::Object(fields) => fields
                .values()
                .any(|field| uses(field, name, pipeline, seen)),
            _ => false,
        }
}

/// Has each reference to `old` in `value` name `new`, and mean of it what it meant.
fn rename(
    value: &mut Value,
    old: &str,
    new: &str,
) {
    let key = reference(value)
        .filter(|reference| reference.name == old && reference.kind != Kind::Argument)
        .map(|reference| reference.kind.key());
    if let Some(key) = key {
        value[key] = new.into();
    }
    for part in parts(value) {
        rename(part, old, new);
    }
}
