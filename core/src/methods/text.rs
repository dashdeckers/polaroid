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
        return match node {
            Value::Null => "_".to_owned(),
            Value::String(variant) => format!("<{variant}>"),
            _ => {
                let variant = node
                    .as_object()
                    .and_then(|tagged| tagged.keys().next())
                    .map_or("?", String::as_str);
                format!("<{variant}>")
            }
        };
    };

    let mut text = match &method.receiver {
        Some(at) => format!("{}.", self::text(methods, self::at(node, at))),
        None if method
            .namespace
            .is_empty() =>
        {
            "pl.".to_owned()
        }
        None => String::new(),
    };
    if !method
        .namespace
        .is_empty()
    {
        text += method.namespace;
        text += ".";
    }
    text += method.name;

    format!("{text}({})", arguments(methods, method, node))
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

    let mut args = Vec::new();
    // once an argument is left out, those after it go by name
    let mut is_by_name = false;
    for input in &method.inputs {
        let value = at(node, &input.at);
        if input.is_variadic {
            let items = value
                .as_array()
                .into_iter()
                .flatten();
            args.extend(items.map(|item| argument(methods, item)));
            continue;
        }
        if !value.is_null() && value == at(&method.node, &input.at) {
            is_by_name = true;
            continue;
        }
        let written = argument(methods, value);
        args.push(match is_by_name {
            true => format!("{}={written}", input.name),
            false => written,
        });
    }
    // the first option of a call without inputs goes without its name
    let mut is_by_name = !method
        .inputs
        .is_empty();
    for option in &method.options {
        let value = at(node, &option.at);
        if value == at(&method.node, &option.at) {
            is_by_name = true;
            continue;
        }
        let written = plain(value);
        args.push(match is_by_name {
            true => format!("{}={written}", option.name),
            false => written,
        });
        is_by_name = true;
    }

    args.join(", ")
}

/// The value at a JSON pointer into `node`, or null if it leads nowhere.
fn at<'a>(
    node: &'a Value,
    pointer: &str,
) -> &'a Value {
    node.pointer(pointer)
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
    let plain = [
        &value["Dyn"]["Int"],
        &value["Dyn"]["Float"],
        &value["Dyn"]["Str"],
        &value["Scalar"]["String"],
        &value["Scalar"]["Boolean"],
    ]
    .into_iter()
    .find(|plain| !plain.is_null());
    match plain {
        Some(plain) => Some(self::plain(plain)),
        None if value["Scalar"]["Null"].is_string() => Some("None".to_owned()),
        None => None,
    }
}

/// A value that is no expression, as Python writes it. A data type that is
/// only a name is `pl.` and that name.
fn plain(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Object(tagged) if tagged.len() == 1 && value["Literal"].is_string() => {
            format!(
                "pl.{}",
                value["Literal"]
                    .as_str()
                    .unwrap_or_default()
            )
        }
        _ => value.to_string(),
    }
}
