//! Names: what stands for a named expression or value in JSON, and what takes its place in a build.

use std::fmt;

use polars::prelude::*;
use serde_json::{Map, Number, Value};

use crate::engine::{Pipeline, guard};
use crate::registry::Entry;
use crate::{Registry, schema};

/// What a reference names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Expression,
    Value,
}

impl Kind {
    /// The key that a reference of this kind holds its name under.
    fn key(self) -> &'static str {
        match self {
            Self::Expression => "$expr",
            Self::Value => "$value",
        }
    }

    /// The kind whose key `fields` hold, if they hold one.
    fn of(fields: &Map<String, Value>) -> Option<Self> {
        [Self::Expression, Self::Value]
            .into_iter()
            .find(|kind| fields.contains_key(kind.key()))
    }
}

/// A reference as JSON writes it: `{"$expr": name}`, or `{"$value": name, "at": .., "part": ..}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference<'a> {
    pub kind: Kind,
    pub name: &'a str,
    /// The JSON pointer to the part of a value that is meant: empty for all of it.
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
    /// As it is written in an expression: `@name`, `@name.field[0]`, `@name.keys`.
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "@{}", self.name)?;
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
        Kind::Expression => &[],
        Kind::Value => &["at", "part"],
    };
    let text = |key: &str| {
        fields
            .get(key)
            .and_then(Value::as_str)
    };
    let is_reference = fields
        .iter()
        .all(|(key, field)| {
            (key == kind.key() || taken.contains(&key.as_str())) && field.is_string()
        });
    Some(Reference {
        kind,
        name: text(kind.key()).filter(|_| is_reference)?,
        at: text("at").unwrap_or_default(),
        part: text("part"),
    })
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

impl Registry {
    /// `args` as a node of `block` reads them, each name replaced, or why it does not take them.
    pub fn resolved(
        &self,
        block: &str,
        args: &Value,
        pipeline: &Pipeline,
    ) -> PolarsResult<Value> {
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
) -> PolarsResult<Value> {
    if !has_reference(&args) {
        return Ok(args);
    }
    let (schema, defs) = entry.schema();
    numbered(&mut args, "", &schema, &defs, pipeline)?;
    resolve(&mut args, pipeline, &mut Vec::new())?;
    Ok(args)
}

/// Replaces each reference in `value` by what it names; one named again in `chain` names itself.
fn resolve(
    value: &mut Value,
    pipeline: &Pipeline,
    chain: &mut Vec<String>,
) -> PolarsResult<()> {
    // a value under a literal is the literal of Polars that it is
    if let Some(named) = lifted(value).map(|named| named.to_string()) {
        let plain = &mut value["Literal"];
        resolve(plain, pipeline, chain)?;
        *value = serde_json::to_value(literal(plain, &named)?)
            .map_err(|e| polars_err!(ComputeError: "`{named}`: {e}"))?;
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
            resolve(part, pipeline, chain)?;
        }
        return Ok(());
    };
    let (kind, name) = (
        reference.kind,
        reference
            .name
            .to_owned(),
    );
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
    let named = match kind {
        Kind::Expression => pipeline
            .expressions
            .get(&name)
            .ok_or_else(|| polars_err!(ComputeError: "no expression is called '{name}'")),
        Kind::Value => pipeline
            .values
            .get(&name)
            .ok_or_else(|| polars_err!(ComputeError: "no value is called '{name}'")),
    };
    let mut named = named?.clone();
    chain.push(name.clone());
    resolve(&mut named, pipeline, chain)?;
    chain.pop();
    *value = match kind {
        Kind::Value => meant(named, &reference)?,
        Kind::Expression => match serde_json::from_value::<Expr>(named.clone()) {
            Ok(_) => named,
            Err(e) => polars_bail!(ComputeError: "`{name}` is no expression: {e}"),
        },
    };
    Ok(())
}

/// The reference to a value that `value` holds as a literal: `{"Literal": {"$value": name}}`.
fn lifted(value: &Value) -> Option<Reference<'_>> {
    let fields = value
        .as_object()
        .filter(|fields| fields.len() == 1)?;
    reference(fields.get("Literal")?).filter(|reference| reference.kind == Kind::Value)
}

/// The part of what is `named` that `reference` means: all of it, or what its pointer leads to.
fn meant(
    named: Value,
    reference: &Reference,
) -> PolarsResult<Value> {
    let root = format!("@{}", reference.name);
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

/// Puts its number in the place of each expression named where `schema` takes a number.
fn numbered(
    value: &mut Value,
    at: &str,
    schema: &Value,
    defs: &Value,
    pipeline: &Pipeline,
) -> PolarsResult<()> {
    let Some(name) = marked(value).map(str::to_owned) else {
        match value {
            Value::Array(items) => {
                for (i, item) in items
                    .iter_mut()
                    .enumerate()
                {
                    numbered(item, &format!("{at}/{i}"), schema, defs, pipeline)?;
                }
            }
            Value::Object(fields) => {
                for (key, field) in fields {
                    numbered(field, &format!("{at}/{key}"), schema, defs, pipeline)?;
                }
            }
            _ => {}
        }
        return Ok(());
    };
    let taken = schema::pointer(defs, schema, at);
    let taken = schema::optional(taken).map_or(taken, |some| schema::resolve(defs, some));
    let is_whole = match schema::json_type(taken) {
        "integer" => true,
        "number" => false,
        _ => return Ok(()),
    };
    resolve(value, pipeline, &mut Vec::new())?;
    let field = at.trim_start_matches('/');
    let number = worked_out(value)
        .ok_or_else(|| polars_err!(ComputeError: "`{name}` is no number, which `{field}` takes"))?;
    // a whole number worked out of others may come as a float, which is whole if it stays itself
    *value = match (is_whole, number.as_f64()) {
        (true, Some(float)) if number.is_f64() => {
            let whole = float as i64;
            polars_ensure!(
                whole as f64 == float,
                ComputeError: "`{name}` is no whole number, which `{field}` takes"
            );
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
    match (reference(value), value) {
        (Some(reference), _) if reference.name == name => true,
        (Some(reference), _) if seen.contains(&reference.name) => false,
        (Some(reference), _) => {
            seen.push(reference.name);
            pipeline
                .named(reference.name)
                .is_some_and(|named| uses(named, name, pipeline, seen))
        }
        (None, Value::Array(items)) => items
            .iter()
            .any(|item| uses(item, name, pipeline, seen)),
        (None, Value::Object(fields)) => fields
            .values()
            .any(|field| uses(field, name, pipeline, seen)),
        (None, _) => false,
    }
}

/// Has each reference to `old` in `value` name `new`, and mean of it what it meant.
fn rename(
    value: &mut Value,
    old: &str,
    new: &str,
) {
    let key = reference(value)
        .filter(|reference| reference.name == old)
        .map(|reference| reference.kind.key());
    if let Some(key) = key {
        value[key] = new.into();
        return;
    }
    for part in parts(value) {
        rename(part, old, new);
    }
}
