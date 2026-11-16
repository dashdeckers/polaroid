//! Check and Run in a child process of this program, so that a crash is an error and a stop a kill.

use std::collections::BTreeMap;
use std::process::Stdio;

use tokio::io::AsyncWriteExt;

use super::Columns;
use crate::cli::CHILD;
use crate::polars::prelude::{DataFrame, PolarsResult, polars_err};
use crate::{Pipeline, stream};

/// Every node's columns or error, from this program's `check` command.
pub async fn check(pipeline: Pipeline) -> Result<BTreeMap<String, Columns>, String> {
    let pipeline = serde_json::to_vec(&pipeline).map_err(|e| e.to_string())?;
    let checks = child(&["check", "-"], pipeline).await?;
    serde_json::from_slice(&checks).map_err(|e| e.to_string())
}

/// The frames of the outputs by node id, from `collect`; a node in `known` is not computed.
pub async fn run(
    pipeline: Pipeline,
    known: Vec<(String, DataFrame)>,
) -> Result<Vec<(String, DataFrame)>, String> {
    let input = (|| {
        let mut input = Vec::new();
        let pipeline =
            serde_json::to_vec(&pipeline).map_err(|e| polars_err!(ComputeError: "{e}"))?;
        stream::put(&mut input, &pipeline)?;
        for (id, mut frame) in known {
            stream::write(&mut input, &id, &mut frame)?;
        }
        PolarsResult::Ok(input)
    })()
    .map_err(|e| e.to_string())?;

    let frames = child(&["collect"], input).await?;
    stream::read(&frames).map_err(|e| e.to_string())
}

/// What this program's `command` writes to stdout, given `input`; killed if this is dropped.
async fn child(
    command: &[&str],
    input: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let output = async {
        let mut child = tokio::process::Command::new(std::env::current_exe()?)
            .args(command)
            .env(CHILD, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // dropped once written, which ends what the child reads
        let mut stdin = child
            .stdin
            .take()
            .expect("stdin is piped");
        stdin
            .write_all(&input)
            .await?;
        drop(stdin);

        child
            .wait_with_output()
            .await
    }
    .await
    .map_err(|e| e.to_string())?;

    match output
        .status
        .success()
    {
        true => Ok(output.stdout),
        false => Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            output.status
        )),
    }
}
