use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Block;

/// Reads a Parquet file, or every file under a directory recursively.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanParquet {
    pub path: String,
    pub columns: Vec<(String, DataType)>,
}

impl Block for ScanParquet {
    const INPUTS: &'static [&'static str] = &[];

    fn apply(
        &self,
        _: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let schema: Schema = self
            .columns
            .iter()
            .map(|(name, dtype)| Field::new(name.into(), dtype.clone()))
            .collect();

        let options = ParquetOptions {
            schema: Some(Arc::new(schema)),
            ..Default::default()
        };

        let args = UnifiedScanArgs {
            missing_columns_policy: MissingColumnsPolicy::Insert,
            extra_columns_policy: ExtraColumnsPolicy::Ignore,
            glob: true,
            ..Default::default()
        };

        let sources = ScanSources::Paths(
            [PlRefPath::new(&self.path)]
                .into_iter()
                .collect(),
        );

        let scan: LazyFrame = DslBuilder::scan_parquet(sources, options, args)?
            .build()
            .into();

        Ok(scan.select(
            self.columns
                .iter()
                .map(|(name, _)| col(name.as_str()))
                .collect::<Vec<_>>(),
        ))
    }
}
