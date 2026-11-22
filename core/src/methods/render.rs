//! Expressions written out as text: a chain of calls, each in short.

use serde_json::Value;

use super::{Arg, Method, read};

/// `node` as a chain of calls: `pl.col("a").str.contains("x", …)`.
pub fn text(node: &Value) -> String {
    let Some(method) = read(node) else {
        let variant = match node {
            Value::Null => return "_".to_owned(),
            Value::String(variant) => variant.as_str(),
            _ => node
                .as_object()
                .and_then(|tagged| tagged.keys().next())
                .map_or("?", String::as_str),
        };
        return format!("<{variant}>");
    };

    let in_namespace = !method
        .namespace
        .is_empty();
    let root = match &method.receiver {
        Some(at) => text(pointer(node, at)),
        None if in_namespace => String::new(),
        None => "pl".to_owned(),
    };
    let path = [
        root.as_str(),
        method.namespace,
        method.name,
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(".");

    format!("{path}({})", arguments(method, node))
}

/// The arguments of `node` in short: its inputs or else its first option, and `…` for the rest.
pub fn arguments(
    method: &Method,
    node: &Value,
) -> String {
    // a literal is its value
    if let Some(literal) = literal(node) {
        return literal;
    }

    // the inputs, as far as the last that is not at its default
    let is_set = |input: &Arg| {
        let value = pointer(node, &input.at);
        input.is_variadic || value.is_null() || value != pointer(&method.node, &input.at)
    };
    let shown = method
        .inputs
        .iter()
        .rposition(is_set)
        .map_or(0, |last| last + 1);
    let mut args = Vec::new();
    for input in &method.inputs[..shown] {
        let value = pointer(node, &input.at);
        match value
            .as_array()
            .filter(|_| input.is_variadic)
        {
            Some(items) => args.extend(
                items
                    .iter()
                    .map(argument),
            ),
            None => args.push(argument(value)),
        }
    }

    // without inputs the first option stands in, unless it is a struct
    let first = method
        .options
        .first()
        .filter(|_| {
            method
                .inputs
                .is_empty()
        })
        .map(|option| plain(pointer(node, &option.at)))
        .filter(|written| !written.starts_with(['{', '[']));
    let options = usize::from(first.is_some());
    args.extend(first);

    // an input left out is at its default, but an option may be anything
    if options < method.options.len() {
        args.push("…".to_owned());
    }
    args.join(", ")
}

/// The value that the JSON pointer `at` leads to in `node`, or null.
fn pointer<'a>(
    node: &'a Value,
    at: &str,
) -> &'a Value {
    node.pointer(at)
        .unwrap_or(&Value::Null)
}

/// An expression that is an argument: a literal as its bare value.
fn argument(node: &Value) -> String {
    literal(node).unwrap_or_else(|| text(node))
}

/// A plain literal as Python writes it: a number, string, boolean or null.
pub fn literal(node: &Value) -> Option<String> {
    let value = &node["Literal"];
    let is_null = value["Scalar"]["Null"].is_string();
    [
        &value["Dyn"]["Int"],
        &value["Dyn"]["Float"],
        &value["Dyn"]["Str"],
        &value["Scalar"]["String"],
        &value["Scalar"]["Boolean"],
    ]
    .into_iter()
    .find(|found| !found.is_null())
    .map(plain)
    .or_else(|| is_null.then(|| "None".to_owned()))
}

/// A value that is no expression, as Python writes it; a named data type is `pl.Name`.
fn plain(value: &Value) -> String {
    match (value, value["Literal"].as_str()) {
        (Value::Object(tagged), Some(dtype)) if tagged.len() == 1 => format!("pl.{dtype}"),
        (Value::Null, _) => "None".to_owned(),
        (Value::Bool(true), _) => "True".to_owned(),
        (Value::Bool(false), _) => "False".to_owned(),
        _ => value.to_string(),
    }
}
