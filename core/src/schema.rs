//! Reading a JSON Schema, as the catalog holds them: its `$ref`s, types,
//! variants and defaults, and the schema of a value inside another.

use serde_json::Value;

/// A value to start from: the schema's default, else null if it may be null,
/// else [`blank`].
pub fn default(
    defs: &Value,
    schema: &Value,
) -> Value {
    let schema = resolve(defs, schema);
    match schema.get("default") {
        Some(default) => default.clone(),
        None if optional(schema).is_some() => Value::Null,
        None => blank(defs, schema),
    }
}

/// An empty value of `schema`'s type, or null where a variant has to be
/// picked.
pub fn blank(
    defs: &Value,
    schema: &Value,
) -> Value {
    match kind(schema) {
        "object" => schema["properties"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, field)| (name.clone(), default(defs, field)))
            .collect(),
        "array" => schema["prefixItems"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| default(defs, item))
            .collect(),
        "boolean" => false.into(),
        "integer" | "number" => 0.into(),
        "string" if variants(schema).is_empty() => "".into(),
        _ => Value::Null,
    }
}

/// What a nullable `schema` holds when it is not null: `Option`'s `Some`.
pub fn optional(schema: &Value) -> Option<&Value> {
    match (&schema["anyOf"], &schema["type"]) {
        (Value::Array(options), _) => options
            .iter()
            .find(|option| option["type"] != "null"),
        (_, Value::Array(_)) => Some(schema),
        _ => None,
    }
}

/// The JSON type of `schema`, null aside.
pub fn kind(schema: &Value) -> &str {
    match &schema["type"] {
        Value::String(kind) => kind,
        Value::Array(kinds) => kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|kind| *kind != "null")
            .unwrap_or_default(),
        _ => "",
    }
}

/// Follows a `$ref` into `defs`.
pub fn resolve<'a>(
    defs: &'a Value,
    schema: &'a Value,
) -> &'a Value {
    match schema["$ref"].as_str() {
        Some(path) => &defs[path.trim_start_matches("#/$defs/")],
        None => schema,
    }
}

/// The variants of an enum `schema`: the name of each, and the schema of its
/// payload if it has one. Empty if `schema` is no enum.
pub fn variants(schema: &Value) -> Vec<(&str, Option<&Value>)> {
    let cases = match schema["oneOf"].as_array() {
        Some(cases) => cases,
        None if schema
            .get("enum")
            .is_some() =>
        {
            std::slice::from_ref(schema)
        }
        None => &[],
    };

    cases
        .iter()
        .flat_map(|case| {
            let named = case["const"].as_str();
            let listed = case["enum"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str);
            let tagged = case["properties"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(name, payload)| (name.as_str(), Some(payload)));
            named
                .into_iter()
                .chain(listed)
                .map(|name| (name, None))
                .chain(tagged)
        })
        .collect()
}

/// The schema of the value at the JSON pointer `at` into a value of `schema`.
/// The pointer leads through the variant of an enum to its payload, through a
/// struct to a field, and through an array to an item.
///
/// Returns the null schema if the pointer leads nowhere, or to a variant
/// without payload.
pub fn at<'a>(
    defs: &'a Value,
    schema: &'a Value,
    at: &str,
) -> &'a Value {
    at.split('/')
        .skip(1)
        .fold(resolve(defs, schema), |schema, key| {
            let variant = variants(schema)
                .into_iter()
                .find(|(name, _)| *name == key);
            let inner = match (variant, key.parse::<usize>()) {
                (Some((_, payload)), _) => payload.unwrap_or(&Value::Null),
                (None, Ok(i)) if schema["prefixItems"].is_array() => &schema["prefixItems"][i],
                (None, Ok(_)) => &schema["items"],
                (None, Err(_)) => &schema["properties"][key],
            };
            resolve(defs, inner)
        })
}
