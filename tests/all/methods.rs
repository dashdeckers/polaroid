//! The Methods checked against Polars: its JSON Schema, and the nodes it builds.

use std::collections::BTreeSet;

use polaroid::methods::{Arg, Method, methods, read};
use polaroid::polars::prelude::*;
use polaroid::schema::{self, resolve, variants};
use schemars::generate::SchemaSettings;
use serde_json::{Value, json};

/// The kinds of node that no Method reads.
const OTHERS: &[&str] = &[
    // a Rust closure
    "/AnonymousFunction",
    // about data types, not values
    "/DataTypeFunction",
    // not real expressions
    "/Display",
    "/SubPlan",
    // Rust's `/`, which rounds integers towards zero
    "/BinaryExpr/op/RustDivide",
    // takes a Rust closure
    "/RenameAlias/function/Map",
    // take selectors, which are no expressions
    "/Selector/Union",
    "/Selector/Difference",
    "/Selector/ExclusiveOr",
    "/Selector/Intersect",
    // take a Rust closure
    "/Function/function/FoldHorizontal",
    "/Function/function/ReduceHorizontal",
    "/Function/function/CumReduceHorizontal",
    "/Function/function/CumFoldHorizontal",
    // internal to Polars
    "/Function/function/RowEncode",
    "/Function/function/RowDecode",
    // built by the function `format_str`, whose payload follows from its format
    "/Function/function/StringExpr/Format",
    // takes a selector, which is no expression
    "/Function/function/StructExpr/SelectFields",
    // takes a Rust closure
    "/Function/function/StructExpr/MapFieldNames",
];

/// The `$defs` of the JSON Schema of an expression.
fn defs() -> Value {
    let mut generator = SchemaSettings::draft2020_12().into_generator();
    generator.subschema_for::<Expr>();
    json!(generator.definitions())
}

/// The variants of the enum at `at` in an expression, each with its payload.
fn nodes<'a>(
    defs: &'a Value,
    at: &str,
) -> Vec<(&'a str, Option<&'a Value>)> {
    variants(schema::pointer(defs, &defs["Expr"], at))
}

pub fn json_of(expr: Expr) -> Value {
    serde_json::to_value(&expr).expect("an expression serializes")
}

fn arg<'a>(
    node: &'a Value,
    args: &[Arg],
    name: &str,
) -> &'a Value {
    let arg = args
        .iter()
        .find(|arg| arg.name == name)
        .expect("the argument is named");
    node.pointer(&arg.at)
        .expect("the argument is in the node")
}

pub fn method<'a>(
    methods: &'a [Method],
    namespace: &str,
    name: &str,
) -> &'a Method {
    methods
        .iter()
        .find(|method| method.namespace == namespace && method.name == name)
        .expect("the method is in the table")
}

/// The node of `method` on a column `receiver`, each input a column of its name.
fn filled(method: &Method) -> Value {
    let mut node = method.call(json!({ "Column": "receiver" }));
    for input in &method.inputs {
        let column = json!({ "Column": input.name });
        let value = node
            .pointer_mut(&input.at)
            .expect("the input is in the node");
        match input.is_variadic {
            true => *value = json!([column]),
            false if value.is_null() => *value = column,
            false => {}
        }
    }
    node
}

fn label(method: &Method) -> String {
    format!("{}.{} ({})", method.namespace, method.name, method.kind)
}

#[test]
fn every_node_is_a_method_or_listed() {
    let methods = methods();
    let defs = defs();
    let kinds = || {
        methods
            .iter()
            .map(|method| method.kind.as_str())
            .chain(
                OTHERS
                    .iter()
                    .copied(),
            )
            .filter_map(|kind| kind.rsplit_once('/'))
    };
    // the enums that tell the kinds apart, the expression itself among them
    let enums: BTreeSet<&str> = kinds()
        .map(|(at, _)| at)
        .chain([""])
        .collect();
    let mut problems = Vec::new();

    for at in &enums {
        let nodes = nodes(&defs, at);
        if nodes.is_empty() {
            problems.push(format!("{at}: the schema has no enum there"));
        }

        for (variant, _) in &nodes {
            let kind = format!("{at}/{variant}");
            let is_read = methods
                .iter()
                .any(|method| method.kind == kind);
            let is_other = OTHERS.contains(&kind.as_str());
            // its kinds are told by an enum further in
            let is_split = enums
                .iter()
                .filter_map(|inner| inner.strip_prefix(kind.as_str()))
                .any(|rest| rest.is_empty() || rest.starts_with('/'));

            match usize::from(is_read) + usize::from(is_other) + usize::from(is_split) {
                0 => problems.push(format!("{kind}: no method reads it")),
                1 => {}
                _ => problems.push(format!(
                    "{kind}: it is more than one of read, listed and split"
                )),
            }
        }

        for (_, variant) in kinds().filter(|(of, _)| of == at) {
            let is_node = nodes
                .iter()
                .any(|(node, _)| *node == variant);
            if !is_node {
                problems.push(format!("{at}/{variant}: the schema has no such node"));
            }
        }
    }

    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn every_receiver_and_input_is_an_expression_in_the_schema() {
    let defs = defs();
    let expr = &defs["Expr"];
    let is_expr = |at: &str| schema::pointer(&defs, expr, at) == expr;

    for method in methods() {
        let is_fit = method
            .receiver
            .iter()
            .all(|at| is_expr(at))
            && method
                .inputs
                .iter()
                .all(|input| match input.is_variadic {
                    true => is_expr(&format!("{}/0", input.at)),
                    false => is_expr(&input.at),
                });
        assert!(is_fit, "{}", label(method));
    }
}

#[test]
fn every_option_is_a_part_of_its_payload_in_the_schema() {
    let defs = defs();

    for method in methods() {
        let (at, variant) = method
            .kind
            .rsplit_once('/')
            .expect("a kind ends in its variant");
        let payload = nodes(&defs, at)
            .into_iter()
            .find(|(node, _)| *node == variant)
            .and_then(|(_, payload)| payload);
        let payload = resolve(&defs, payload.unwrap_or(&Value::Null));

        for option in &method.options {
            let is_in_schema = !schema::pointer(&defs, &defs["Expr"], &option.at).is_null();
            let key = option
                .at
                .strip_prefix(&method.kind)
                .expect("an option is in the payload")
                .trim_start_matches('/');
            // a field of a struct, a part of a tuple, or the payload as a whole
            let is_field = payload["properties"]
                .get(key)
                .is_some()
                && option.name == key;
            let is_part = key
                .parse::<usize>()
                .is_ok_and(|i| !payload["prefixItems"][i].is_null());
            let is_whole = key.is_empty() && !payload.is_null();
            assert!(
                is_in_schema && (is_field || is_part || is_whole),
                "{}: {option:?} is no part of {payload}",
                label(method)
            );
        }
    }
}

#[test]
fn every_method_round_trips_and_is_read_by_itself_alone() {
    let methods = methods();
    let mut problems = Vec::new();

    for method in methods {
        let node = filled(method);

        match serde_json::from_value::<Expr>(node.clone()).map(json_of) {
            Ok(back) if back == node => {}
            Ok(back) => problems.push(format!("{}: Polars gives back {back}", label(method))),
            Err(e) => problems.push(format!("{}: {e}", label(method))),
        }

        let readers: Vec<String> = methods
            .iter()
            .filter(|other| other.reads(&node))
            .map(label)
            .collect();
        if readers != [label(method)] {
            problems.push(format!("{}: read by {readers:?}", label(method)));
        }
    }

    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn every_name_is_once_a_method_and_once_a_function_at_most() {
    let mut seen = BTreeSet::new();
    for method in methods() {
        let is_function = method
            .receiver
            .is_none();
        let name = (method.namespace, method.name, is_function);
        assert!(seen.insert(name), "{name:?} is in the table twice");
    }
}

#[test]
fn a_call_is_what_polars_builds() {
    let methods = methods();

    let contains = filled(method(methods, "str", "contains"));
    let built = col("receiver")
        .str()
        .contains(col("pattern"), true);
    assert_eq!(contains, json_of(built));

    // the receiver is inside the payload, beside the options
    let cast = filled(method(methods, "", "cast"));
    assert_eq!(cast, json_of(col("receiver").strict_cast(DataType::String)));

    // an operator is told by a field
    let add = filled(method(methods, "", "add"));
    assert_eq!(add, json_of(col("receiver") + col("other")));

    // a function has no receiver, and a variadic input is a list
    let coalesce_ = method(methods, "", "coalesce");
    assert_eq!(coalesce_.receiver, None);
    assert_eq!(filled(coalesce_), json_of(coalesce(&[col("exprs")])));

    // two methods of one node, told by its payload
    let clip_min = method(methods, "", "clip_min");
    assert!(clip_min.reads(&json_of(col("a").clip_min(lit(0)))));
    assert!(!clip_min.reads(&json_of(col("a").clip_max(lit(0)))));
    assert!(
        clip_min
            .options
            .is_empty()
    );
}

#[test]
fn a_chain_reads_from_its_last_call_back_to_its_root() {
    let chain = json_of(
        col("name")
            .str()
            .strip_chars(lit(" "))
            .str()
            .to_lowercase()
            .str()
            .contains(lit("x"), false)
            .sum()
            .gt(lit(2))
            .alias("many"),
    );

    let mut names = Vec::new();
    let mut node = &chain;
    while let Some(method) = read(node) {
        names.push(method.name);
        let Some(receiver) = &method.receiver else {
            break;
        };
        node = node
            .pointer(receiver)
            .expect("a call has its receiver");
    }
    let expected = [
        "alias",
        "gt",
        "sum",
        "contains",
        "to_lowercase",
        "strip_chars",
        "col",
    ];
    assert_eq!(names, expected);
    assert_eq!(*node, json!({ "Column": "name" }));

    let alias = read(&chain).expect("the chain ends in a call");
    assert_eq!(*arg(&chain, &alias.options, "name"), json!("many"));

    let contains = &chain["Alias"][0]["BinaryExpr"]["left"]["Agg"]["Sum"];
    let method = read(contains).expect("the chain holds a call");
    assert_eq!(method.name, "contains");
    assert_eq!(*arg(contains, &method.inputs, "pattern"), json_of(lit("x")));
    assert_eq!(*arg(contains, &method.options, "literal"), json!(false));
    assert_eq!(*arg(contains, &method.options, "strict"), json!(false));
}

#[test]
fn a_method_says_what_polars_says_of_it_and_of_its_arguments() {
    let contains = method(methods(), "str", "contains");
    let doc = contains
        .doc
        .expect("Python has it");
    assert_eq!(
        doc.says,
        "Check if the string contains a substring that matches a pattern."
    );
    assert_eq!(
        contains.says_of("literal"),
        Some("Treat `pattern` as a literal string, not as a regular expression.")
    );
    assert_eq!(contains.says_of("none"), None);

    // the function of a name is another than the method of it
    let says = |is_function: bool| {
        methods()
            .iter()
            .find(|method| {
                method
                    .namespace
                    .is_empty()
                    && method.name == "len"
                    && method
                        .receiver
                        .is_none()
                        == is_function
            })
            .and_then(|method| method.doc)
            .map(|doc| doc.says)
    };
    assert!(says(true).is_some());
    assert_ne!(says(true), says(false));

    // all but a few, which Python has under another name or not at all
    let untold = methods()
        .iter()
        .filter(|method| method.doc.is_none())
        .count();
    assert!(untold <= 13, "{untold} methods say nothing");
}
