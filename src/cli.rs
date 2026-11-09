use std::collections::BTreeMap;
use std::fs::File;
use std::time::Instant;

use crate::polars::prelude::*;
use crate::{Pipeline, Registry};

const USAGE: &str = "usage:
  run <pipeline.json> <out-dir>   each output to <out-dir>/<id>.parquet
  check <pipeline.json>           every node's columns or error, as JSON
  catalog                         every block's args, as JSON Schema";

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

        ["check", pipeline] => {
            let pipeline = Pipeline::load(pipeline)?;
            let checks: BTreeMap<&str, Result<Vec<String>, String>> = pipeline
                .schemas(&registry)?
                .into_iter()
                .map(|(id, schema)| {
                    let columns = schema
                        .map(|schema| {
                            schema
                                .iter()
                                .map(|(name, dtype)| format!("{name}: {dtype}"))
                                .collect()
                        })
                        .map_err(|e| e.to_string());
                    (id, columns)
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&checks).expect("strings serialize")
            );
        }

        ["catalog"] => println!("{:#}", registry.catalog()),

        _ => polars_bail!(ComputeError: "{USAGE}"),
    }

    Ok(())
}
