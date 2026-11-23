//! Expressions written out as text.

use polaroid::methods::{literal_of, methods, text, typed};
use polaroid::polars::prelude::*;
use serde_json::{Value, json};

use super::methods::{json_of, method};

#[test]
fn an_expression_is_written_as_a_chain_of_calls() {
    let methods = methods();
    let written = |expr: Expr| text(&json_of(expr));

    let chain = col("name")
        .str()
        .strip_chars(lit(" "))
        .str()
        .to_lowercase()
        .str()
        .contains(lit("x"), false)
        .sum()
        .gt(lit(2))
        .alias("many");
    assert_eq!(
        written(chain),
        r#"pl.col("name").str.strip_chars(" ").str.to_lowercase().str.contains("x", …).sum().gt(2).alias("many")"#
    );

    // the inputs go as far as the last that is not at its default
    let slice = col("a")
        .str()
        .slice(lit(1), lit(NULL));
    assert_eq!(written(slice), r#"pl.col("a").str.slice(1)"#);
    let nulls = || lit(NULL);
    let replace = col("d")
        .dt()
        .replace(
            nulls(),
            nulls(),
            lit(1),
            nulls(),
            nulls(),
            nulls(),
            nulls(),
            lit("raise"),
        );
    assert_eq!(written(replace), r#"pl.col("d").dt.replace(None, None, 1)"#);

    // functions, with a variadic input and with a default
    assert_eq!(
        written(coalesce(&[col("a"), col("b"), lit(0)])),
        r#"pl.coalesce(pl.col("a"), pl.col("b"), 0)"#
    );
    let choice = when(col("a").is_null())
        .then(lit("none"))
        .otherwise(lit(NULL));
    assert_eq!(written(choice), r#"pl.when(pl.col("a").is_null(), "none")"#);

    // the first option of a call without inputs, and `…` for the rest
    assert_eq!(
        written(lit(5).strict_cast(DataType::Int8)),
        "pl.lit(5).cast(pl.Int8, …)"
    );

    // a hole, and a node that no method reads
    let contains = method(methods, "str", "contains").call(json_of(col("a")));
    assert_eq!(text(&contains), r#"pl.col("a").str.contains(_, …)"#);
    let unread = json!({ "Display": { "inputs": [], "fmt_str": "" } });
    assert_eq!(text(&unread), "<Display>");
}

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
        assert_eq!(literal_of(text), value, "{text}");
        assert_eq!(typed(&value).as_deref(), Some(text), "{text}");
    }

    // quotes that are not needed go
    assert_eq!(typed(&literal_of("\"abc\"")).as_deref(), Some("abc"));
    // what is no plain value is not typed
    assert_eq!(typed(&json!({ "Series": {} })), None);
}
