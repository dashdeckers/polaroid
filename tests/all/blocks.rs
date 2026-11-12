//! The blocks, each on a frame of its own.

use std::collections::HashMap;

use polaroid::polars::prelude::*;
use polaroid::{Pipeline, Registry, polars};
use serde_json::{Value, json};

/// What a node of `block` with `args` makes of `frame`.
fn apply(
    block: &str,
    args: Value,
    frame: DataFrame,
) -> PolarsResult<DataFrame> {
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "in", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "out", "block": block, "inputs": ["in"], "args": args },
        ],
        "outputs": ["out"],
    });
    let pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let known = HashMap::from([("in".to_owned(), frame)]);
    let mut frames = pipeline.run_from(&Registry::with_polars(), &known)?;
    Ok(frames.remove(0))
}

#[test]
fn rename_takes_as_many_new_names_as_columns() {
    let frame = df!("a" => [1], "b" => [2]).unwrap();
    let args = json!({ "existing": ["a", "b"], "new": ["x"] });
    assert!(apply("rename", args, frame).is_err());
}

#[test]
fn unique_of_no_columns_is_of_every_column() {
    let frame = df!("a" => [1, 1, 2]).unwrap();
    let unique = apply("unique", json!({ "subset": [] }), frame).unwrap();
    assert_eq!(unique.height(), 2);
}

#[test]
fn unpivot_of_no_columns_is_of_every_other_column() {
    let frame = df!("id" => [1], "a" => [2], "b" => [3]).unwrap();
    let args = json!({ "on": [], "index": ["id"] });
    let long = apply("unpivot", args, frame).unwrap();
    assert_eq!(long.height(), 2);
}
