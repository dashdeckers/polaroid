use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::time::Instant;

use crate::polars::prelude::*;
use crate::{Pipeline, Registry, stream};

const USAGE: &str = "usage:
  run <pipeline.json> <out-dir>   each output to <out-dir>/<id>.parquet
  run <pipeline.json>             the one output, if there is one, to stdout as Parquet
  check <pipeline.json>           every node's columns and types, or error, as JSON
  catalog                         every block's args, as JSON Schema
  gui <pipeline.json>             edit and run the pipeline visually

a <pipeline.json> of `-` is read from stdin";

/// Set for the commands that the editor runs, which add to its log.
pub(crate) const CHILD: &str = "POLAROID_CHILD";

fn start_log() {
    let started = Instant::now();
    // a fresh log, unless the editor that runs this command has started one
    let fresh = match std::env::var_os(CHILD) {
        Some(_) => Ok(()),
        None => File::create("polaroid.log").map(drop),
    };
    let log = fresh.and_then(|()| {
        OpenOptions::new()
            .append(true)
            .open("polaroid.log")
    });

    match log {
        Ok(file) => {
            // fails if a logger is already set, which then stays in charge
            let _ = fern::Dispatch::new()
                .format(move |out, message, record| {
                    let seconds = started
                        .elapsed()
                        .as_secs_f64();
                    out.finish(format_args!(
                        "{seconds:9.3} {:<5} {message}",
                        record.level()
                    ))
                })
                .chain(file)
                .apply();
        }
        Err(e) => eprintln!("polaroid.log: {e}"),
    }
}

pub fn cli(registry: Registry) -> PolarsResult<()> {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .collect();

    // a run and the editor tell of themselves, and so does what the editor runs
    let is_told = matches!(
        args.first()
            .map(String::as_str),
        Some("run" | "gui")
    );
    if is_told || std::env::var_os(CHILD).is_some() {
        start_log();
    }
    log::info!("polaroid {}", args.join(" "));

    command(registry, &args)
        .inspect(|()| log::info!("done"))
        .inspect_err(|e| log::error!("{e}"))
}

/// Every node's columns and types, or error, as JSON; a node in `known` has those of that frame.
fn checked(
    registry: &Registry,
    pipeline: &Pipeline,
    known: &HashMap<String, DataFrame>,
) -> PolarsResult<String> {
    let schemas = pipeline.schemas_from(registry, known)?;
    let checks: BTreeMap<_, _> = schemas
        .iter()
        .map(|(id, schema)| {
            let columns = match schema {
                Ok(schema) => Ok(schema
                    .iter()
                    .map(|(name, dtype)| (name.as_str(), dtype))
                    .collect::<Vec<_>>()),
                Err(e) => Err(e.to_string()),
            };
            (*id, columns)
        })
        .collect();
    Ok(serde_json::to_string_pretty(&checks).expect("columns serialize"))
}

/// What the editor asked for: a `check` of a pipeline, or to `collect` its outputs.
///
/// It asks with a pipeline and then frames that stand in for their nodes: those it has saved, or
/// to a check empty ones of the columns it has found.
fn answered(
    registry: &Registry,
    mut asked: &[u8],
) -> PolarsResult<Vec<u8>> {
    let what = stream::part(&mut asked)?;
    let pipeline: Pipeline = serde_json::from_slice(stream::part(&mut asked)?)
        .map_err(|e| polars_err!(ComputeError: "the pipeline: {e}"))?;
    log::info!("asked to {}", String::from_utf8_lossy(what));
    let known = stream::read(asked)?
        .into_iter()
        .collect();
    let mut answer = Vec::new();
    match what {
        b"check" => answer.extend(checked(registry, &pipeline, &known)?.into_bytes()),
        b"collect" => {
            for (id, mut frame) in pipeline
                .outputs
                .iter()
                .zip(pipeline.run_from(registry, &known)?)
            {
                log::info!("writing {} rows of '{id}'", frame.height());
                stream::write(&mut answer, id, &mut frame)?;
            }
        }
        _ => polars_bail!(ComputeError: "asked for neither a check nor to collect"),
    }
    log::info!("answered");
    Ok(answer)
}

#[allow(clippy::print_stdout)]
fn command(
    registry: Registry,
    args: &[String],
) -> PolarsResult<()> {
    let args: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .collect();

    match args.as_slice() {
        ["run", path, out] => {
            let pipeline = Pipeline::load(path)?;
            // an output's id names its file, so it may be nothing but a file name
            for id in &pipeline.outputs {
                let is_name = Path::new(id).file_name() == Some(OsStr::new(id));
                polars_ensure!(is_name, ComputeError: "output '{id}' cannot name a file");
            }
            let frames = pipeline.run(&registry)?;
            std::fs::create_dir_all(out)?;
            for (id, mut frame) in pipeline
                .outputs
                .iter()
                .zip(frames)
            {
                let path = format!("{out}/{id}.parquet");
                log::info!("writing {} rows to {path}", frame.height());
                ParquetWriter::new(File::create(&path)?).finish(&mut frame)?;
                println!("{} rows to {path}", frame.height());
            }
        }

        ["run", path] => {
            let pipeline = Pipeline::load(path)?;
            polars_ensure!(
                pipeline.outputs.len() <= 1,
                ComputeError: "stdout takes one output, the pipeline has {}", pipeline.outputs.len()
            );
            polars_ensure!(
                pipeline.outputs.is_empty() || !std::io::stdout().is_terminal(),
                ComputeError: "stdout is a terminal: redirect or pipe it to take the Parquet"
            );
            // with no output, only the writes of the pipeline are done
            if let Some(mut frame) = pipeline
                .run(&registry)?
                .pop()
            {
                log::info!("writing {} rows to stdout", frame.height());
                ParquetWriter::new(std::io::stdout()).finish(&mut frame)?;
            }
        }

        // for the editor alone: it asks over stdin, one thing after another, until stdin ends
        ["serve"] if std::env::var_os(CHILD).is_some() => {
            let (mut input, mut out) = (std::io::stdin().lock(), std::io::stdout().lock());
            while let Some(asked) = stream::next(&mut input)? {
                let mut answer = Vec::new();
                // what fails is told, and the next thing is asked of the same process
                match answered(&registry, &asked) {
                    Ok(body) => {
                        stream::put(&mut answer, b"ok")?;
                        answer.extend(body);
                    }
                    Err(e) => {
                        log::error!("{e}");
                        stream::put(&mut answer, b"error")?;
                        stream::put(
                            &mut answer,
                            e.to_string()
                                .as_bytes(),
                        )?;
                    }
                }
                stream::put(&mut out, &answer)?;
                out.flush()?;
            }
        }

        ["check", path] => {
            let checks = checked(&registry, &Pipeline::load(path)?, &HashMap::new())?;
            println!("{checks}");
        }

        ["catalog"] => println!(
            "{}",
            serde_json::to_string_pretty(&registry.catalog()).expect("a catalog serializes")
        ),

        #[cfg(feature = "gui")]
        ["gui", path] => crate::gui::open(registry, path)?,

        #[cfg(not(feature = "gui"))]
        ["gui", _] => polars_bail!(ComputeError: "this build has no gui: add `--features gui`"),

        _ => polars_bail!(ComputeError: "{USAGE}"),
    }

    Ok(())
}
