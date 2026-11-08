//! Sources: the only blocks that read files, and they read no frame.

use calamine::{Data, Reader, open_workbook_auto};
use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Block;

/// Every `.parquet` under `dir`, recursively, read as `columns`: a column a
/// file lacks reads as nulls, and one not listed is ignored.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanParquetDir {
    pub dir: String,
    /// Each column's name and type.
    pub columns: Vec<(String, DataType)>,
}

impl Block for ScanParquetDir {
    const INPUTS: &'static [&'static str] = &[];

    fn apply_all(
        &self,
        _: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let schema: Schema = self
            .columns
            .iter()
            .map(|(name, dtype)| Field::new(name.into(), dtype.clone()))
            .collect();
        let args = UnifiedScanArgs {
            schema: Some(Arc::new(schema)),
            missing_columns_policy: MissingColumnsPolicy::Insert,
            extra_columns_policy: ExtraColumnsPolicy::Ignore,
            glob: true,
            ..Default::default()
        };
        let glob = format!(
            "{}/**/*.parquet",
            self.dir
                .trim_end_matches(['/', '\\'])
        );
        let sources = ScanSources::Paths(
            [PlRefPath::new(glob)]
                .into_iter()
                .collect(),
        );
        let scan: LazyFrame = DslBuilder::scan_parquet(sources, Default::default(), args)?
            .build()
            .into();
        // selected by name too: a slice straight on the scan, such as a
        // preview's, otherwise trips over a column it was told to ignore
        Ok(scan.select(
            self.columns
                .iter()
                .map(|(name, _)| col(name.as_str()))
                .collect::<Vec<_>>(),
        ))
    }
}

/// A cell as text, as a sheet is read throughout: every column a string.
fn text(cell: &Data) -> Option<String> {
    match cell {
        Data::Empty => None,
        Data::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// Columns of a spreadsheet's sheet, every one read as text. The first row is
/// the header.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadSheet {
    pub path: String,
    pub sheet: String,
    /// Each column as `(header, name)`: found by its header, renamed to name.
    pub columns: Vec<(String, String)>,
}

impl Block for ReadSheet {
    const INPUTS: &'static [&'static str] = &[];

    fn apply_all(
        &self,
        _: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let (path, sheet) = (&self.path, &self.sheet);
        let failed = |e: calamine::Error| polars_err!(ComputeError: "reading {path}: {e}");
        let range = open_workbook_auto(path)
            .map_err(failed)?
            .worksheet_range(sheet)
            .map_err(failed)?;

        let mut rows = range.rows();
        let header: Vec<Option<String>> = rows
            .next()
            .ok_or_else(|| polars_err!(ComputeError: "{path}: sheet {sheet} is empty"))?
            .iter()
            .map(text)
            .collect();

        let columns = self
            .columns
            .iter()
            .map(|(heading, name)| {
                let index = header
                    .iter()
                    .position(|h| h.as_ref() == Some(heading))
                    .ok_or_else(
                        || polars_err!(ColumnNotFound: "{path}: sheet {sheet} has no column {heading}"),
                    )?;
                Ok((name, index))
            })
            .collect::<PolarsResult<Vec<_>>>()?;

        let body: Vec<&[Data]> = rows.collect();
        let series = columns
            .iter()
            .map(|&(name, index)| {
                let values: Vec<Option<String>> = body
                    .iter()
                    .map(|row| {
                        row.get(index)
                            .and_then(text)
                    })
                    .collect();
                Column::new(name.into(), values)
            })
            .collect();

        Ok(DataFrame::new_infer_height(series)?.lazy())
    }
}

/// A CSV file's `columns`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanCsv {
    pub path: String,
    pub columns: Vec<String>,
}

impl Block for ScanCsv {
    const INPUTS: &'static [&'static str] = &[];

    fn apply_all(
        &self,
        _: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        Ok(LazyCsvReader::new(PlRefPath::new(&self.path))
            .finish()?
            .select(
                self.columns
                    .iter()
                    .map(|name| col(name.as_str()))
                    .collect::<Vec<_>>(),
            ))
    }
}
