//! The values that a JSON Schema starts from.

use polaroid::schema::{default, nudged, read};
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

#[test]
fn a_number_is_read_as_a_person_types_it_on_the_way() {
    // as it is written where nothing asks for a whole number
    let written = |typed: &str| read(typed, false).map(|number| json!(number));
    assert_eq!(written("1"), Some(json!(1)));
    assert_eq!(written("1.0"), Some(json!(1.0)));
    assert_eq!(written("1e3"), Some(json!(1000.0)));
    // and whole where a whole number is asked for, if it is one
    let read = |typed: &str| read(typed, true).map(|number| json!(number));
    for (typed, number) in [
        ("1.", json!(1)),
        (" 900.00 ", json!(900)),
        ("1e3", json!(1000)),
        ("-2", json!(-2)),
        ("2.5", json!(2.5)),
        (".5", json!(0.5)),
        // a comma is the point where there is none
        ("1,5", json!(1.5)),
    ] {
        assert_eq!(read(typed), Some(number), "{typed}");
    }
    // and none yet on the way, or none at all
    for typed in [
        "", "-", ".", "1.e", "x", "1,5,3", "1.5,3", "inf", "NaN",
    ] {
        assert_eq!(read(typed), None, "{typed}");
    }
}
