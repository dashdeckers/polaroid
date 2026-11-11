//! Checks of the Methods against Polars: the JSON Schema of its expressions,
//! and what it makes of the nodes they build.

use std::collections::BTreeSet;

use polars::prelude::*;
use schemars::generate::SchemaSettings;
use serde_json::{Value, json};

use super::{Arg, Method, methods, others, read, rows, text};
use crate::schema::{self, resolve, variants};

/// The `$defs` of the JSON Schema of an expression.
fn defs() -> Value {
    let mut generator = SchemaSettings::draft2020_12().into_generator();
    generator.subschema_for::<Expr>();
    json!(generator.definitions())
}

/// Whether the schema has an expression where `method` has its receiver and
/// each input.
fn inputs_fit(
    defs: &Value,
    method: &Method,
) -> bool {
    let expr = &defs["Expr"];
    let is_expr = |at: &String| schema::at(defs, expr, at) == expr;
    method
        .receiver
        .iter()
        .all(is_expr)
        && method
            .inputs
            .iter()
            .all(|input| match input.is_variadic {
                true => is_expr(&format!("{}/0", input.at)),
                false => is_expr(&input.at),
            })
}

/// Whether each option of `method` is a part of a payload of this schema: a
/// field of a struct, a part of a tuple, or the payload as a whole.
fn options_fit(
    defs: &Value,
    method: &Method,
    payload: Option<&Value>,
) -> bool {
    let payload = resolve(defs, payload.unwrap_or(&Value::Null));
    method
        .options
        .iter()
        .all(|option| {
            let is_in_schema = !schema::at(defs, &defs["Expr"], &option.at).is_null();
            let key = option
                .at
                .strip_prefix(&method.kind)
                .expect("an option is in the payload")
                .trim_start_matches('/');
            let is_field = payload["properties"]
                .get(key)
                .is_some()
                && option.name == key;
            let is_part = key
                .parse::<usize>()
                .is_ok_and(|i| !payload["prefixItems"][i].is_null());
            let is_whole = key.is_empty() && !payload.is_null();
            is_in_schema && (is_field || is_part || is_whole)
        })
}

fn json_of(expr: Expr) -> Value {
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

fn method<'a>(
    methods: &'a [Method],
    namespace: &str,
    name: &str,
) -> &'a Method {
    methods
        .iter()
        .find(|method| method.namespace == namespace && method.name == name)
        .expect("the method is in the table")
}

/// The node of `method` called on a column named `receiver`, each input
/// without default a column of its own name.
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

/// Every kind of node in the schema is read by a Method, or is listed as read
/// by none. The schema has an expression where a Method has its receiver or
/// an input, and each of its options is a part of the payload of its node.
#[test]
fn every_node_is_a_method_or_listed() {
    let mut problems = Vec::new();
    let mut methods = Vec::new();
    for row in rows() {
        match row {
            Ok(method) => methods.push(method),
            Err(problem) => problems.push(problem),
        }
    }
    let others = others();

    // the enums that tell the kinds apart, the expression itself among them
    let kinds = methods
        .iter()
        .map(|method| method.kind.as_str())
        .chain(
            others
                .iter()
                .copied(),
        );
    let enums: BTreeSet<&str> = kinds
        .clone()
        .filter_map(|kind| kind.rsplit_once('/'))
        .map(|(at, _)| at)
        .chain([""])
        .collect();

    let defs = defs();
    for at in &enums {
        let nodes = variants(schema::at(&defs, &defs["Expr"], at));
        if nodes.is_empty() {
            problems.push(format!("{at}: the schema has no enum there"));
        }

        for (variant, payload) in &nodes {
            let kind = format!("{at}/{variant}");
            let readers: Vec<&Method> = methods
                .iter()
                .filter(|method| method.kind == kind)
                .collect();
            let is_other = others.contains(&kind.as_str());
            // its kinds are told by an enum further in
            let is_split = enums
                .iter()
                .filter_map(|inner| inner.strip_prefix(kind.as_str()))
                .any(|rest| rest.is_empty() || rest.starts_with('/'));

            match (readers.is_empty(), is_other, is_split) {
                (true, false, false) => problems.push(format!("{kind}: no method reads it")),
                (true, true, false) | (true, false, true) => {}
                (false, false, false) => {
                    for method in readers {
                        if !inputs_fit(&defs, method) {
                            problems.push(format!(
                                "{kind}: {} has a receiver or input where the schema has no \
                                 expression",
                                method.name,
                            ));
                        }
                        if !options_fit(&defs, method, *payload) {
                            problems.push(format!(
                                "{kind}: {} has the options {:?}, its payload is {}",
                                method.name,
                                method.options,
                                payload.unwrap_or(&Value::Null),
                            ));
                        }
                    }
                }
                _ => problems.push(format!(
                    "{kind}: it is more than one of read, listed and split"
                )),
            }
        }

        let here = kinds
            .clone()
            .filter_map(|kind| kind.rsplit_once('/'))
            .filter(|(of, _)| of == at);
        for (_, variant) in here {
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

/// Polars takes the node of every Method and gives the same node back, and no
/// other Method reads it.
#[test]
fn every_method_round_trips() {
    let methods = methods();
    let label = |method: &Method| format!("{}.{} ({})", method.namespace, method.name, method.kind);
    let mut problems = Vec::new();

    for method in &methods {
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

/// A namespace has a name once as a method and once as a function at most.
#[test]
fn every_name_is_unique() {
    let mut seen = BTreeSet::new();
    for method in methods() {
        let is_function = method
            .receiver
            .is_none();
        let name = (method.namespace, method.name, is_function);
        assert!(seen.insert(name), "{name:?} is in the table twice");
    }
}

/// A call with its inputs filled is the node that the Rust method builds.
#[test]
fn a_call_is_what_polars_builds() {
    let methods = methods();

    let contains = filled(method(&methods, "str", "contains"));
    let built = col("receiver")
        .str()
        .contains(col("pattern"), true);
    assert_eq!(contains, json_of(built));

    // the receiver is inside the payload, beside the options
    let cast = filled(method(&methods, "", "cast"));
    assert_eq!(cast, json_of(col("receiver").strict_cast(DataType::String)));

    // an operator is told by a field
    let add = filled(method(&methods, "", "add"));
    assert_eq!(add, json_of(col("receiver") + col("other")));

    // a function has no receiver, and a variadic input is a list
    let coalesce_ = method(&methods, "", "coalesce");
    assert_eq!(coalesce_.receiver, None);
    assert_eq!(filled(coalesce_), json_of(coalesce(&[col("exprs")])));

    // two methods of one node, told by its payload
    let clip_min = method(&methods, "", "clip_min");
    assert!(clip_min.reads(&json_of(col("a").clip_min(lit(0)))));
    assert!(!clip_min.reads(&json_of(col("a").clip_max(lit(0)))));
    assert!(
        clip_min
            .options
            .is_empty()
    );
}

/// An expression reads as a chain from its last call back to its root, each
/// call with its arguments.
#[test]
fn a_chain_reads_from_its_last_call() {
    let methods = methods();
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
    while let Some(method) = read(&methods, node) {
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

    let alias = read(&methods, &chain).expect("the chain ends in a call");
    assert_eq!(*arg(&chain, &alias.options, "name"), json!("many"));

    let contains = &chain["Alias"][0]["BinaryExpr"]["left"]["Agg"]["Sum"];
    let method = read(&methods, contains).expect("the chain holds a call");
    assert_eq!(method.name, "contains");
    assert_eq!(*arg(contains, &method.inputs, "pattern"), json_of(lit("x")));
    assert_eq!(*arg(contains, &method.options, "literal"), json!(false));
    assert_eq!(*arg(contains, &method.options, "strict"), json!(false));
}

/// An expression is written as Python writes its calls: arguments at their
/// defaults are left out, and literal arguments are bare values.
#[test]
fn an_expression_is_written_as_python_does() {
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
        r#"pl.col("name").str.strip_chars(" ").str.to_lowercase().str.contains("x", strict=False).sum().gt(2).alias("many")"#
    );

    // an input at its default is left out, and those after it go by name
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
    assert_eq!(written(replace), r#"pl.col("d").dt.replace(day=1)"#);

    // functions, with a variadic input and with a default
    assert_eq!(
        written(coalesce(&[col("a"), col("b"), lit(0)])),
        r#"pl.coalesce(pl.col("a"), pl.col("b"), 0)"#
    );
    let choice = when(col("a").is_null())
        .then(lit("none"))
        .otherwise(lit(NULL));
    assert_eq!(written(choice), r#"pl.when(pl.col("a").is_null(), "none")"#);

    // a data type that is a name, and a literal that is no argument
    assert_eq!(
        written(lit(5).strict_cast(DataType::Int8)),
        "pl.lit(5).cast(pl.Int8)"
    );

    // a hole, and a node that no method reads
    let contains = method(&methods, "str", "contains").call(json_of(col("a")));
    assert_eq!(text(&methods, &contains), r#"pl.col("a").str.contains(_)"#);
    let unread = json!({ "Display": { "inputs": [], "fmt_str": "" } });
    assert_eq!(text(&methods, &unread), "<Display>");
}
