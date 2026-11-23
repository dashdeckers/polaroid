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

#[test]
fn every_block_and_every_one_of_its_args_is_described() {
    let catalog = Registry::with_polars().catalog();
    for (name, block) in &catalog.blocks {
        let args = polaroid::schema::resolve(&catalog.defs, &block.args);
        assert!(args["description"].is_string(), "{name}");
        let fields = args["properties"]
            .as_object()
            .into_iter()
            .flatten();
        for (field, schema) in fields {
            assert!(schema["description"].is_string(), "{name}.{field}");
        }
    }
}

#[test]
fn sort_puts_nulls_first_either_way() {
    let by = serde_json::to_value(col("a")).unwrap();
    for descending in [false, true] {
        let frame = df!("a" => [Some(1), None, Some(3)]).unwrap();
        let args = json!({ "by": [by], "descending": descending });
        let sorted = apply("sort", args, frame).unwrap();
        let first = sorted
            .column("a")
            .unwrap()
            .get(0)
            .unwrap();
        assert_eq!(first, AnyValue::Null);
    }
}

#[test]
fn drop_nulls_of_no_columns_drops_no_row() {
    let frame = df!("a" => [Some(1), None]).unwrap();
    let kept = apply("drop_nulls", json!({ "columns": [] }), frame).unwrap();
    assert_eq!(kept.height(), 2);
}

#[test]
fn a_column_that_is_not_there_is_an_error_to_drop_and_to_rename() {
    let frame = df!("a" => [1]).unwrap();
    assert!(apply("drop", json!({ "columns": ["b"] }), frame.clone()).is_err());
    let args = json!({ "existing": ["b"], "new": ["c"] });
    assert!(apply("rename", args, frame).is_err());
}

#[test]
fn explode_gives_a_null_row_for_a_null_list() {
    let lists = df!("id" => [1, 2])
        .unwrap()
        .lazy()
        .with_column(
            when(col("id").eq(lit(1)))
                .then(concat_list([lit(1), lit(2)]).unwrap())
                .otherwise(lit(NULL))
                .alias("list"),
        )
        .collect()
        .unwrap();
    let long = apply("explode", json!({ "columns": ["list"] }), lists).unwrap();
    assert_eq!(long.height(), 3);
    assert!(apply("explode", json!({ "columns": [] }), long).is_err());
}
