//! The `serve` command, which the editor keeps running as a child process to ask things of.

use std::fs::File;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

use polaroid::polars;
use polaroid::polars::prelude::*;

/// Starts `serve`, as the editor or as a user would.
fn serve(is_child: bool) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_polaroid"));
    command
        .arg("serve")
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match is_child {
        true => command.env("POLAROID_CHILD", "1"),
        false => command.env_remove("POLAROID_CHILD"),
    };
    command
        .spawn()
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

/// What `served` answers when asked for `what` of `pipeline` with these frames known: whether
/// all went well, and what it says.
fn ask(
    served: &mut Child,
    what: &str,
    pipeline: &serde_json::Value,
    known: &[(&str, DataFrame)],
) -> (bool, Vec<u8>) {
    let mut asked = Vec::new();
    put(&mut asked, what.as_bytes());
    put(
        &mut asked,
        pipeline
            .to_string()
            .as_bytes(),
    );
    for (id, frame) in known {
        let mut arrow = Vec::new();
        IpcWriter::new(&mut arrow)
            .finish(&mut frame.clone())
            .unwrap();
        put(&mut asked, id.as_bytes());
        put(&mut asked, &arrow);
    }
    let mut whole = Vec::new();
    put(&mut whole, &asked);
    let stdin = served
        .stdin
        .as_mut()
        .unwrap();
    stdin
        .write_all(&whole)
        .unwrap();
    stdin
        .flush()
        .unwrap();

    let stdout = served
        .stdout
        .as_mut()
        .unwrap();
    let mut len = [0; 8];
    stdout
        .read_exact(&mut len)
        .unwrap();
    let mut answer = vec![0; u64::from_le_bytes(len) as usize];
    stdout
        .read_exact(&mut answer)
        .unwrap();
    let mut answer = answer.as_slice();
    let is_well = part(&mut answer) == b"ok";
    (is_well, answer.to_vec())
}

/// The heights of the frames in an answer, by node id.
fn heights(answer: (bool, Vec<u8>)) -> Vec<(String, usize)> {
    let (is_well, answer) = answer;
    assert!(is_well, "{}", String::from_utf8_lossy(&answer));
    let mut bytes = answer.as_slice();
    let mut heights = Vec::new();
    while !bytes.is_empty() {
        let id = String::from_utf8(part(&mut bytes).to_vec()).unwrap();
        let frame = IpcReader::new(std::io::Cursor::new(part(&mut bytes)))
            .finish()
            .unwrap();
        heights.push((id, frame.height()));
    }
    heights
}

#[test]
fn one_process_answers_one_thing_after_another_for_the_editor_alone() {
    let path = std::env::temp_dir().join(format!("polaroid-serve-{}.parquet", std::process::id()));
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
    let mut served = serve(true);

    // every output, in their order
    assert_eq!(
        heights(ask(&mut served, "collect", &pipeline, &[])),
        [
            ("some".to_owned(), 2),
            ("all".to_owned(), 4)
        ]
    );

    // a known frame stands in for its node
    let known = [("all", df!("x" => [9]).unwrap())];
    pipeline["outputs"] = serde_json::json!(["some"]);
    assert_eq!(
        heights(ask(&mut served, "collect", &pipeline, &known)),
        [("some".to_owned(), 1)]
    );

    // the columns of every node, as the `check` command prints them
    let (is_well, checks) = ask(&mut served, "check", &pipeline, &[]);
    let checks: serde_json::Value = serde_json::from_slice(&checks).unwrap();
    assert!(is_well);
    assert_eq!(checks["some"]["Ok"][0][0], "x");

    // and those of a node that reads a known frame, which is not looked at
    let (_, checks) = ask(
        &mut served,
        "check",
        &pipeline,
        &[("all", df!("y" => [0; 0]).unwrap())],
    );
    let checks: serde_json::Value = serde_json::from_slice(&checks).unwrap();
    assert_eq!(checks["some"]["Ok"][0][0], "y");

    // what fails is told, and the next thing is answered all the same
    pipeline["nodes"][1]["args"] = serde_json::json!({ "n": "two" });
    let (is_well, problem) = ask(&mut served, "collect", &pipeline, &[]);
    assert!(!is_well);
    assert!(!problem.is_empty());
    let (is_well, _) = ask(&mut served, "neither", &pipeline, &[]);
    assert!(!is_well);

    // nothing saved is nothing to do
    pipeline["nodes"][1]["args"] = serde_json::json!({ "n": 2 });
    pipeline["outputs"] = serde_json::json!([]);
    assert_eq!(heights(ask(&mut served, "collect", &pipeline, &[])), []);

    // it ends when it is asked no more
    drop(served.stdin.take());
    assert!(
        served
            .wait()
            .unwrap()
            .success()
    );
    assert!(
        !serve(false)
            .wait()
            .unwrap()
            .success()
    );

    std::fs::remove_file(path).unwrap();
}
