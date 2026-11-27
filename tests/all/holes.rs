//! Expressions with holes: a call gives each hole what stands in its place.

use polaroid::methods::text;
use polaroid::{Kind, Pipeline, Taken, marked, marker, reference};
use serde_json::{Value, json};

use super::orders::{alias, binary, col, lit, numbers, pipeline, refusal, resolved, texts};

/// The hole `name`, or the part of what it is given that the pointer `at` leads to.
fn hole(
    name: &str,
    at: &str,
) -> Value {
    match at {
        "" => json!({ "$arg": name }),
        at => json!({ "$arg": name, "at": at }),
    }
}

/// A call of the expression `name` that gives its holes what is `given`.
fn call(
    name: &str,
    given: Value,
) -> Value {
    json!({ "$expr": name, "with": given })
}

/// Whether `x` lies in a band: from its `lo`, and below its `hi`.
fn in_band(
    x: Value,
    lo: Value,
    hi: Value,
) -> Value {
    json!({ "Function": {
        "input": [x, { "Literal": lo }, { "Literal": hi }],
        "function": { "Boolean": { "IsBetween": { "closed": "Left" } } },
    } })
}

/// Functions of a shop, what they are called with, and the bands of its prices.
fn named() -> Value {
    let with_tax = binary(lit(json!(1)), "Plus", hole("rate", ""));
    let total = binary(
        json!({ "Column": hole("column", "") }),
        "Multiply",
        col("quantity"),
    );
    json!({
        "values": {
            "bands": { "cheap": { "lo": 0, "hi": 10 }, "dear": { "lo": 10, "hi": 1000 } },
            "ladder": [{ "lo": 0, "hi": 10 }],
            "rates": { "DE": 0.19, "FR": 0.2 },
        },
        "expressions": {
            "rate": lit(json!(0.19)),
            "gross_of": binary(hole("x", ""), "Multiply", with_tax),
            "taxed": call("gross_of", json!({ "x": hole("x", ""), "rate": marker("rate") })),
            "in_band": in_band(hole("x", ""), hole("band", "/lo"), hole("band", "/hi")),
            "total_of": { "Alias": [total, hole("name", "")] },
            "twice": binary(hole("n", ""), "Multiply", lit(json!(2))),
        },
    })
}

/// The orders with a column `out` of `expr` added.
fn with(expr: Value) -> Pipeline {
    pipeline(
        named(),
        "with_columns",
        json!({ "exprs": [alias(expr, "out")] }),
    )
}

/// Why the orders cannot be given a column of `expr`.
fn refused(expr: Value) -> String {
    refusal(&with(expr))
}

#[test]
fn a_call_gives_each_hole_of_an_expression_what_stands_in_its_place() {
    let gross = |column: &str| {
        call(
            "gross_of",
            json!({ "x": col(column), "rate": lit(json!(0.19)) }),
        )
    };
    assert_eq!(
        numbers(&with(gross("price")), "out"),
        [2.38, 238.0, 47.6, 5.95]
    );
    assert_eq!(
        numbers(&with(gross("shipping")), "out"),
        [1.19, 35.7, 5.95, 2.38]
    );

    // what the Block reads is the expression with its holes filled
    let read = resolved(&with(gross("price"))).unwrap();
    let filled = binary(
        col("price"),
        "Multiply",
        binary(lit(json!(1)), "Plus", lit(json!(0.19))),
    );
    assert_eq!(read["exprs"][0], alias(filled, "out"));
}

#[test]
fn an_argument_is_resolved_where_its_call_stands() {
    // a named expression, and a part of a value as a literal
    let by_name = call(
        "gross_of",
        json!({ "x": col("price"), "rate": marker("rate") }),
    );
    assert_eq!(numbers(&with(by_name), "out"), [2.38, 238.0, 47.6, 5.95]);
    let french = json!({ "Literal": { "$value": "rates", "at": "/FR" } });
    let by_value = call("gross_of", json!({ "x": col("price"), "rate": french }));
    assert_eq!(numbers(&with(by_value), "out"), [2.4, 240.0, 48.0, 6.0]);

    // and a call, whose own holes are filled first
    let twice = call("twice", json!({ "n": col("price") }));
    let of_call = call("gross_of", json!({ "x": twice, "rate": marker("rate") }));
    assert_eq!(numbers(&with(of_call), "out"), [4.76, 476.0, 95.2, 11.9]);
}

#[test]
fn a_function_passes_its_holes_on_to_the_functions_it_calls() {
    let taxed = call("taxed", json!({ "x": col("price") }));
    assert_eq!(numbers(&with(taxed), "out"), [2.38, 238.0, 47.6, 5.95]);
}

#[test]
fn a_part_of_an_argument_is_a_literal_under_a_literal() {
    let kept = |band: Value| {
        let predicate = call("in_band", json!({ "x": col("price"), "band": band }));
        pipeline(named(), "filter", json!({ "predicates": [predicate] }))
    };
    // a part of a value, a row of a list, and a table written where it is given
    let cheap = kept(json!({ "$value": "bands", "at": "/cheap" }));
    assert_eq!(texts(&cheap, "item"), ["pen", "tea"]);
    let row = kept(json!({ "$value": "ladder", "at": "/0" }));
    assert_eq!(texts(&row, "item"), ["pen", "tea"]);
    let written = kept(json!({ "lo": 10, "hi": 100 }));
    assert_eq!(texts(&written, "item"), ["lamp"]);

    let read = resolved(&cheap).unwrap();
    assert_eq!(
        read["predicates"][0]["Function"]["input"],
        json!([
            col("price"),
            lit(json!(0)),
            lit(json!(10))
        ])
    );
}

#[test]
fn a_hole_stands_at_a_plain_place_as_a_column_name_or_an_alias() {
    let total = call(
        "total_of",
        json!({ "column": "shipping", "name": "shipping_total" }),
    );
    let pipeline = pipeline(named(), "with_columns", json!({ "exprs": [total] }));
    assert_eq!(
        numbers(&pipeline, "shipping_total"),
        [10.0, 30.0, 10.0, 8.0]
    );
}

#[test]
fn a_number_arg_names_a_call_that_comes_to_a_number() {
    let twice = call("twice", json!({ "n": lit(json!(2)) }));
    let pipeline = pipeline(named(), "limit", json!({ "n": twice }));
    assert_eq!(resolved(&pipeline).unwrap(), json!({ "n": 4 }));
}

#[test]
fn the_holes_of_an_expression_are_found_with_what_each_takes() {
    let mut pipeline = with(marker("rate"));
    let rounded = json!({ "Function": {
        "input": [hole("x", "")],
        "function": { "Round": { "decimals": hole("places", ""), "mode": "HalfToEven" } },
    } });
    let listed = json!({ "Agg": { "Implode": {
        "input": col("item"),
        "maintain_order": hole("kept", ""),
    } } });
    pipeline
        .expressions
        .insert("rounded".to_owned(), rounded);
    pipeline
        .expressions
        .insert("listed".to_owned(), listed);

    let holes = |name: &str| {
        pipeline
            .holes(name)
            .unwrap()
    };
    use Taken::{Any, Expression, Flag, Number, Text};
    assert_eq!(
        holes("gross_of"),
        [
            ("x", Expression),
            ("rate", Expression)
        ]
    );
    assert_eq!(
        holes("total_of"),
        [
            ("column", Text),
            ("name", Text)
        ]
    );
    assert_eq!(
        holes("rounded"),
        [
            ("x", Expression),
            ("places", Number)
        ]
    );
    assert_eq!(holes("listed"), [("kept", Flag)]);
    // one of which a part is meant, and one passed on, may be given anything
    assert_eq!(
        holes("in_band"),
        [
            ("x", Expression),
            ("band", Any)
        ]
    );
    assert_eq!(holes("taxed"), [("x", Any)]);
    assert_eq!(holes("rate"), []);
}

#[test]
fn a_hole_used_at_two_kinds_of_place_is_an_error() {
    let mut pipeline = with(marker("rate"));
    let both = alias(hole("x", ""), "");
    pipeline
        .expressions
        .insert("both".to_owned(), both);
    pipeline.expressions["both"]["Alias"][1] = hole("x", "");
    let wrong = pipeline
        .holes("both")
        .unwrap_err()
        .to_string();
    assert!(
        wrong.contains("`$x` stands where an expression is taken, and where text"),
        "{wrong}"
    );
    // and so is every call of it
    pipeline.nodes[1].args["exprs"][0] = call("both", json!({ "x": "price" }));
    assert!(refusal(&pipeline).contains("`$x` stands where"));
}

#[test]
fn an_expression_is_told_to_have_a_hole_whether_it_is_sound_or_not() {
    let mut pipeline = with(marker("rate"));
    assert!(pipeline.is_holed("gross_of"));
    // one that only passes its holes on has them too
    assert!(pipeline.is_holed("taxed"));
    assert!(!pipeline.is_holed("rate"));
    assert!(!pipeline.is_holed("gone"));
    // and one that is no sound function, which names what is not there
    let unsound = binary(hole("x", ""), "Plus", marker("gone"));
    pipeline
        .expressions
        .insert("unsound".to_owned(), unsound);
    assert!(
        pipeline
            .holes("unsound")
            .is_err()
    );
    assert!(pipeline.is_holed("unsound"));
}

#[test]
fn a_call_that_gives_too_little_or_too_much_is_an_error() {
    let missing = refused(call("gross_of", json!({ "x": col("price") })));
    assert!(
        missing.contains("`gross_of` takes `rate`, which is not given"),
        "{missing}"
    );
    let unknown = call(
        "twice",
        json!({ "n": col("price"), "times": lit(json!(3)) }),
    );
    let unknown = refused(unknown);
    assert!(unknown.contains("`twice` takes no `times`"), "{unknown}");

    // a function named without a call gives nothing, as an argument too: it is no value
    let bare = refused(marker("twice"));
    assert!(
        bare.contains("`twice` takes `n`, which is not given"),
        "{bare}"
    );
    let passed = refused(call("twice", json!({ "n": marker("twice") })));
    assert!(
        passed.contains("`twice` takes `n`, which is not given"),
        "{passed}"
    );
}

#[test]
fn an_argument_of_another_kind_than_its_hole_takes_is_an_error() {
    let text = refused(call(
        "gross_of",
        json!({ "x": "price", "rate": marker("rate") }),
    ));
    assert!(
        text.contains("`gross_of`: `x` is the text \"price\", where an expression is taken"),
        "{text}"
    );
    let number = refused(call("total_of", json!({ "column": "price", "name": 1 })));
    assert!(
        number.contains("`total_of`: `name` is the number 1, where text is taken"),
        "{number}"
    );
    let expression = refused(call(
        "total_of",
        json!({ "column": col("price"), "name": "total" }),
    ));
    assert!(
        expression.contains("`total_of`: `column` is a table, where text is taken"),
        "{expression}"
    );

    // and an expression is no literal, where what is plain is one
    let band = json!({ "lo": col("price"), "hi": 1 });
    let literal = refused(call("in_band", json!({ "x": col("price"), "band": band })));
    assert!(
        literal.contains("`$band.lo` is an expression, which is no literal"),
        "{literal}"
    );
}

#[test]
fn a_part_that_an_argument_does_not_have_is_an_error_that_names_the_hole() {
    let band = |band: Value| refused(call("in_band", json!({ "x": col("price"), "band": band })));
    let no_field = band(json!({ "lo": 0 }));
    assert!(no_field.contains("there is no `$band.hi`"), "{no_field}");
    let list = band(json!({ "$value": "ladder" }));
    assert!(
        list.contains("`$band` is a list: name a row, as `$band[0]`"),
        "{list}"
    );
    let table = band(json!({ "lo": {}, "hi": 1 }));
    assert!(
        table.contains("`$band.lo` is a table, which is no literal"),
        "{table}"
    );
}

#[test]
fn a_hole_outside_a_function_is_an_error() {
    let in_args = refused(hole("x", ""));
    assert!(
        in_args.contains("`$x` is a hole, and stands in no function"),
        "{in_args}"
    );
    // a value is no function, though a function names it
    let mut pipeline = with(call("twice", json!({ "n": { "$value": "open" } })));
    pipeline
        .values
        .insert("open".to_owned(), hole("n", ""));
    assert!(refusal(&pipeline).contains("`$n` is a hole, and stands in no function"));
}

#[test]
fn a_function_that_calls_itself_is_an_error_that_names_the_chain() {
    let mut pipeline = with(call("twice", json!({ "n": col("price") })));
    pipeline
        .expressions
        .insert(
            "twice".to_owned(),
            call("twice", json!({ "n": hole("n", "") })),
        );
    assert!(refusal(&pipeline).contains("`twice` uses `twice`"));

    // through another, and through an argument
    pipeline
        .expressions
        .insert(
            "twice".to_owned(),
            call("taxed", json!({ "x": hole("n", "") })),
        );
    pipeline
        .expressions
        .insert(
            "gross_of".to_owned(),
            binary(
                hole("x", ""),
                "Plus",
                call("twice", json!({ "n": hole("rate", "") })),
            ),
        );
    let wrong = refusal(&pipeline);
    assert!(
        wrong.contains("`twice` uses `taxed`, which uses `gross_of`, which uses `twice`"),
        "{wrong}"
    );
}

#[test]
fn a_function_is_checked_alone_with_its_holes_left_open() {
    let mut pipeline = with(marker("rate"));
    for name in [
        "gross_of", "taxed", "in_band", "total_of", "twice", "rate",
    ] {
        assert!(
            pipeline
                .holes(name)
                .is_ok(),
            "{name}"
        );
    }
    // also where an argument of a call it makes is built on a hole
    let doubled = binary(hole("x", ""), "Multiply", lit(json!(2)));
    let twice_taxed = call("gross_of", json!({ "x": doubled, "rate": marker("rate") }));
    pipeline
        .expressions
        .insert("twice_taxed".to_owned(), twice_taxed);
    assert_eq!(
        pipeline
            .holes("twice_taxed")
            .unwrap(),
        [("x", Taken::Expression)]
    );
    let why = |pipeline: &Pipeline, name: &str| {
        pipeline
            .holes(name)
            .unwrap_err()
            .to_string()
    };
    assert!(why(&pipeline, "none").contains("no expression is called 'none'"));

    // what it names is not there, or it gives a call too little
    pipeline
        .expressions
        .shift_remove("rate");
    assert!(why(&pipeline, "taxed").contains("no expression is called 'rate'"));
    pipeline
        .expressions
        .insert(
            "taxed".to_owned(),
            call("gross_of", json!({ "x": hole("x", "") })),
        );
    assert!(why(&pipeline, "taxed").contains("`gross_of` takes `rate`, which is not given"));
    pipeline
        .expressions
        .insert(
            "taxed".to_owned(),
            call("taxed", json!({ "x": hole("x", "") })),
        );
    assert!(why(&pipeline, "taxed").contains("`taxed` uses `taxed`"));
}

#[test]
fn what_a_call_and_its_arguments_name_is_used_and_renamed() {
    let french = json!({ "Literal": { "$value": "rates", "at": "/FR" } });
    let gross = call("gross_of", json!({ "x": col("price"), "rate": french }));
    let mut pipeline = with(gross);
    assert_eq!(pipeline.users("gross_of"), ["out"]);
    assert_eq!(pipeline.users("rates"), ["out"]);
    assert_eq!(pipeline.dependents("rate"), ["taxed"]);
    assert_eq!(pipeline.dependents("gross_of"), ["taxed"]);
    // a hole is no name of the pipeline, though an expression is called the same
    assert!(
        pipeline
            .dependents("x")
            .is_empty()
    );

    assert!(pipeline.rename_named("gross_of", "with_tax"));
    assert!(pipeline.rename_named("rates", "taxes"));
    assert!(pipeline.rename_named("rate", "usual"));
    let renamed = &pipeline.nodes[1].args["exprs"][0]["Alias"][0];
    assert_eq!(marked(renamed), Some("with_tax"));
    assert_eq!(
        renamed["with"]["rate"]["Literal"],
        json!({ "$value": "taxes", "at": "/FR" })
    );
    assert_eq!(
        pipeline.expressions["taxed"],
        call(
            "with_tax",
            json!({ "x": hole("x", ""), "rate": marker("usual") })
        )
    );
    assert_eq!(numbers(&pipeline, "out"), [2.4, 240.0, 48.0, 6.0]);
}

#[test]
fn a_reference_takes_arguments_or_a_part_as_its_kind_does() {
    let called = call("twice", json!({ "n": 1 }));
    let read = reference(&called).unwrap();
    assert_eq!((read.kind, read.name), (Kind::Expression, "twice"));
    assert_eq!(read.with.unwrap()["n"], 1);
    assert_eq!(Value::from(&read), called);
    let part = hole("band", "/lo");
    let read = reference(&part).unwrap();
    assert_eq!(Value::from(&read), part);
    assert_eq!(
        (read.kind, read.name, read.at),
        (Kind::Argument, "band", "/lo")
    );

    // arguments are a table, and a hole has no arguments, keys or values
    for plain in [
        json!({ "$expr": "twice", "with": [1] }),
        json!({ "$arg": "n", "with": {} }),
        json!({ "$arg": "n", "part": "keys" }),
    ] {
        assert_eq!(reference(&plain), None, "{plain}");
    }
}

#[test]
fn calls_and_holes_are_written_as_they_are_read() {
    let named = named();
    let body = |name: &str| text(&named["expressions"][name]);
    assert_eq!(body("gross_of"), "$x.mul(pl.lit(1).add($rate))");
    assert_eq!(body("taxed"), "@gross_of(x=$x, rate=@rate)");
    assert_eq!(body("in_band"), "$x.is_between($band.lo, $band.hi, …)");
    assert_eq!(
        body("total_of"),
        r#"pl.col($column).mul(pl.col("quantity")).alias($name)"#
    );

    // an argument is an expression in short, a literal bare, and plain JSON as it is
    let band = json!({ "$value": "bands", "at": "/cheap" });
    let in_band = call("in_band", json!({ "x": col("price"), "band": band }));
    assert_eq!(
        text(&in_band),
        r#"@in_band(x=pl.col("price"), band=@bands.cheap)"#
    );
    let total = call("total_of", json!({ "column": "shipping", "name": "total" }));
    assert_eq!(
        text(&total),
        r#"@total_of(column="shipping", name="total")"#
    );
    let twice = call("twice", json!({ "n": lit(json!(2)) }));
    assert_eq!(text(&alias(twice, "n")), r#"@twice(n=2).alias("n")"#);
    let written = call(
        "in_band",
        json!({ "x": col("price"), "band": { "lo": 0, "hi": 1 } }),
    );
    assert_eq!(
        text(&written),
        r#"@in_band(x=pl.col("price"), band={"lo":0,"hi":1})"#
    );
}

#[test]
fn a_pipeline_file_with_calls_is_read_run_and_written_as_it_was() {
    let band = json!({ "$value": "bands", "at": "/cheap" });
    let cheap = call("in_band", json!({ "x": col("price"), "band": band }));
    let pipeline = pipeline(named(), "filter", json!({ "predicates": [cheap] }));
    let written = serde_json::to_string_pretty(&pipeline).unwrap();
    let path = std::env::temp_dir().join(format!("polaroid-holes-{}.json", std::process::id()));
    std::fs::write(&path, &written).unwrap();

    let read = Pipeline::load(
        path.to_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(texts(&read, "item"), ["pen", "tea"]);
    assert_eq!(serde_json::to_string_pretty(&read).unwrap(), written);
    std::fs::remove_file(path).unwrap();
}
