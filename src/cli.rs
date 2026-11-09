use std::collections::BTreeMap;
use std::fs::File;

use crate::polars::prelude::*;
use crate::{Pipeline, Registry};

const USAGE: &str = "usage:
  run <pipeline.json> <out-dir>   each output to <out-dir>/<id>.parquet
  check <pipeline.json>           every node's columns or error, as JSON
  catalog                         every block's args, as JSON Schema";

pub fn cli(registry: Registry) -> PolarsResult<()> {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .collect();

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
