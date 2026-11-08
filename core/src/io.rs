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

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs::{File, create_dir_all};
    use std::path::Path;

    use super::*;

    fn write(
        path: &Path,
        mut frame: DataFrame,
    ) -> PolarsResult<()> {
        create_dir_all(
            path.parent()
                .unwrap(),
        )?;
        ParquetWriter::new(File::create(path)?).finish(&mut frame)?;
        Ok(())
    }

    fn scan(path: &Path) -> PolarsResult<DataFrame> {
        ScanParquet {
            path: path
                .to_string_lossy()
                .into_owned(),
            columns: vec![
                ("a".into(), DataType::Int32),
                ("b".into(), DataType::Int32),
            ],
        }
        .apply(vec![])?
        .collect()
    }

    #[test]
    fn reads_a_single_file() -> PolarsResult<()> {
        let path = temp_dir().join("polaroid-io-file.parquet");
        write(&path, df!("a" => [1], "b" => [2])?)?;

        assert_eq!(scan(&path)?.height(), 1);
        Ok(())
    }

    #[test]
    fn reads_a_directory_recursively() -> PolarsResult<()> {
        let dir = temp_dir().join("polaroid-io-dir");
        write(&dir.join("one.parquet"), df!("a" => [1], "b" => [2])?)?;
        write(
            &dir.join("nested/two.parquet"),
            df!("a" => [3], "extra" => [4])?,
        )?;

        let frame = scan(&dir)?;
        assert_eq!(frame.height(), 2);
        assert_eq!(frame.width(), 2);
        assert_eq!(
            frame
                .column("b")?
                .null_count(),
            1
        );
        Ok(())
    }
}
