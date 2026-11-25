//! A small table of orders and a pipeline over it, which the tests of names share.

use std::collections::HashMap;

use polaroid::polars::prelude::*;
use polaroid::{Pipeline, Registry, polars};
use serde_json::{Value, json};

/// Four orders: an item, its category, price and shipping, how many, and where to.
pub fn orders() -> DataFrame {
    df!(
        "item" => ["pen", "desk", "lamp", "tea"],
        "category" => ["office", "furniture", "furniture", "food"],
        "price" => [2.0, 200.0, 40.0, 5.0],
        "shipping" => [1.0, 30.0, 5.0, 2.0],
        "quantity" => [10, 1, 2, 4],
        "country" => ["DE", "DE", "FR", "FR"],
    )
    .unwrap()
}

/// A column, as Polars writes it in JSON.
pub fn col(name: &str) -> Value {
    json!({ "Column": name })
}

/// A literal number or text, as Polars writes it in JSON.
pub fn lit(value: Value) -> Value {
    match &value {
        Value::Number(number) if number.is_f64() => {
            json!({ "Literal": { "Dyn": { "Float": value } } })
        }
        Value::Number(_) => json!({ "Literal": { "Dyn": { "Int": value } } }),
        _ => json!({ "Literal": { "Scalar": { "String": value } } }),
    }
}

/// `left` and `right` under an operator, as `Multiply`, `Plus` or `Lt`.
pub fn binary(
    left: Value,
    op: &str,
    right: Value,
) -> Value {
    json!({ "BinaryExpr": { "left": left, "op": op, "right": right } })
}

/// An expression under another name.
pub fn alias(
    expr: Value,
    name: &str,
) -> Value {
    json!({ "Alias": [expr, name] })
}

/// A pipeline of a node `orders` and one node `out` of `block` that reads it, with what is `named`.
pub fn pipeline(
    named: Value,
    block: &str,
    args: Value,
) -> Pipeline {
    let mut pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "orders", "block": "scan_parquet", "args": { "path": "" } },
            { "id": "out", "block": block, "inputs": ["orders"], "args": args },
        ],
        "outputs": ["out"],
    });
    for (key, names) in named
        .as_object()
        .unwrap()
    {
        pipeline[key] = names.clone();
    }
    serde_json::from_value(pipeline).unwrap()
}

/// The frame of the node `out`, run over the orders.
pub fn run(pipeline: &Pipeline) -> PolarsResult<DataFrame> {
    let known = HashMap::from([("orders".to_owned(), orders())]);
    let mut frames = pipeline.run_from(&Registry::with_polars(), &known)?;
    Ok(frames.remove(0))
}

/// The column `name` of the node `out`, run over the orders, as numbers rounded to cents.
pub fn numbers(
    pipeline: &Pipeline,
    name: &str,
) -> Vec<f64> {
    run(pipeline)
        .unwrap()
        .column(name)
        .unwrap()
        .cast(&DataType::Float64)
        .unwrap()
        .f64()
        .unwrap()
        .into_no_null_iter()
        .map(|number| (number * 100.0).round() / 100.0)
        .collect()
}

/// The column `name` of the node `out`, run over the orders, as text; a null is `-`.
pub fn texts(
    pipeline: &Pipeline,
    name: &str,
) -> Vec<String> {
    run(pipeline)
        .unwrap()
        .column(name)
        .unwrap()
        .str()
        .unwrap()
        .iter()
        .map(|text| {
            text.unwrap_or("-")
                .to_owned()
        })
        .collect()
}

/// Why the node `out` cannot be built.
pub fn refusal(pipeline: &Pipeline) -> String {
    run(pipeline)
        .unwrap_err()
        .to_string()
}

/// The args of the node `out` as its Block reads them.
pub fn resolved(pipeline: &Pipeline) -> PolarsResult<Value> {
    let out = &pipeline.nodes[1];
    Registry::with_polars().resolved(&out.block, &out.args, pipeline)
}
