//! The `run` command on a pipeline that only writes.

use std::fs::File;
use std::io::Write;
use std::process::{Command, Stdio};

use polaroid::polars;
use polaroid::polars::prelude::*;

#[test]
fn a_pipeline_with_no_output_only_writes() {
    let directory = std::env::temp_dir();
    let read = directory.join(format!("polaroid-run-{}-in.parquet", std::process::id()));
    let written = directory.join(format!("polaroid-run-{}-out.parquet", std::process::id()));
    let mut frame = df!("x" => [1, 2, 3]).unwrap();
    ParquetWriter::new(File::create(&read).unwrap())
        .finish(&mut frame)
        .unwrap();

    let pipeline = serde_json::json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "all", "block": "scan_parquet", "args": { "path": read } },
            { "id": "kept", "block": "write_parquet", "inputs": ["all"], "args": { "path": written } },
        ],
        "outputs": [],
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_polaroid"))
        .args(["run", "-"])
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            pipeline
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    let output = child
        .wait_with_output()
        .unwrap();

    assert!(
        output
            .status
            .success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output
            .stdout
            .is_empty()
    );
    let kept = ParquetReader::new(File::open(&written).unwrap())
        .finish()
        .unwrap();
    assert!(kept.equals(&frame));

    std::fs::remove_file(read).unwrap();
    std::fs::remove_file(written).unwrap();
}
