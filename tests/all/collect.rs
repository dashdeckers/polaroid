//! The `collect` command, which the editor runs in a child process.

use std::fs::File;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use polaroid::polars;
use polaroid::polars::prelude::*;

/// Runs `collect` on `pipeline` and its `known` frames, as the editor or as a user would.
fn collect(
    pipeline: &serde_json::Value,
    known: &[(&str, DataFrame)],
    is_child: bool,
) -> Output {
    let mut input = Vec::new();
    put(
        &mut input,
        pipeline
            .to_string()
            .as_bytes(),
    );
    for (id, frame) in known {
        let mut parquet = Vec::new();
        ParquetWriter::new(&mut parquet)
            .finish(&mut frame.clone())
            .unwrap();
        put(&mut input, id.as_bytes());
        put(&mut input, &parquet);
    }

    let mut command = Command::new(env!("CARGO_BIN_EXE_polaroid"));
    command
        .arg("collect")
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match is_child {
        true => command.env("POLAROID_CHILD", "1"),
        false => command.env_remove("POLAROID_CHILD"),
    };
    let mut child = command
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&input)
        .unwrap();
    child
        .wait_with_output()
        .unwrap()
}

/// Adds a length and that many bytes to `bytes`.
fn put(
    bytes: &mut Vec<u8>,
    part: &[u8],
) {
    bytes.extend((part.len() as u64).to_le_bytes());
    bytes.extend(part);
}

/// Takes a length and that many bytes off the front of `bytes`.
fn part<'a>(bytes: &mut &'a [u8]) -> &'a [u8] {
    let (len, rest) = bytes
        .split_first_chunk::<8>()
        .unwrap();
    let (part, rest) = rest.split_at(u64::from_le_bytes(*len) as usize);
    *bytes = rest;
    part
}

#[test]
fn every_output_is_written_for_the_editor_alone() {
    let path =
        std::env::temp_dir().join(format!("polaroid-collect-{}.parquet", std::process::id()));
    let mut frame = df!("x" => [1, 2, 3, 4]).unwrap();
    ParquetWriter::new(File::create(&path).unwrap())
        .finish(&mut frame)
        .unwrap();

    let mut pipeline = serde_json::json!({
        "polars": polars::VERSION,
        "nodes": [
            { "id": "all", "block": "scan_parquet", "args": { "path": path } },
            { "id": "some", "block": "limit", "inputs": ["all"], "args": { "n": 2 } },
        ],
        "outputs": ["some", "all"],
    });

    // the heights of the frames written, by node id
    let heights = |output: Output| {
        assert!(
            output
                .status
                .success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut bytes = output
            .stdout
            .as_slice();
        let mut heights = Vec::new();
        while !bytes.is_empty() {
            let id = String::from_utf8(part(&mut bytes).to_vec()).unwrap();
            let frame = ParquetReader::new(std::io::Cursor::new(part(&mut bytes)))
                .finish()
                .unwrap();
            heights.push((id, frame.height()));
        }
        heights
    };

    assert_eq!(
        heights(collect(&pipeline, &[], true)),
        [
            ("some".to_owned(), 2),
            ("all".to_owned(), 4)
        ]
    );

    // a known frame stands in for its node
    let known = [("all", df!("x" => [9]).unwrap())];
    pipeline["outputs"] = serde_json::json!(["some"]);
    assert_eq!(
        heights(collect(&pipeline, &known, true)),
        [("some".to_owned(), 1)]
    );

    assert!(
        !collect(&pipeline, &[], false)
            .status
            .success()
    );

    // nothing saved is nothing to do
    pipeline["outputs"] = serde_json::json!([]);
    let output = collect(&pipeline, &[], true);
    assert!(
        output
            .status
            .success()
    );
    assert!(
        output
            .stdout
            .is_empty()
    );

    std::fs::remove_file(path).unwrap();
}
