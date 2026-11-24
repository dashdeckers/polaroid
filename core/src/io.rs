use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use file_format::FileFormat;
use polars::io::path_utils::expand_paths;
use polars::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::block::checked;
use crate::{Block, ColumnName};

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

fn comma() -> char {
    ','
}

fn yes() -> bool {
    true
}

/// The byte of a character that separates the values of a line of CSV.
fn byte(separator: char) -> PolarsResult<u8> {
    polars_ensure!(
        separator.is_ascii(),
        ComputeError: "the separator {separator} is no single byte"
    );
    Ok(separator as u8)
}

/// Reads a CSV file, or several of the same columns as one frame.
///
/// The data types of the columns are told from the first rows.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanCsv {
    /// Path to a file, or a glob pattern such as data/*.csv.
    pub path: String,
    /// Character that separates the values of a line.
    #[serde(default = "comma")]
    pub separator: char,
    /// Whether the first line names the columns. Without it they are named column_1, column_2
    /// and so on.
    #[serde(default = "yes")]
    pub has_header: bool,
    /// Read values that look like dates and times as such. Otherwise they are strings.
    #[serde(default)]
    pub try_parse_dates: bool,
}

impl Block<0> for ScanCsv {
    fn apply(
        self,
        []: [LazyFrame; 0],
    ) -> PolarsResult<LazyFrame> {
        LazyCsvReader::new(PlRefPath::new(&self.path))
            .with_separator(byte(self.separator)?)
            .with_has_header(self.has_header)
            .with_try_parse_dates(self.try_parse_dates)
            .finish()
    }
}

/// Writes the frame to a CSV file on every Run, replacing a file that is there.
///
/// The nodes that read this one get the frame it writes.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteCsv {
    /// Path of the CSV file to write.
    pub path: String,
    /// Character that separates the values of a line.
    #[serde(default = "comma")]
    pub separator: char,
    /// Whether the first line names the columns.
    #[serde(default = "yes")]
    pub include_header: bool,
}

impl Block for WriteCsv {
    /// Plans the write, which a run of the Pipeline then does.
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let serialize = SerializeOptions {
            separator: byte(self.separator)?,
            ..Default::default()
        };
        let options = CsvWriterOptions {
            include_header: self.include_header,
            serialize_options: Arc::new(serialize),
            ..Default::default()
        };
        lf.sink(
            SinkDestination::File {
                target: SinkTarget::Path(PlRefPath::new(&self.path)),
            },
            FileWriteFormat::Csv(options),
            UnifiedSinkArgs::default(),
        )
    }
}

/// Writes each value of a column to a file of its own on every Run, replacing a file that is there.
///
/// The file of a row is folder/directories/name.extension, and folders that are missing are made.
/// Each directory, name and extension is to name one file or folder on every platform, and no
/// two rows the same file. The nodes that read this one get the frame it writes.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteFiles {
    /// Column of what to write, of binary values such as the images of a plot. A row where it is
    /// null writes no file.
    pub column: ColumnName,
    /// Folder under which every file is written.
    pub folder: String,
    /// Column of the folders of each file under folder: of strings, or of lists of strings with
    /// the outermost first. With none, and in a row where it is null, the file is in folder.
    #[serde(default)]
    pub directories: Option<ColumnName>,
    /// Column of the names of the files without their extension, of strings.
    pub name: ColumnName,
    /// Column of the extensions of the files, such as svg; where it is none, the bytes tell it.
    #[serde(default)]
    pub extension: Option<ColumnName>,
}

impl WriteFiles {
    /// Finds a column that is not in `schema`, or that does not hold what is read of it.
    fn check(
        &self,
        schema: &Schema,
    ) -> PolarsResult<()> {
        let strings = DataType::List(Box::new(DataType::String));
        let holds = |column: &ColumnName, types: &[&DataType]| {
            let dtype = schema.try_get(&column.0)?;
            polars_ensure!(
                types.contains(&dtype),
                SchemaMismatch: "column '{}' holds {dtype}, which write_files does not take there",
                column.0
            );
            Ok(())
        };
        holds(&self.column, &[&DataType::Binary])?;
        holds(&self.name, &[&DataType::String])?;
        if let Some(extension) = &self.extension {
            holds(extension, &[&DataType::String])?;
        }
        match &self.directories {
            Some(directories) => holds(directories, &[&DataType::String, &strings]),
            None => Ok(()),
        }
    }
}

impl Block for WriteFiles {
    /// Plans the writes, which a run of the Pipeline then does.
    fn apply(
        self,
        [lf]: [LazyFrame; 1],
    ) -> PolarsResult<LazyFrame> {
        let (this, strings) = (Arc::new(self), DataType::List(Box::new(DataType::String)));
        let block = this.clone();
        let lf = checked(lf, move |schema| block.check(schema));

        let write = move |frame: DataFrame| {
            let text = |column: &ColumnName| {
                frame
                    .column(&column.0)?
                    .str()
            };
            let values = frame
                .column(&this.column.0)?
                .binary()?;
            let names = text(&this.name)?;
            let extensions = this
                .extension
                .as_ref()
                .map(text)
                .transpose()?;
            // a single folder is a list of one
            let folders = this
                .directories
                .as_ref()
                .map(|directories| {
                    frame
                        .column(&directories.0)?
                        .cast(&strings)
                })
                .transpose()?;
            // the files written, to find two rows that write the same one
            let mut written = HashSet::new();
            for row in 0..frame.height() {
                let Some(value) = values.get(row) else {
                    continue;
                };
                let Some(name) = names.get(row) else {
                    polars_bail!(ComputeError: "row {row}: the file has no name");
                };
                let extension = match extensions.and_then(|extensions| extensions.get(row)) {
                    Some(extension) => extension
                        .strip_prefix('.')
                        .unwrap_or(extension)
                        .to_owned(),
                    None => extension_of(value, row)?,
                };
                let mut path = PathBuf::from(&this.folder);
                let folders = match &folders {
                    Some(folders) => folders
                        .list()?
                        .get_as_series(row),
                    None => None,
                };
                if let Some(folders) = folders {
                    for folder in folders
                        .str()?
                        .iter()
                    {
                        path.push(part(folder.unwrap_or_default())?);
                    }
                }
                let file = format!("{}.{}", part(name)?, part(&extension)?);
                let told = |e| polars_err!(ComputeError: "{}: {e}", path.display());
                std::fs::create_dir_all(&path).map_err(told)?;
                path.push(file);
                // a file system may hold A and a to be the same file
                let key = path
                    .to_string_lossy()
                    .to_lowercase();
                let is_new = written.insert(key);
                polars_ensure!(is_new, ComputeError: "two rows write '{}'", path.display());
                let told = |e| polars_err!(ComputeError: "{}: {e}", path.display());
                std::fs::write(&path, value).map_err(told)?;
            }
            Ok(false)
        };
        // every row in one batch, so that a row is numbered as in the frame
        lf.sink_batches(PlanCallback::new(write), true, Some(NonZeroUsize::MAX))
    }
}

/// The extension of a file of `bytes`, as file-format tells it; an error of `row` if it cannot.
fn extension_of(
    bytes: &[u8],
    row: usize,
) -> PolarsResult<String> {
    let extension = FileFormat::from_bytes(bytes)
        .extension()
        .to_owned();
    // what it answers for bytes or text it does not know
    let is_told = !matches!(extension.as_str(), "bin" | "txt" | "empty");
    polars_ensure!(
        is_told,
        ComputeError: "row {row}: its bytes do not tell the type of the file: give an extension"
    );
    Ok(extension)
}

/// `text`, if it names one file or folder on every platform: Windows takes the least.
fn part(text: &str) -> PolarsResult<&str> {
    let device = text
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let is_numbered = |port: &str| {
        device
            .strip_prefix(port)
            .is_some_and(|number| number.len() == 1 && number.as_bytes()[0].is_ascii_digit())
    };
    let is_device = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || is_numbered("COM")
        || is_numbered("LPT");
    let is_plain = !text.is_empty()
        && !text.contains(|c: char| c.is_control() || r#"<>:"/\|?*"#.contains(c))
        // which rules out . and .. too
        && !text.ends_with(['.', ' ']);
    polars_ensure!(
        is_plain && !is_device,
        ComputeError: "'{text}' does not name one file or folder on every platform"
    );
    Ok(text)
}
