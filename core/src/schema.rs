//! Reading a JSON Schema: its `$ref`s, types, variants and defaults.

use serde_json::{Number, Value};

/// The number that `typed` writes exactly; `1.` or `-` is none yet.
pub fn number(typed: &str) -> Option<Number> {
    typed
        .parse::<Number>()
        .ok()
        .filter(|number| number.to_string() == typed)
}

/// The number `value` moved by `steps`, within what its schema takes; none yet is moved from 0.
///
/// A step is 1 for an integer, and else a tenth of the power of ten that the number is at.
pub fn nudged(
    schema: &Value,
    value: &Value,
    steps: i32,
) -> Value {
    let within = |number: f64| {
        let least = schema["minimum"]
            .as_f64()
            .unwrap_or(f64::MIN);
        let most = schema["maximum"]
            .as_f64()
            .unwrap_or(f64::MAX);
        number.clamp(least, most)
    };
    let from = value
        .as_f64()
        .unwrap_or_default();
    if json_type(schema) == "integer" {
        return Value::from(within(from + f64::from(steps)) as i64);
    }
    // below 1 a step is a tenth too, so that a drag comes through 0
    let step = 10f64.powf(
        from.abs()
            .max(1.0)
            .log10()
            .floor()
            - 1.0,
    );
    // a whole number of steps, which a sum of them is not quite
    let moved = (from / step).round() + f64::from(steps);
    Value::from(within((moved * step * 10.0).round() / 10.0))
}

/// A value to start from: the default, else null if nullable, else [`blank`].
pub fn default(
    defs: &Value,
    schema: &Value,
) -> Value {
    let resolved = resolve(defs, schema);
    // a default beside a `$ref` is the field's own
    match schema
        .get("default")
        .or(resolved.get("default"))
    {
        Some(default) => default.clone(),
        None if optional(resolved).is_some() => Value::Null,
        None => blank(defs, resolved),
    }
}

/// The fields or parts of the schema's type, each from its default; null where one is to be given.
pub fn blank(
    defs: &Value,
    schema: &Value,
) -> Value {
    let schema = resolve(defs, schema);
    match json_type(schema) {
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
        _ => Value::Null,
    }
}

/// What a nullable `schema` holds when it is not null: `Option`'s `Some`.
pub fn optional(schema: &Value) -> Option<&Value> {
    let is_null = |option: &Value| option["type"] == "null";
    // alternatives of which none is null are no nullable
    let options = schema["anyOf"]
        .as_array()
        .filter(|options| {
            options
                .iter()
                .any(is_null)
        });
    match (options, &schema["type"]) {
        (Some(options), _) => options
            .iter()
            .find(|option| !is_null(option)),
        (_, Value::Array(_)) => Some(schema),
        _ => None,
    }
}

/// The JSON type of `schema`, null aside.
pub fn json_type(schema: &Value) -> &str {
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

/// The variants of an enum: each name, and the schema of its payload if it has one.
pub fn variants(schema: &Value) -> Vec<(&str, Option<&Value>)> {
    let cases = match (schema["oneOf"].as_array(), schema.get("enum")) {
        (Some(cases), _) => cases.as_slice(),
        (None, Some(_)) => std::slice::from_ref(schema),
        (None, None) => &[],
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

/// The schema of the value at the JSON pointer `at`, or the null schema if it leads nowhere.
pub fn pointer<'a>(
    defs: &'a Value,
    schema: &'a Value,
    at: &str,
) -> &'a Value {
    at.split('/')
        .skip(1)
        .fold(resolve(defs, schema), |schema, key| {
            // through a nullable to what it holds
            let schema = optional(schema).map_or(schema, |some| resolve(defs, some));
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
