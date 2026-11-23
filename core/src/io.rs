use polars::io::path_utils::expand_paths;
use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Block;

/// Reads a Parquet file, or several as one frame.
///
/// Only the columns and rows that the pipeline needs are read.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanParquet {
    /// Path to a file, to a directory, of which every file under it is read, or a glob pattern
    /// such as data/*.parquet. A column that one of the files lacks is null in its rows.
    pub path: String,
}

impl Block<0> for ScanParquet {
    /// Lists the files, but reads none.
    fn apply(
        self,
        []: [LazyFrame; 0],
    ) -> PolarsResult<LazyFrame> {
        let path = [PlRefPath::new(&self.path)];
        let (glob, hidden_prefixes, mut cloud) = (true, [], None);
        let files = expand_paths(&path, glob, &hidden_prefixes, &mut cloud);
        let files = polars_async::ASYNC.block_on(files)?;
        polars_ensure!(!files.is_empty(), ComputeError: "no Parquet files under '{}'", self.path);

        let scans = files
            .iter()
            .map(|file| {
                // a listed file is that file, whatever its name looks like
                let args = ScanArgsParquet {
                    glob: false,
                    ..Default::default()
                };
                LazyFrame::scan_parquet(file.clone(), args)
            })
            .collect::<PolarsResult<Vec<_>>>()?;

        concat(
            scans,
            UnionArgs {
                diagonal: true,
                to_supertypes: true,
                ..Default::default()
            },
        )
    }
}

/// Writes the frame to a Parquet file on every Run, replacing a file that is there.
///
/// The nodes that read this one get the frame it writes.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteParquet {
    /// Path of the Parquet file to write.
    pub path: String,
}

impl Block for WriteParquet {
    /// Plans the write, which a run of the Pipeline then does.
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        lf.sink(
            SinkDestination::File {
                target: SinkTarget::Path(PlRefPath::new(&self.path)),
            },
            FileWriteFormat::Parquet(Default::default()),
            UnifiedSinkArgs::default(),
        )
    }
}
