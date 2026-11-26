//! Named expressions: one naming others, a number arg naming one, and what follows a name.

use polaroid::methods::text;
use polaroid::{Pipeline, Registry, marked, marker};
use serde_json::{Value, json};

use super::orders::{alias, binary, col, lit, numbers, pipeline, refusal, resolved, run, texts};

/// A rate, two expressions built on it and on each other, and numbers for the args of a node.
fn named() -> Value {
    let with_tax = || binary(lit(json!(1)), "Plus", marker("rate"));
    let gross = binary(col("price"), "Multiply", with_tax());
    let gross_total = binary(marker("net_total"), "Multiply", with_tax());
    json!({ "expressions": {
        "rate": lit(json!(0.19)),
        "gross": alias(gross, "gross"),
        "net_total": binary(col("price"), "Multiply", col("quantity")),
        "gross_total": alias(gross_total, "gross_total"),
        "top_n": lit(json!(2)),
        "twice_n": binary(marker("top_n"), "Multiply", lit(json!(2))),
    } })
}

/// The orders with the columns of these names added.
fn priced(names: &[&str]) -> Pipeline {
    let exprs: Vec<Value> = names
        .iter()
        .map(|name| marker(name))
        .collect();
    pipeline(named(), "with_columns", json!({ "exprs": exprs }))
}

/// The first rows of the orders, as many as `n` says.
fn first(n: Value) -> Pipeline {
    pipeline(named(), "limit", json!({ "n": n }))
}

#[test]
fn an_expression_names_another_and_runs_as_what_that_is() {
    let pipeline = priced(&["gross"]);
    assert_eq!(numbers(&pipeline, "gross"), [2.38, 238.0, 47.6, 5.95]);

    // what the Block reads holds the rate where its name stood
    let read = resolved(&pipeline).unwrap();
    let rate = &read["exprs"][0]["Alias"][0]["BinaryExpr"]["right"]["BinaryExpr"]["right"];
    assert_eq!(*rate, json!({ "Literal": { "Dyn": { "Float": 0.19 } } }));
}

#[test]
fn an_expression_built_on_expressions_is_resolved_through_them() {
    let pipeline = priced(&["gross_total"]);
    assert_eq!(numbers(&pipeline, "gross_total"), [23.8, 238.0, 95.2, 23.8]);
    // a name used twice is resolved twice, and no marker is left
    let both = priced(&["gross", "gross_total"]);
    let read = resolved(&both)
        .unwrap()
        .to_string();
    assert!(!read.contains("$expr"), "{read}");
    assert_eq!(numbers(&both, "gross"), [2.38, 238.0, 47.6, 5.95]);
}

#[test]
fn an_expression_is_written_with_the_names_it_is_built_on() {
    let named = named();
    assert_eq!(text(&marker("rate")), "@rate");
    assert_eq!(
        text(&named["expressions"]["gross_total"]),
        r#"@net_total.mul(pl.lit(1).add(@rate)).alias("gross_total")"#
    );
}

#[test]
fn a_name_that_is_no_expression_is_an_error_wherever_it_is_named() {
    let in_args = priced(&["net"]);
    assert!(refusal(&in_args).contains("no expression is called 'net'"));

    let mut in_another = priced(&["gross"]);
    in_another
        .expressions
        .shift_remove("rate");
    assert!(refusal(&in_another).contains("no expression is called 'rate'"));
}

#[test]
fn an_expression_that_names_itself_is_an_error_that_names_the_chain() {
    let mut pipeline = priced(&["gross_total"]);
    pipeline
        .expressions
        .insert("rate".to_owned(), marker("rate"));
    assert!(refusal(&pipeline).contains("`rate` uses `rate`"));

    // through others, from the first that is named twice
    pipeline
        .expressions
        .insert("rate".to_owned(), marker("net_total"));
    pipeline
        .expressions
        .insert("net_total".to_owned(), marker("gross_total"));
    let wrong = refusal(&pipeline);
    assert!(
        wrong.contains("`gross_total` uses `net_total`, which uses `gross_total`"),
        "{wrong}"
    );
}

#[test]
fn a_node_whose_names_fail_leaves_the_other_nodes_to_build() {
    let mut pipeline = priced(&["net"]);
    pipeline.outputs = vec!["orders".to_owned()];
    let frames = pipeline
        .frames(&polaroid::Registry::with_polars())
        .unwrap();
    assert!(frames["out"].is_err());
    assert!(frames["orders"].is_ok());
}

#[test]
fn a_number_arg_names_an_expression_that_is_a_number() {
    let pipeline = first(marker("top_n"));
    assert_eq!(resolved(&pipeline).unwrap(), json!({ "n": 2 }));
    assert_eq!(
        run(&pipeline)
            .unwrap()
            .height(),
        2
    );
}

#[test]
fn a_number_arg_names_an_expression_of_literals_and_named_numbers() {
    let pipeline = first(marker("twice_n"));
    assert_eq!(resolved(&pipeline).unwrap(), json!({ "n": 4 }));

    // a whole number that comes of a float is whole where a whole one is taken
    let mut halved = first(marker("half_n"));
    let half = binary(marker("twice_n"), "Multiply", lit(json!(0.5)));
    halved
        .expressions
        .insert("half_n".to_owned(), half);
    assert_eq!(resolved(&halved).unwrap(), json!({ "n": 2 }));
}

#[test]
fn a_number_arg_refuses_a_name_that_is_no_number_it_takes() {
    let of = |name: &str| refusal(&first(marker(name)));
    // one that reads a column, one that is no whole number, and one that is not there
    assert!(of("gross").contains("`gross` is no number, which `n` takes"));
    assert!(of("rate").contains("`rate` is no whole number, which `n` takes"));
    assert!(of("none").contains("no expression is called 'none'"));

    // the count of the rows is no number, and a float too great for a whole number is not whole
    let with = |name: &str, expression: Value| {
        let mut pipeline = first(marker(name));
        pipeline
            .expressions
            .insert(name.to_owned(), expression);
        refusal(&pipeline)
    };
    assert!(with("count", json!("Len")).contains("`count` is no number, which `n` takes"));
    assert!(with("vast", lit(json!(1e30))).contains("`vast` is no whole number"));

    // a number stored as it is, is no expression
    let mut plain = first(marker("three"));
    plain
        .expressions
        .insert("three".to_owned(), json!(3));
    assert!(refusal(&plain).contains("`three` is no expression"));
}

#[test]
fn a_name_in_a_list_of_expressions_stays_an_expression_though_it_is_a_number() {
    let pipeline = pipeline(
        named(),
        "top_k",
        json!({ "k": marker("top_n"), "by": [marker("net_total")] }),
    );
    let read = resolved(&pipeline).unwrap();
    assert_eq!(read["k"], json!(2));
    assert!(read["by"][0]["BinaryExpr"].is_object());
    assert_eq!(texts(&pipeline, "item"), ["desk", "lamp"]);
}

#[test]
fn args_that_the_block_does_not_read_are_refused_once_resolved() {
    let wrong = resolved(&first(json!("two"))).unwrap_err();
    assert!(
        wrong
            .to_string()
            .contains("args: invalid type")
    );
    assert!(resolved(&first(json!(2))).is_ok());
}

#[test]
fn an_expression_is_renamed_in_the_expressions_that_name_it() {
    let mut pipeline = priced(&["gross", "rate"]);
    assert!(pipeline.rename_named("rate", "tax"));
    let named = &pipeline.expressions["gross_total"]["Alias"][0]["BinaryExpr"];
    assert_eq!(named["right"]["BinaryExpr"]["right"], marker("tax"));
    assert_eq!(marked(&pipeline.nodes[1].args["exprs"][1]), Some("tax"));
    assert_eq!(numbers(&pipeline, "gross"), [2.38, 238.0, 47.6, 5.95]);

    // not to the name of another, though no node names that one
    assert!(!pipeline.rename_named("tax", "top_n"));
}

#[test]
fn a_node_uses_what_the_expressions_it_names_are_built_on() {
    let pipeline = priced(&["gross_total"]);
    assert_eq!(pipeline.users("gross_total"), ["out"]);
    assert_eq!(pipeline.users("rate"), ["out"]);
    assert_eq!(pipeline.users("net_total"), ["out"]);
    assert!(
        pipeline
            .users("gross")
            .is_empty()
    );
}

#[test]
fn the_expressions_built_on_one_are_those_it_cannot_name() {
    let mut pipeline = priced(&[]);
    assert_eq!(pipeline.dependents("rate"), ["gross", "gross_total"]);
    assert_eq!(pipeline.dependents("top_n"), ["twice_n"]);
    assert!(
        pipeline
            .dependents("gross")
            .is_empty()
    );

    // in a cycle each is built on the others, and looking ends
    pipeline
        .expressions
        .insert("rate".to_owned(), marker("gross"));
    assert_eq!(pipeline.dependents("rate"), ["gross", "gross_total"]);
    assert_eq!(pipeline.dependents("gross"), ["rate", "gross_total"]);
}

#[test]
fn a_removed_expression_leaves_those_that_named_it_without_one() {
    let mut pipeline = priced(&["gross_total"]);
    pipeline
        .expressions
        .shift_remove("net_total");
    assert_eq!(pipeline.users("net_total"), ["out"]);
    assert!(refusal(&pipeline).contains("no expression is called 'net_total'"));
}

#[test]
fn a_pipeline_file_is_read_run_and_written_as_it_was() {
    let pipeline = priced(&["gross", "gross_total"]);
    let written = serde_json::to_string_pretty(&pipeline).unwrap();
    let path = std::env::temp_dir().join(format!("polaroid-names-{}.json", std::process::id()));
    std::fs::write(&path, &written).unwrap();

    let read = Pipeline::load(
        path.to_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(numbers(&read, "gross_total"), [23.8, 238.0, 95.2, 23.8]);
    assert_eq!(serde_json::to_string_pretty(&read).unwrap(), written);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_refusal_says_where_in_the_args_it_is_about() {
    let at = |pipeline: &Pipeline| {
        let out = &pipeline.nodes[1];
        Registry::with_polars()
            .resolved(&out.block, &out.args, pipeline)
            .expect_err("the args are refused")
            .at
    };
    // a name that is no number where one is taken, and a name that is not there
    let mut count = first(marker("count"));
    count
        .expressions
        .insert("count".to_owned(), json!("Len"));
    assert_eq!(at(&count).as_deref(), Some("/n"));
    let exprs = json!({ "exprs": [marker("gross"), marker("nothing")] });
    let missing = pipeline(named(), "with_columns", exprs);
    assert_eq!(at(&missing).as_deref(), Some("/exprs/1"));
    // and what its block does not read is about the args as a whole
    assert_eq!(at(&first(json!("two"))), None);
}
