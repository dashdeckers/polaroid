//! Expressions written out as text, the way Python Polars writes its calls.

use serde_json::Value;

use super::{Method, read};

/// `node` as a chain of calls: `pl.col("a").str.contains("x", strict=False)`.
///
/// An argument at its default is left out, and those after it go by name. A
/// literal that is an argument is its bare value. A hole is `_`, and a node
/// that none of `methods` reads is its variant in angle brackets.
pub fn text(
    methods: &[Method],
    node: &Value,
) -> String {
    let Some(method) = read(methods, node) else {
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
        Some(at) => text(methods, pointer(node, at)),
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

    format!("{path}({})", arguments(methods, method, node))
}

/// The arguments of `node`, a call of `method`, as [`text`] writes them
/// between its parentheses.
pub fn arguments(
    methods: &[Method],
    method: &Method,
    node: &Value,
) -> String {
    // a literal is its value
    if let Some(literal) = literal(node) {
        return literal;
    }

    let named = |name: &str, written: String, is_by_name: bool| match is_by_name {
        true => format!("{name}={written}"),
        false => written,
    };
    let mut args = Vec::new();

    // once an argument is left out, those after it go by name
    let mut is_by_name = false;
    for input in &method.inputs {
        let value = pointer(node, &input.at);
        if input.is_variadic {
            let items = value
                .as_array()
                .into_iter()
                .flatten();
            args.extend(items.map(|item| argument(methods, item)));
        } else if !value.is_null() && value == pointer(&method.node, &input.at) {
            is_by_name = true;
        } else {
            args.push(named(&input.name, argument(methods, value), is_by_name));
        }
    }

    // the first option of a call without inputs goes without its name
    let mut is_by_name = !method
        .inputs
        .is_empty();
    for option in &method.options {
        let value = pointer(node, &option.at);
        if value != pointer(&method.node, &option.at) {
            args.push(named(&option.name, plain(value), is_by_name));
        }
        is_by_name = true;
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
fn argument(
    methods: &[Method],
    node: &Value,
) -> String {
    literal(node).unwrap_or_else(|| text(methods, node))
}

/// The value of a literal `node` as Python writes it, if it is a plain one: a
/// number, a string, a boolean or null.
fn literal(node: &Value) -> Option<String> {
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

/// A value that is no expression, as Python writes it. A data type that is
/// only a name is `pl.` and that name.
fn plain(value: &Value) -> String {
    match (value, value["Literal"].as_str()) {
        (Value::Object(tagged), Some(dtype)) if tagged.len() == 1 => format!("pl.{dtype}"),
        (Value::Null, _) => "None".to_owned(),
        (Value::Bool(true), _) => "True".to_owned(),
        (Value::Bool(false), _) => "False".to_owned(),
        _ => value.to_string(),
    }
}
