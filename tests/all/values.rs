//! Named values: plain JSON that stands whole in args, and as a literal in an expression.

use polaroid::methods::text;
use polaroid::{Kind, Pipeline, Reference, Registry, marker, reference};
use serde_json::{Value, json};

use super::orders::{alias, binary, col, numbers, pipeline, refusal, resolved, run, texts};

/// The literal that the value `name` is, or that part of it is.
fn value_lit(
    name: &str,
    rest: Value,
) -> Value {
    let mut named = json!({ "$value": name });
    for (key, field) in rest
        .as_object()
        .unwrap()
    {
        named[key] = field.clone();
    }
    json!({ "Literal": named })
}

/// `expr` looked up in the table `name`: each of its keys replaced by its value, others by null.
fn looked_up(
    expr: Value,
    name: &str,
) -> Value {
    json!({ "Function": {
        "input": [
            expr,
            value_lit(name, json!({ "part": "keys" })),
            value_lit(name, json!({ "part": "values" })),
            { "Literal": { "Scalar": { "Null": "Null" } } },
        ],
        "function": { "ReplaceStrict": { "return_dtype": { "Literal": "Float64" } } },
    } })
}

/// Tables, lists and plain values of a shop, and expressions that read them as literals.
fn named() -> Value {
    let one = json!({ "Literal": { "Dyn": { "Int": 1 } } });
    let kept = binary(one, "Minus", looked_up(col("category"), "discount"));
    let cheap = value_lit("bands", json!({ "at": "/cheap/hi" }));
    let home = json!({ "Agg": { "Implode": {
        "input": value_lit("home", json!({})),
        "maintain_order": true,
    } } });
    json!({
        "values": {
            "discount": { "office": 0.0, "furniture": 0.1, "food": 0.05 },
            "bands": { "cheap": { "lo": 0, "hi": 10 }, "dear": { "lo": 10, "hi": 1000 } },
            "ladder": [{ "lo": 0 }, { "lo": 10 }],
            "home": ["DE"],
            "top": 2,
            "label": "cost",
            "by_price": [col("price")],
            "shown": [marker("discounted"), { "$value": "by_price", "at": "/0" }],
            "most": { "$value": "top" },
        },
        "expressions": {
            "discounted": alias(binary(col("price"), "Multiply", kept), "discounted"),
            "is_cheap": binary(col("price"), "Lt", cheap),
            "is_home": { "Function": {
                "input": [col("country"), home],
                "function": { "Boolean": { "IsIn": { "nulls_equal": false } } },
            } },
        },
    })
}

/// The orders that hold under the expression named `name`.
fn kept(name: &str) -> Pipeline {
    pipeline(named(), "filter", json!({ "predicates": [marker(name)] }))
}

/// The first rows of the orders, as many as `n` says.
fn first(n: Value) -> Pipeline {
    pipeline(named(), "limit", json!({ "n": n }))
}

/// Why the orders cannot be kept under `predicate`, with `values` added to those named.
fn refused(
    values: Value,
    predicate: Value,
) -> String {
    let mut pipeline = pipeline(named(), "filter", json!({ "predicates": [predicate] }));
    for (name, value) in values
        .as_object()
        .unwrap()
    {
        pipeline
            .values
            .insert(name.clone(), value.clone());
    }
    refusal(&pipeline)
}

#[test]
fn a_value_stands_whole_where_args_name_it() {
    let few = first(json!({ "$value": "top" }));
    assert_eq!(resolved(&few).unwrap(), json!({ "n": 2 }));
    assert_eq!(
        run(&few)
            .unwrap()
            .height(),
        2
    );

    // a list of expressions is a value as any other
    let dearest = pipeline(
        named(),
        "top_k",
        json!({ "k": 1, "by": { "$value": "by_price" } }),
    );
    assert_eq!(resolved(&dearest).unwrap()["by"], json!([col("price")]));
    assert_eq!(texts(&dearest, "item"), ["desk"]);
}

#[test]
fn a_value_names_values_and_expressions_which_are_resolved_in_it() {
    let through = first(json!({ "$value": "most" }));
    assert_eq!(resolved(&through).unwrap(), json!({ "n": 2 }));

    let shown = pipeline(named(), "select", json!({ "exprs": { "$value": "shown" } }));
    let read = resolved(&shown)
        .unwrap()
        .to_string();
    assert!(!read.contains('$'), "{read}");
    assert_eq!(numbers(&shown, "discounted"), [2.0, 180.0, 36.0, 4.75]);
    assert_eq!(numbers(&shown, "price"), [2.0, 200.0, 40.0, 5.0]);
}

#[test]
fn a_value_is_typed_by_the_block_that_reads_it() {
    let wrong = resolved(&first(json!({ "$value": "label" }))).unwrap_err();
    assert!(
        wrong
            .to_string()
            .contains("args: invalid type: string \"cost\"")
    );
    // and a text stands at a plain place of an expression as it stands in args
    let named_by = alias(col("price"), "");
    let mut renamed = pipeline(named(), "select", json!({ "exprs": [named_by] }));
    renamed.nodes[1].args["exprs"][0]["Alias"][1] = json!({ "$value": "label" });
    assert_eq!(numbers(&renamed, "cost"), [2.0, 200.0, 40.0, 5.0]);
}

#[test]
fn a_part_of_a_value_is_a_literal_in_an_expression() {
    let pipeline = kept("is_cheap");
    assert_eq!(texts(&pipeline, "item"), ["pen", "tea"]);
    let read = resolved(&pipeline).unwrap();
    let bound = &read["predicates"][0]["BinaryExpr"]["right"];
    assert_eq!(*bound, json!({ "Literal": { "Dyn": { "Int": 10 } } }));

    // a key that holds a slash is escaped in the pointer to it
    let mut by_weight = kept("is_cheap");
    by_weight.values["bands"] = json!({ "cheap": { "hi": 0 }, "per/kg": 100 });
    let slashed = value_lit("bands", json!({ "at": "/per~1kg" }));
    by_weight.expressions["is_cheap"]["BinaryExpr"]["right"] = slashed;
    assert_eq!(texts(&by_weight, "item"), ["pen", "lamp", "tea"]);

    // a row of a list by its place, and then a field of it
    let ladder = value_lit("ladder", json!({ "at": "/1/lo" }));
    let mut above = kept("above");
    above
        .expressions
        .insert("above".to_owned(), binary(col("price"), "GtEq", ladder));
    assert_eq!(texts(&above, "item"), ["desk", "lamp"]);
}

#[test]
fn a_plain_value_is_the_literal_of_its_kind() {
    let literal = |plain: Value| {
        let mut pipeline = pipeline(
            named(),
            "select",
            json!({ "exprs": [value_lit("plain", json!({}))] }),
        );
        pipeline
            .values
            .insert("plain".to_owned(), plain);
        resolved(&pipeline).unwrap()["exprs"][0]["Literal"].clone()
    };
    assert_eq!(literal(json!(2)), json!({ "Dyn": { "Int": 2 } }));
    assert_eq!(literal(json!(0.5)), json!({ "Dyn": { "Float": 0.5 } }));
    assert_eq!(literal(json!("a")), json!({ "Scalar": { "String": "a" } }));
    assert_eq!(
        literal(json!(true)),
        json!({ "Scalar": { "Boolean": true } })
    );
    assert_eq!(
        literal(json!(null)),
        json!({ "Scalar": { "Null": "Null" } })
    );
    // a list is a series, which Polars writes as bytes: it is never stored
    assert!(literal(json!([1, 2]))["Series"].is_array());
}

#[test]
fn a_list_is_a_literal_series_that_a_column_is_looked_for_in() {
    assert_eq!(texts(&kept("is_home"), "item"), ["pen", "desk"]);
}

#[test]
fn the_keys_and_values_of_a_table_look_a_column_up() {
    let mut pipeline = pipeline(
        named(),
        "with_columns",
        json!({ "exprs": [marker("discounted")] }),
    );
    assert_eq!(numbers(&pipeline, "discounted"), [2.0, 180.0, 36.0, 4.75]);

    // whole numbers among the values are numbers like the others, and a key not there is null
    pipeline.values["discount"] = json!({ "office": 0, "furniture": 0.5 });
    let frame = run(&pipeline).unwrap();
    let discounted: Vec<Option<f64>> = frame
        .column("discounted")
        .unwrap()
        .f64()
        .unwrap()
        .iter()
        .collect();
    assert_eq!(
        discounted,
        [
            Some(2.0),
            Some(100.0),
            Some(20.0),
            None
        ]
    );
}

#[test]
fn a_value_that_is_not_there_or_has_no_such_part_is_an_error_that_names_it() {
    let no_value = refused(json!({}), value_lit("none", json!({})));
    assert!(no_value.contains("no value is called 'none'"), "{no_value}");

    // a reference with what its kind does not take is an error, and no plain JSON
    let wrong = refused(json!({}), value_lit("bands", json!({ "at": 1 })));
    assert!(
        wrong.contains("holds `$value`, and is no reference"),
        "{wrong}"
    );

    let no_field = refused(json!({}), value_lit("bands", json!({ "at": "/cheap/top" })));
    assert!(
        no_field.contains("there is no `@bands.cheap.top`"),
        "{no_field}"
    );
    let no_row = refused(json!({}), value_lit("ladder", json!({ "at": "/2" })));
    assert!(no_row.contains("there is no `@ladder[2]`"), "{no_row}");

    let no_keys = refused(json!({}), value_lit("home", json!({ "part": "keys" })));
    assert!(
        no_keys.contains("there is no `@home.keys`: a table alone has those"),
        "{no_keys}"
    );
    let no_part = refused(json!({}), value_lit("discount", json!({ "part": "names" })));
    assert!(
        no_part.contains("`@discount` has its `keys` and its `values`, but no `names`"),
        "{no_part}"
    );
}

#[test]
fn a_field_asked_of_a_list_is_an_error_that_asks_for_a_row() {
    let wrong = refused(json!({}), value_lit("ladder", json!({ "at": "/lo" })));
    assert!(
        wrong.contains("`@ladder` is a list: name a row, as `@ladder[0]`"),
        "{wrong}"
    );
}

#[test]
fn a_table_or_a_mixed_list_is_no_literal() {
    let table = refused(json!({}), value_lit("discount", json!({})));
    assert!(
        table.contains("`@discount` is a table, which is no literal: name its keys or its values"),
        "{table}"
    );
    let part = refused(json!({}), value_lit("bands", json!({ "at": "/cheap" })));
    assert!(part.contains("`@bands.cheap` is a table"), "{part}");

    for mixed in [
        json!([1, "a"]),
        json!([[1], [2]]),
    ] {
        let mixed = refused(json!({ "mixed": mixed }), value_lit("mixed", json!({})));
        assert!(
            mixed.contains("`@mixed` is no list of one kind, which a literal takes"),
            "{mixed}"
        );
    }
}

#[test]
fn values_and_expressions_that_name_each_other_in_a_ring_are_an_error() {
    let ring = json!({ "first": { "$value": "second" }, "second": [{ "$value": "first" }] });
    let wrong = refused(ring, json!({ "$value": "first" }));
    assert!(
        wrong.contains("`first` uses `second`, which uses `first`"),
        "{wrong}"
    );

    // through an expression that reads the value as a literal
    let through = json!({ "bands": { "cheap": { "hi": marker("is_cheap") } } });
    let wrong = refused(through, marker("is_cheap"));
    assert!(
        wrong.contains("`is_cheap` uses `bands`, which uses `is_cheap`"),
        "{wrong}"
    );
}

#[test]
fn an_expression_and_a_value_share_their_names() {
    let mut pipeline = kept("is_cheap");
    // neither is renamed to the name of the other kind
    assert!(!pipeline.rename_named("top", "is_cheap"));
    assert!(!pipeline.rename_named("is_cheap", "top"));

    // and a file that calls both the same builds nothing
    pipeline
        .values
        .insert("is_cheap".to_owned(), json!(1));
    let wrong = pipeline
        .frames(&Registry::with_polars())
        .err()
        .unwrap();
    assert!(
        wrong
            .to_string()
            .contains("an expression and a value are both called 'is_cheap'")
    );
}

#[test]
fn a_value_is_renamed_where_it_is_named_and_means_what_it_meant() {
    let mut pipeline = first(json!({ "$value": "top" }));
    assert!(pipeline.rename_named("top", "few"));
    assert!(pipeline.rename_named("bands", "ranges"));
    assert!(pipeline.rename_named("by_price", "order"));

    assert_eq!(pipeline.nodes[1].args["n"], json!({ "$value": "few" }));
    assert_eq!(pipeline.values["most"], json!({ "$value": "few" }));
    assert_eq!(
        pipeline.values["shown"][1],
        json!({ "$value": "order", "at": "/0" })
    );
    assert_eq!(
        pipeline.expressions["is_cheap"]["BinaryExpr"]["right"],
        json!({ "Literal": { "$value": "ranges", "at": "/cheap/hi" } })
    );
    let names: Vec<_> = pipeline
        .values
        .keys()
        .collect();
    assert_eq!(names[..2], ["discount", "ranges"]);
    assert_eq!(resolved(&pipeline).unwrap(), json!({ "n": 2 }));

    // an expression renamed is followed into the values that name it
    assert!(pipeline.rename_named("discounted", "reduced"));
    assert_eq!(pipeline.values["shown"][0], marker("reduced"));
}

#[test]
fn what_uses_a_name_is_found_through_values_and_expressions() {
    let pipeline = pipeline(named(), "select", json!({ "exprs": { "$value": "shown" } }));
    for name in [
        "shown",
        "discounted",
        "discount",
        "by_price",
    ] {
        assert_eq!(pipeline.users(name), ["out"], "{name}");
    }
    assert!(
        pipeline
            .users("bands")
            .is_empty()
    );
    assert_eq!(pipeline.dependents("discount"), ["discounted", "shown"]);
    assert_eq!(pipeline.dependents("top"), ["most"]);
    assert_eq!(pipeline.dependents("bands"), ["is_cheap"]);
}

#[test]
fn a_name_is_an_expression_or_a_value_or_neither() {
    let pipeline = pipeline(named(), "select", json!({ "exprs": { "$value": "shown" } }));
    assert_eq!(pipeline.kind_of("discounted"), Some(Kind::Expression));
    assert_eq!(pipeline.kind_of("shown"), Some(Kind::Value));
    assert_eq!(pipeline.kind_of("gone"), None);
}

#[test]
fn a_reference_is_its_name_with_what_its_kind_takes_alone() {
    let value = json!({ "$value": "bands", "at": "/cheap/hi" });
    let read = reference(&value).unwrap();
    assert_eq!(
        (read.kind, read.name, read.at, read.part),
        (Kind::Value, "bands", "/cheap/hi", None)
    );
    // and it is written as it was read
    assert_eq!(Value::from(&read), value);
    let keys = Reference {
        part: Some("keys"),
        ..Reference::to(Kind::Value, "bands")
    };
    assert_eq!(
        Value::from(&keys),
        json!({ "$value": "bands", "part": "keys" })
    );
    assert_eq!(
        reference(&marker("gross"))
            .unwrap()
            .kind,
        Kind::Expression
    );

    // with anything else, or a name that is no text, it is a value like any other
    for plain in [
        json!({ "$value": "bands", "more": 1 }),
        json!({ "$value": "bands", "at": 1 }),
        json!({ "$expr": "gross", "at": "/x" }),
        json!({ "$value": 1 }),
        json!("bands"),
    ] {
        assert_eq!(reference(&plain), None, "{plain}");
    }
}

#[test]
fn a_value_is_written_by_its_name_and_the_part_that_is_meant() {
    let written = |rest: Value| text(&value_lit("bands", rest)["Literal"]);
    assert_eq!(written(json!({})), "@bands");
    assert_eq!(written(json!({ "at": "/cheap/hi" })), "@bands.cheap.hi");
    assert_eq!(written(json!({ "at": "/0/lo" })), "@bands[0].lo");
    assert_eq!(written(json!({ "part": "keys" })), "@bands.keys");
    assert_eq!(
        written(json!({ "at": "/cheap", "part": "values" })),
        "@bands.cheap.values"
    );

    // a key that holds a slash is written as it is, where its pointer escapes it
    assert_eq!(written(json!({ "at": "/per~1kg" })), "@bands.per/kg");

    // in an expression it is a literal, and bare as an argument
    let named = named();
    assert_eq!(text(&value_lit("top", json!({}))), "pl.lit(@top)");
    assert_eq!(
        text(&named["expressions"]["is_cheap"]),
        r#"pl.col("price").lt(@bands.cheap.hi)"#
    );
}

#[test]
fn a_pipeline_file_with_values_is_read_run_and_written_as_it_was() {
    let pipeline = kept("is_cheap");
    let written = serde_json::to_string_pretty(&pipeline).unwrap();
    let path = std::env::temp_dir().join(format!("polaroid-values-{}.json", std::process::id()));
    std::fs::write(&path, &written).unwrap();

    let read = Pipeline::load(
        path.to_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(texts(&read, "item"), ["pen", "tea"]);
    assert_eq!(serde_json::to_string_pretty(&read).unwrap(), written);
    std::fs::remove_file(path).unwrap();

    // a file without values is read as before, and written without them
    let mut plain = read;
    plain.values = Default::default();
    plain.expressions = Default::default();
    plain.nodes[1].args = json!({ "predicates": [] });
    let written = serde_json::to_string(&plain).unwrap();
    assert!(!written.contains("values"), "{written}");
    assert!(serde_json::from_str::<Pipeline>(&written).is_ok());
}
