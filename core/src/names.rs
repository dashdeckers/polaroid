//! Names: what stands for a named expression in JSON, and what takes its place in a build.

use polars::prelude::*;
use serde_json::{Map, Number, Value};

use crate::engine::{Pipeline, guard};
use crate::registry::Entry;
use crate::{Registry, schema};

/// What stands for the expression of a name: in the args of a Node, or in another expression.
pub fn marker(name: &str) -> Value {
    serde_json::json!({ "$expr": name })
}

/// The name of the expression that `value` stands for, if it is a marker.
pub fn marked(value: &Value) -> Option<&str> {
    value
        .as_object()
        .filter(|marker| marker.len() == 1)?
        .get("$expr")?
        .as_str()
}

impl Pipeline {
    /// Calls the expression `old` `new`, in its place and where it is named; false if it cannot.
    pub fn rename_expression(
        &mut self,
        old: &str,
        new: &str,
    ) -> bool {
        let expressions = &mut self.expressions;
        // an empty or taken name would have its users take another expression
        if !expressions.contains_key(old) || new.is_empty() || expressions.contains_key(new) {
            return false;
        }
        *expressions = std::mem::take(expressions)
            .into_iter()
            .map(|(name, expression)| match name == old {
                true => (new.to_owned(), expression),
                false => (name, expression),
            })
            .collect();
        let named = self
            .nodes
            .iter_mut()
            .map(|node| &mut node.args)
            .chain(
                self.expressions
                    .values_mut(),
            );
        for value in named {
            rename(value, old, new);
        }
        true
    }

    /// The ids of the Nodes whose args use the expression `name`, themselves or through others.
    pub fn users(
        &self,
        name: &str,
    ) -> Vec<&str> {
        self.nodes
            .iter()
            .filter(|node| uses(&node.args, name, &self.expressions, &mut Vec::new()))
            .map(|node| node.id.as_str())
            .collect()
    }

    /// The expressions that use the expression `name`, themselves or through others.
    pub fn dependents(
        &self,
        name: &str,
    ) -> Vec<&str> {
        self.expressions
            .iter()
            .filter(|(other, expression)| {
                *other != name && uses(expression, name, &self.expressions, &mut Vec::new())
            })
            .map(|(other, _)| other.as_str())
            .collect()
    }
}

impl Registry {
    /// `args` as a node of `block` reads them, each marker replaced, or why it does not take them.
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

/// `args` of a node of `entry`, each marker replaced by its number or else its expression.
pub(crate) fn resolved(
    entry: &Entry,
    mut args: Value,
    pipeline: &Pipeline,
) -> PolarsResult<Value> {
    if !has_marker(&args) {
        return Ok(args);
    }
    let (schema, defs) = entry.schema();
    numbered(&mut args, "", &schema, &defs, pipeline)?;
    resolve(&mut args, pipeline, &mut Vec::new())?;
    Ok(args)
}

/// Replaces each marker in `value` by its expression; one named again in `chain` names itself.
fn resolve(
    value: &mut Value,
    pipeline: &Pipeline,
    chain: &mut Vec<String>,
) -> PolarsResult<()> {
    let Some(name) = marked(value).map(str::to_owned) else {
        for part in parts(value) {
            resolve(part, pipeline, chain)?;
        }
        return Ok(());
    };
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
    let mut expression = pipeline
        .expressions
        .get(&name)
        .cloned()
        .ok_or_else(|| polars_err!(ComputeError: "no expression is called '{name}'"))?;
    chain.push(name.clone());
    resolve(&mut expression, pipeline, chain)?;
    chain.pop();
    if let Err(e) = serde_json::from_value::<Expr>(expression.clone()) {
        polars_bail!(ComputeError: "`{name}` is no expression: {e}");
    }
    *value = expression;
    Ok(())
}

/// Puts its number in the place of each marker that stands where `schema` takes a number.
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

/// Whether `value` holds a marker anywhere.
fn has_marker(value: &Value) -> bool {
    match value {
        _ if marked(value).is_some() => true,
        Value::Array(items) => items
            .iter()
            .any(has_marker),
        Value::Object(fields) => fields
            .values()
            .any(has_marker),
        _ => false,
    }
}

/// Whether `value` names `name`, itself or through expressions, each of them `seen` once.
fn uses<'a>(
    value: &'a Value,
    name: &str,
    expressions: &'a Map<String, Value>,
    seen: &mut Vec<&'a str>,
) -> bool {
    match (marked(value), value) {
        (Some(marked), _) if marked == name => true,
        (Some(marked), _) if seen.contains(&marked) => false,
        (Some(marked), _) => {
            seen.push(marked);
            expressions
                .get(marked)
                .is_some_and(|expression| uses(expression, name, expressions, seen))
        }
        (None, Value::Array(items)) => items
            .iter()
            .any(|item| uses(item, name, expressions, seen)),
        (None, Value::Object(fields)) => fields
            .values()
            .any(|field| uses(field, name, expressions, seen)),
        (None, _) => false,
    }
}

/// Makes each marker of `old` in `value` one of `new`.
fn rename(
    value: &mut Value,
    old: &str,
    new: &str,
) {
    if marked(value) == Some(old) {
        *value = marker(new);
        return;
    }
    for part in parts(value) {
        rename(part, old, new);
    }
}
