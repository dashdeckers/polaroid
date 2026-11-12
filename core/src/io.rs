use polars::io::path_utils::expand_paths;
use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{Block, take};

/// Reads a Parquet file, every file under a directory recursively, or a glob.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanParquet {
    pub path: String,
}

impl Block for ScanParquet {
    const INPUTS: &'static [&'static str] = &[];

    /// Lists the files, but reads none.
    fn apply(
        self,
        _: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let path = [PlRefPath::new(&self.path)];
        let (glob, hidden_prefixes, mut cloud) = (true, [], None);
        let files = expand_paths(&path, glob, &hidden_prefixes, &mut cloud);
        let files = polars_async::ASYNC.block_on(files)?;
        polars_ensure!(!files.is_empty(), ComputeError: "no Parquet files under '{}'", self.path);

        let scans = files
            .iter()
            .map(|file| LazyFrame::scan_parquet(file.clone(), Default::default()))
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

/// Writes the frame to a Parquet file, replacing any there, and gives it on.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteParquet {
    pub path: String,
}

impl Block for WriteParquet {
    /// Plans the write, which a run of the Pipeline then does.
    fn apply(
        self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        lf.sink(
            SinkDestination::File {
                target: SinkTarget::Path(PlRefPath::new(&self.path)),
            },
            FileWriteFormat::Parquet(Default::default()),
            UnifiedSinkArgs::default(),
        )
    }
}
