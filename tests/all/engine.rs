//! Running a Pipeline: known frames, and nodes that write.

use std::collections::HashMap;

use polaroid::polars::prelude::*;
use polaroid::{Pipeline, Registry, is_sink, polars};

fn pipeline(path: &str) -> Pipeline {
    let pipeline = serde_json::json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "all", "block": "scan_parquet", "args": { "path": path } },
            { "id": "some", "block": "limit", "inputs": ["all"], "args": { "n": 2 } },
        ],
        "outputs": ["some"],
    });
    serde_json::from_value(pipeline).unwrap()
}

#[test]
fn a_known_frame_is_not_computed() {
    let registry = Registry::with_polars();
    let pipeline = pipeline("no such file.parquet");
    assert!(
        pipeline
            .run(&registry)
            .is_err()
    );

    let known = HashMap::from([("all".to_owned(), df!("x" => [1, 2, 3]).unwrap())]);
    let frames = pipeline
        .run_from(&registry, &known)
        .unwrap();
    assert_eq!(frames.len(), 1);
    assert!(frames[0].equals(&df!("x" => [1, 2]).unwrap()));
}

#[test]
fn a_known_frame_gives_its_columns_to_a_check_and_is_not_looked_at() {
    let registry = Registry::with_polars();
    let pipeline = pipeline("no such file.parquet");
    let columns = |known: &HashMap<String, DataFrame>| -> Vec<Option<Vec<String>>> {
        let schemas = pipeline
            .schemas_from(&registry, known)
            .unwrap();
        ["all", "some"]
            .map(|id| {
                let schema = schemas[id]
                    .as_ref()
                    .ok()?;
                Some(
                    schema
                        .iter_names()
                        .map(ToString::to_string)
                        .collect(),
                )
            })
            .to_vec()
    };

    // the file is not there, so neither the node that reads it nor the next has columns
    assert_eq!(columns(&HashMap::new()), [None, None]);
    // an empty frame of its columns stands in for the first, and the next has them of it
    let known = HashMap::from([("all".to_owned(), df!("x" => [0; 0]).unwrap())]);
    let x = Some(vec!["x".to_owned()]);
    assert_eq!(columns(&known), [x.clone(), x]);
}

#[test]
fn a_write_runs_and_gives_its_frame() {
    let registry = Registry::with_polars();
    let path = std::env::temp_dir().join(format!("polaroid-write-{}.parquet", std::process::id()));
    let pipeline = serde_json::json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "all", "block": "scan_parquet", "args": { "path": "no such file.parquet" } },
            { "id": "written", "block": "write_parquet", "inputs": ["all"], "args": { "path": path } },
            { "id": "after", "block": "limit", "inputs": ["written"], "args": { "n": 1 } },
        ],
        "outputs": [],
    });
    let mut pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();

    // planning a write does not do it
    let plans = pipeline
        .frames(&registry)
        .unwrap();
    assert!(is_sink(
        plans["written"]
            .as_ref()
            .unwrap()
    ));
    assert!(plans["after"].is_ok());
    assert!(!path.exists());

    let frame = df!("x" => [1, 2, 3]).unwrap();
    let known = HashMap::from([("all".to_owned(), frame.clone())]);
    let frames = pipeline
        .run_from(&registry, &known)
        .unwrap();
    assert!(frames.is_empty());
    let written = LazyFrame::scan_parquet(
        PlRefPath::new(
            path.to_str()
                .unwrap(),
        ),
        Default::default(),
    )
    .unwrap()
    .collect()
    .unwrap();
    assert!(written.equals(&frame));
    std::fs::remove_file(&path).unwrap();

    // a write gives the frame that it writes, to an output and a reader
    pipeline.outputs = vec![
        "written".to_owned(),
        "after".to_owned(),
    ];
    let frames = pipeline
        .run_from(&registry, &known)
        .unwrap();
    assert_eq!(frames.len(), 2);
    assert!(frames[0].equals(&frame));
    assert_eq!(frames[1].height(), 1);
    assert!(path.exists());
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn a_node_that_cannot_be_built_fails_the_run() {
    let mut pipeline = pipeline("no such file.parquet");
    pipeline
        .outputs
        .clear();
    pipeline.nodes[1].args = serde_json::json!({ "m": 2 });

    let known = HashMap::from([("all".to_owned(), df!("x" => [1]).unwrap())]);
    let failed = pipeline.run_from(&Registry::with_polars(), &known);
    assert!(failed.is_err());
}

#[test]
fn an_output_is_a_node_and_named_once() {
    let registry = Registry::with_polars();
    let mut pipeline = pipeline("no such file.parquet");

    pipeline.outputs = vec!["none".to_owned()];
    assert!(
        pipeline
            .schemas(&registry)
            .is_err()
    );
    pipeline.outputs = vec![
        "some".to_owned(),
        "some".to_owned(),
    ];
    assert!(
        pipeline
            .schemas(&registry)
            .is_err()
    );
}

#[test]
fn a_node_that_reads_no_source_is_left_out() {
    let registry = Registry::with_polars();
    let mut pipeline = pipeline("no such file.parquet");
    let draft = serde_json::json!([
        { "id": "draft", "block": "limit", "inputs": [""], "args": { "n": 1 } },
        { "id": "below", "block": "write_parquet", "inputs": ["draft"], "args": { "path": "never.parquet" } },
    ]);
    pipeline
        .nodes
        .extend(serde_json::from_value::<Vec<polaroid::Node>>(draft).unwrap());

    let known = HashMap::from([("all".to_owned(), df!("x" => [1, 2, 3]).unwrap())]);
    let frames = pipeline
        .run_from(&registry, &known)
        .unwrap();
    assert_eq!(frames[0].height(), 2);

    // but not as an output, which is asked for
    pipeline
        .outputs
        .push("draft".to_owned());
    assert!(
        pipeline
            .run_from(&registry, &known)
            .is_err()
    );
}

#[test]
fn a_scanned_file_is_read_by_its_name_and_not_as_a_pattern() {
    let directory = std::env::temp_dir().join(format!("polaroid-names-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for (name, rows) in [
        ("sales[2024].parquet", 2),
        ("sales2.parquet", 3),
    ] {
        let mut frame = df!("x" => vec![name; rows]).unwrap();
        ParquetWriter::new(std::fs::File::create(directory.join(name)).unwrap())
            .finish(&mut frame)
            .unwrap();
    }

    let mut pipeline = pipeline(
        directory
            .to_str()
            .unwrap(),
    );
    pipeline.outputs = vec!["all".to_owned()];
    let frames = pipeline
        .run(&Registry::with_polars())
        .unwrap();
    assert_eq!(frames[0].height(), 5);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_expression_is_used_by_its_name() {
    let pipeline = |name: &str| {
        let pipeline = serde_json::json!({
            "polars": polars::VERSION,
            "nodes": [
                { "id": "all", "block": "scan_parquet", "args": { "path": "" } },
                {
                    "id": "some",
                    "block": "filter",
                    "inputs": ["all"],
                    "args": { "predicates": [{ "$expr": name }] },
                },
            ],
            "outputs": ["some"],
            "expressions": { "large": col("x").gt(lit(1)) },
        });
        serde_json::from_value::<Pipeline>(pipeline).unwrap()
    };
    let frame = df!("x" => [1, 2, 3]).unwrap();
    let known = HashMap::from([("all".to_owned(), frame)]);
    let registry = Registry::with_polars();

    let frames = pipeline("large")
        .run_from(&registry, &known)
        .unwrap();
    assert_eq!(frames[0], df!("x" => [2, 3]).unwrap());

    let unnamed = pipeline("small")
        .run_from(&registry, &known)
        .unwrap_err();
    assert!(
        unnamed
            .to_string()
            .contains("no expression is called 'small'")
    );
}

#[test]
fn a_block_is_in_the_category_named_before_it() {
    use polaroid::frames::Limit;

    let catalog = Registry::default()
        .category("Reshape")
        .register::<Limit, _>("first")
        .category("Mine")
        .register::<Limit, _>("second")
        // a category named again is where it was
        .category("Reshape")
        .register::<Limit, _>("third")
        .catalog();

    let categories = ["Reshape", "Mine"];
    assert_eq!(catalog.categories, categories);
    let category = |block: &str| catalog.blocks[block].category;
    assert_eq!(category("first"), "Reshape");
    assert_eq!(category("second"), "Mine");
    assert_eq!(category("third"), "Reshape");
}

/// A scan `a` read by `b` and `c`, `d` that reads `b`, and `e` that reads nothing yet.
fn graph() -> Pipeline {
    let node = |id: &str, inputs: &[&str]| {
        let args = serde_json::json!({ "n": 1 });
        serde_json::json!({ "id": id, "block": "limit", "inputs": inputs, "args": args })
    };
    let nodes = [
        node("a", &[]),
        node("b", &["a"]),
        node("c", &["a"]),
        node("d", &["b"]),
        node("e", &[""]),
    ];
    let pipeline = serde_json::json!({
        "polars": polars::VERSION,
        "nodes": nodes,
        "outputs": ["d", "c"],
    });
    serde_json::from_value(pipeline).unwrap()
}

/// What each Node reads, in the order of the Nodes.
fn wiring(pipeline: &Pipeline) -> Vec<(&str, Vec<&str>)> {
    pipeline
        .nodes
        .iter()
        .map(|node| {
            let inputs = node
                .inputs
                .iter()
                .map(String::as_str)
                .collect();
            (node.id.as_str(), inputs)
        })
        .collect()
}

#[test]
fn a_pipeline_starts_with_no_nodes_and_builds() {
    let pipeline = Pipeline::default();
    let frames = pipeline
        .frames(&Registry::with_polars())
        .unwrap();
    assert!(frames.is_empty());
}

#[test]
fn nodes_are_found_down_and_up_the_wires() {
    let pipeline = graph();
    fn sorted(found: std::collections::HashSet<&str>) -> Vec<&str> {
        let mut found: Vec<_> = found
            .into_iter()
            .collect();
        found.sort_unstable();
        found
    }
    assert_eq!(sorted(pipeline.downstream(["b"])), ["b", "d"]);
    assert_eq!(sorted(pipeline.downstream(["a"])), ["a", "b", "c", "d"]);
    assert_eq!(sorted(pipeline.upstream(["d"])), ["a", "b", "d"]);
    assert_eq!(sorted(pipeline.upstream(["c", "b"])), ["a", "b", "c"]);
    // what is not wired reads nothing, which is no Node
    assert_eq!(sorted(pipeline.upstream(["e"])), ["", "e"]);
}

#[test]
fn a_node_is_renamed_wherever_it_is_named() {
    let mut pipeline = graph();
    assert!(pipeline.rename("b", "x"));
    let wired = [
        ("a", vec![]),
        ("x", vec!["a"]),
        ("c", vec!["a"]),
        ("d", vec!["x"]),
        ("e", vec![""]),
    ];
    assert_eq!(wiring(&pipeline), wired);

    assert!(pipeline.rename("d", "y"));
    assert_eq!(pipeline.outputs, ["y", "c"]);

    // not to nothing, nor to the id of another, nor of itself, nor if there is no such Node
    assert!(!pipeline.rename("x", ""));
    assert!(!pipeline.rename("x", "c"));
    assert!(!pipeline.rename("x", "x"));
    assert!(!pipeline.rename("b", "z"));
    assert_eq!(wiring(&pipeline)[1], ("x", vec!["a"]));
}

#[test]
fn a_removed_node_is_no_output_and_leaves_its_readers_unwired() {
    let mut pipeline = graph();
    pipeline.remove("a");
    let wired = [
        ("b", vec![""]),
        ("c", vec![""]),
        ("d", vec!["b"]),
        ("e", vec![""]),
    ];
    assert_eq!(wiring(&pipeline), wired);

    pipeline.remove("d");
    assert_eq!(pipeline.outputs, ["c"]);
}

#[test]
fn an_expression_is_renamed_in_its_place_and_where_it_is_used() {
    use polaroid::{marked, marker};

    let pipeline = serde_json::json!({
        "polars": polars::VERSION,
        "nodes": [
            {
                "id": "some",
                "block": "filter",
                "inputs": [""],
                "args": { "predicates": [{ "$expr": "large" }, { "$expr": "odd" }] },
            },
            { "id": "few", "block": "limit", "inputs": ["some"], "args": { "n": 1 } },
        ],
        "outputs": [],
        "expressions": { "first": null, "large": null, "odd": null },
    });
    let mut pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    assert_eq!(pipeline.users("large"), ["some"]);

    assert!(pipeline.rename_expression("large", "big"));
    let names: Vec<_> = pipeline
        .expressions
        .keys()
        .collect();
    assert_eq!(names, ["first", "big", "odd"]);
    let predicates = &pipeline.nodes[0].args["predicates"];
    assert_eq!(predicates[0], marker("big"));
    assert_eq!(marked(&predicates[1]), Some("odd"));
    assert_eq!(pipeline.users("big"), ["some"]);
    assert!(
        pipeline
            .users("large")
            .is_empty()
    );

    // not to nothing, nor to the name of another, nor if there is no such expression
    assert!(!pipeline.rename_expression("big", ""));
    assert!(!pipeline.rename_expression("big", "odd"));
    assert!(!pipeline.rename_expression("large", "huge"));

    // a marker is that alone: with anything beside its name it is a value like any other
    assert_eq!(
        marked(&serde_json::json!({ "$expr": "big", "more": 1 })),
        None
    );
}

#[test]
fn two_nodes_swap_places_in_the_wiring() {
    use serde_json::json;

    let node = |id: &str, block: &str, inputs: &[&str]| json!({ "id": id, "block": block, "inputs": inputs });
    let pipeline = json!({
        "polars": polars::VERSION,
        "nodes": [
            node("read", "scan_parquet", &[]),
            node("first", "limit", &["read"]),
            node("second", "unique", &["first"]),
            node("last", "drop_nulls", &["second"]),
        ],
        "outputs": ["second"],
    });
    let mut pipeline: Pipeline = serde_json::from_value(pipeline).unwrap();
    let inputs = |pipeline: &Pipeline, id: &str| {
        pipeline
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap()
            .inputs
            .clone()
    };

    // one that read the other is read by it now, and their readers follow
    assert!(pipeline.swap("first", "second"));
    assert_eq!(inputs(&pipeline, "second"), ["read"]);
    assert_eq!(inputs(&pipeline, "first"), ["second"]);
    assert_eq!(inputs(&pipeline, "last"), ["first"]);
    // each is the node it was, and an output if it was one
    assert_eq!(pipeline.nodes[1].block, "limit");
    assert_eq!(pipeline.outputs, ["second"]);

    // back again, by the same swap from the other side
    assert!(pipeline.swap("second", "first"));
    assert_eq!(inputs(&pipeline, "first"), ["read"]);
    assert_eq!(inputs(&pipeline, "last"), ["second"]);

    assert!(!pipeline.swap("first", "first"));
    assert!(!pipeline.swap("first", "none"));
}
