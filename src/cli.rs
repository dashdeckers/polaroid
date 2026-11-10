use std::collections::BTreeMap;
use std::fs::File;
use std::io::IsTerminal;
use std::time::Instant;

use crate::polars::prelude::*;
use crate::{Pipeline, Registry};

const USAGE: &str = "usage:
  run <pipeline.json> <out-dir>   each output to <out-dir>/<id>.parquet
  run <pipeline.json>             the one output to stdout, as Parquet
  check <pipeline.json>           every node's columns and types, or error, as JSON
  catalog                         every block's args, as JSON Schema

a <pipeline.json> of `-` is read from stdin";

fn start_log() {
    let started = Instant::now();
    match File::create("polaroid.log") {
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

    start_log();
    log::info!("polaroid {}", args.join(" "));

    command(registry, &args)
        .inspect(|()| log::info!("done"))
        .inspect_err(|e| log::error!("{e}"))
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
        ["run", pipeline, out] => {
            let pipeline = Pipeline::load(pipeline)?;
            std::fs::create_dir_all(out)?;
            for (id, mut frame) in pipeline
                .outputs
                .iter()
                .zip(pipeline.run(&registry)?)
            {
                let path = format!("{out}/{id}.parquet");
                log::info!("writing {} rows to {path}", frame.height());
                ParquetWriter::new(File::create(&path)?).finish(&mut frame)?;
                println!("{} rows to {path}", frame.height());
            }
        }

        ["run", pipeline] => {
            polars_ensure!(
                !std::io::stdout().is_terminal(),
                ComputeError: "stdout is a terminal: redirect or pipe it to take the Parquet"
            );
            let pipeline = Pipeline::load(pipeline)?;
            polars_ensure!(
                pipeline.outputs.len() == 1,
                ComputeError: "stdout takes one output, the pipeline has {}", pipeline.outputs.len()
            );
            let mut frame = pipeline
                .run(&registry)?
                .remove(0);
            log::info!("writing {} rows to stdout", frame.height());
            ParquetWriter::new(std::io::stdout()).finish(&mut frame)?;
        }

        ["check", pipeline] => {
            let pipeline = Pipeline::load(pipeline)?;
            let schemas = pipeline.schemas(&registry)?;
            let checks: BTreeMap<_, _> = schemas
                .iter()
                .map(|(id, schema)| {
                    let columns = schema
                        .as_ref()
                        .map(|schema| {
                            schema
                                .iter()
                                .map(|(name, dtype)| (name.as_str(), dtype))
                                .collect::<Vec<_>>()
                        })
                        .map_err(|e| e.to_string());
                    (*id, columns)
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&checks).expect("columns serialize")
            );
        }

        ["catalog"] => println!("{:#}", registry.catalog()),

        _ => polars_bail!(ComputeError: "{USAGE}"),
    }

    Ok(())
}
