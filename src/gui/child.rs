//! Check and Run in a child process of this program, so that a crash is an error and a stop a kill.

use std::collections::{BTreeMap, HashMap};
use std::process::Stdio;

use tokio::io::AsyncWriteExt;

use super::Columns;
use crate::cli::CHILD;
use crate::polars::prelude::{DataFrame, PolarsResult, polars_ensure};
use crate::{Pipeline, stream};

/// Every node's columns or error, from this program's `check` command.
pub async fn check(pipeline: Pipeline) -> Result<BTreeMap<String, Columns>, String> {
    let pipeline = serde_json::to_vec(&pipeline).expect("a pipeline serializes");
    let checks = child(&["check", "-"], pipeline)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&checks).map_err(|e| e.to_string())
}

/// The frames of the outputs by node id, from `collect`; a node in `known` is not computed.
pub async fn run(
    pipeline: Pipeline,
    known: HashMap<String, DataFrame>,
) -> Result<Vec<(String, DataFrame)>, String> {
    let frames = async {
        let mut input = Vec::new();
        let pipeline = serde_json::to_vec(&pipeline).expect("a pipeline serializes");
        stream::put(&mut input, &pipeline)?;
        for (id, mut frame) in known {
            stream::write(&mut input, &id, &mut frame)?;
        }
        stream::read(&child(&["collect"], input).await?)
    };
    frames
        .await
        .map_err(|e| e.to_string())
}

/// What this program's `command` writes to stdout, given `input`; killed if this is dropped.
async fn child(
    command: &[&str],
    input: Vec<u8>,
) -> PolarsResult<Vec<u8>> {
    let mut child = tokio::process::Command::new(std::env::current_exe()?)
        .args(command)
        .env(CHILD, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    // dropped once written, which ends what the child reads
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(&input)
        .await?;
    let output = child
        .wait_with_output()
        .await?;

    polars_ensure!(
        output.status.success(),
        ComputeError: "{}{}", String::from_utf8_lossy(&output.stderr), output.status
    );
    Ok(output.stdout)
}
