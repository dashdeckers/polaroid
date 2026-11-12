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
