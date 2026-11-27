//! What the editor keeps in a `ui`, of a pipeline or of a node: the objects it writes into.

use serde_json::{Map, Value, json};

/// The object at `keys` under `ui`; whatever is no object on the way there is made an empty one.
pub fn object<'a>(
    ui: &'a mut Value,
    keys: &[&str],
) -> &'a mut Map<String, Value> {
    let mut value = ui;
    for key in keys {
        value = solid(value)
            .entry(*key)
            .or_insert(Value::Null);
    }
    solid(value)
}

/// The object that `value` is, or is made if it is none.
fn solid(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value
        .as_object_mut()
        .expect("it is an object")
}
