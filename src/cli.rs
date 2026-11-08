use std::fs::File;

use crate::polars::prelude::*;
use crate::{Pipeline, Registry};

const USAGE: &str = "usage:
  run <pipeline.json> <out-dir>   each output to <out-dir>/<id>.parquet
  check <pipeline.json>           every node's columns, or its error
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
            let schemas = pipeline.schemas(&registry)?;
            for node in &pipeline.nodes {
                match &schemas[node.id.as_str()] {
                    Ok(schema) => println!(
                        "{}: {:?}",
                        node.id,
                        schema
                            .iter_names()
                            .collect::<Vec<_>>()
                    ),
                    Err(e) => println!("{}: {e}", node.id),
                }
            }
        }
        ["catalog"] => println!("{:#}", registry.catalog()),
        _ => polars_bail!(ComputeError: "{USAGE}"),
    }
    Ok(())
}
