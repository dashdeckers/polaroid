//! Expressions written out as text.

use polaroid::methods::{methods, text};
use polaroid::polars::prelude::*;
use serde_json::json;

use super::methods::{json_of, method};

#[test]
fn an_expression_is_written_as_a_chain_of_calls() {
    let methods = methods();
    let written = |expr: Expr| text(&methods, &json_of(expr));

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
    let contains = method(&methods, "str", "contains").call(json_of(col("a")));
    assert_eq!(
        text(&methods, &contains),
        r#"pl.col("a").str.contains(_, …)"#
    );
    let unread = json!({ "Display": { "inputs": [], "fmt_str": "" } });
    assert_eq!(text(&methods, &unread), "<Display>");
}
