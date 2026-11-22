use super::*;

#[test]
fn a_literal_is_typed_as_python_writes_it() {
    let cases = [
        ("5", json!({ "Dyn": { "Int": 5 } })),
        ("-5", json!({ "Dyn": { "Int": -5 } })),
        ("1.5", json!({ "Dyn": { "Float": 1.5 } })),
        ("True", json!({ "Scalar": { "Boolean": true } })),
        ("None", json!({ "Scalar": { "Null": "Null" } })),
        ("abc", json!({ "Scalar": { "String": "abc" } })),
        ("", json!({ "Scalar": { "String": "" } })),
        // on the way to a number, which typing must not be fought over
        ("1.", json!({ "Scalar": { "String": "1." } })),
        ("-", json!({ "Scalar": { "String": "-" } })),
        // a string that reads as something else is in quotes
        ("\"5\"", json!({ "Scalar": { "String": "5" } })),
        ("\"True\"", json!({ "Scalar": { "String": "True" } })),
    ];
    let typed = |value: &Value| typed(&json!({ "Literal": value }));
    for (text, value) in cases {
        assert_eq!(literal(text), value, "{text}");
        assert_eq!(typed(&value).as_deref(), Some(text), "{text}");
    }

    // quotes that are not needed go
    assert_eq!(typed(&literal("\"abc\"")).as_deref(), Some("abc"));
    // what is no plain value is not typed
    assert_eq!(typed(&json!({ "Series": {} })), None);
}
