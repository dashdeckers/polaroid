//! The values that a JSON Schema starts from.

use polaroid::schema::{default, nudged};
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

#[test]
fn a_number_is_moved_by_steps_of_its_size_within_what_its_schema_takes() {
    let integer = json!({ "type": "integer", "minimum": 0 });
    assert_eq!(nudged(&integer, &json!(5), 3), 8);
    assert_eq!(nudged(&integer, &json!(5), -9), 0);
    // none yet is moved from nothing
    assert_eq!(nudged(&integer, &Value::Null, 2), 2);

    let number = json!({ "type": ["number", "null"] });
    assert_eq!(nudged(&number, &json!(640), 1), 650.0);
    assert_eq!(nudged(&number, &json!(1000), -1), 900.0);
    assert_eq!(nudged(&number, &json!(2.5), 3), 2.8);
    // through nothing, in steps that stay whole tenths
    assert_eq!(nudged(&number, &json!(0.1), -3), -0.2);
    assert_eq!(nudged(&number, &json!(0.3), -3), 0.0);
}
