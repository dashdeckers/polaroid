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
