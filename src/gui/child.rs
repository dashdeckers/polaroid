//! Check and Run in a child process of this program, so that a crash is an error and a stop a kill.

use std::collections::{BTreeMap, HashMap};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};

use super::Columns;
use crate::cli::CHILD;
use crate::polars::prelude::{DataFrame, PolarsResult, polars_bail, polars_err};
use crate::{Pipeline, stream};

/// The process that checks and runs for the editor: started when it is first asked, and kept.
///
/// One that is kept has Polars up and running, which a new one takes its first moments for.
#[derive(Clone, Default)]
pub struct Worker(Arc<Mutex<Option<Child>>>);

impl Worker {
    /// Every node's columns or error; a node in `known` has those of that frame, unlooked at.
    pub async fn check(
        self,
        pipeline: Pipeline,
        known: HashMap<String, DataFrame>,
    ) -> Result<BTreeMap<String, Columns>, String> {
        let checks = self
            .ask("check", &pipeline, known)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&checks).map_err(|e| e.to_string())
    }

    /// The frames of the outputs by node id; a node in `known` is not computed.
    pub async fn run(
        self,
        pipeline: Pipeline,
        known: HashMap<String, DataFrame>,
    ) -> Result<Vec<(String, DataFrame)>, String> {
        let frames = async {
            stream::read(
                &self
                    .ask("collect", &pipeline, known)
                    .await?,
            )
        };
        frames
            .await
            .map_err(|e| e.to_string())
    }

    /// What the process answers when asked for `what` of `pipeline` with these frames saved.
    ///
    /// It is this future's alone until it has answered: dropped before that, it is killed.
    async fn ask(
        &self,
        what: &str,
        pipeline: &Pipeline,
        known: HashMap<String, DataFrame>,
    ) -> PolarsResult<Vec<u8>> {
        let mut asked = Vec::new();
        stream::put(&mut asked, what.as_bytes())?;
        let pipeline = serde_json::to_vec(pipeline).expect("a pipeline serializes");
        stream::put(&mut asked, &pipeline)?;
        for (id, mut frame) in known {
            stream::write(&mut asked, &id, &mut frame)?;
        }

        let kept = self
            .0
            .lock()
            .expect("no holder of the lock panics")
            .take();
        let mut child = match kept {
            Some(child) => child,
            None => Command::new(std::env::current_exe()?)
                .arg("serve")
                .env(CHILD, "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()?,
        };
        let answer = answer(&mut child, &asked).await;
        let Ok(answer) = answer else {
            // it ended: what it last said is why
            let output = child
                .wait_with_output()
                .await?;
            polars_bail!(
                ComputeError: "{}{}", String::from_utf8_lossy(&output.stderr), output.status
            );
        };
        *self
            .0
            .lock()
            .expect("no holder of the lock panics") = Some(child);

        let mut answer = answer.as_slice();
        match stream::part(&mut answer)? {
            b"ok" => Ok(answer.to_vec()),
            _ => Err(polars_err!(
                ComputeError: "{}", String::from_utf8_lossy(stream::part(&mut answer)?)
            )),
        }
    }
}

/// What `child` writes to stdout when `asked` on stdin: a part after its length, as it was asked.
async fn answer(
    child: &mut Child,
    asked: &[u8],
) -> std::io::Result<Vec<u8>> {
    let stdin = child
        .stdin
        .as_mut()
        .expect("stdin is piped");
    stdin
        .write_all(&(asked.len() as u64).to_le_bytes())
        .await?;
    stdin
        .write_all(asked)
        .await?;
    stdin
        .flush()
        .await?;

    let stdout = child
        .stdout
        .as_mut()
        .expect("stdout is piped");
    let mut len = [0; 8];
    stdout
        .read_exact(&mut len)
        .await?;
    let mut answer = vec![0; u64::from_le_bytes(len) as usize];
    stdout
        .read_exact(&mut answer)
        .await?;
    Ok(answer)
}
