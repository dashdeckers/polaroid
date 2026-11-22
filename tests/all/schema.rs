//! The values that a JSON Schema starts from.

use polaroid::schema::default;
use serde_json::{Value, json};

#[test]
fn a_value_starts_from_its_default_or_from_nothing() {
    let defs = json!({});
    let starts = |schema: Value| default(&defs, &schema);

    assert_eq!(starts(json!({ "type": "integer", "default": 5 })), 5);
    // what has to be given is none, not a value that nobody chose
    assert_eq!(
        starts(json!({ "type": "integer", "minimum": 1 })),
        Value::Null
    );
    assert_eq!(starts(json!({ "type": "number" })), Value::Null);
    assert_eq!(starts(json!({ "type": ["integer", "null"] })), Value::Null);

    // nor is a text or a yes or no made up
    assert_eq!(starts(json!({ "type": "string" })), Value::Null);
    assert_eq!(starts(json!({ "type": "boolean" })), Value::Null);
    assert_eq!(
        starts(json!({ "type": "boolean", "default": false })),
        false
    );

    // a list starts empty, and a group with each of its fields
    assert_eq!(
        starts(json!({ "type": "array", "items": { "type": "string" } })),
        json!([])
    );
    let fields = json!({ "type": "object", "properties": { "n": { "type": "integer" } } });
    assert_eq!(starts(fields), json!({ "n": null }));
}
