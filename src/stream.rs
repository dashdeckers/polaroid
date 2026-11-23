//! Frames as bytes between the editor and `collect`: an id and a Parquet, each after its length.

use std::io::Write;

use crate::polars::prelude::*;

/// Writes the frame of node `id` to `out`.
pub fn write(
    out: &mut impl Write,
    id: &str,
    frame: &mut DataFrame,
) -> PolarsResult<()> {
    let mut parquet = Vec::new();
    ParquetWriter::new(&mut parquet).finish(frame)?;
    put(out, id.as_bytes())?;
    put(out, &parquet)
}

/// Writes a part to `out`.
pub fn put(
    out: &mut impl Write,
    part: &[u8],
) -> PolarsResult<()> {
    out.write_all(&(part.len() as u64).to_le_bytes())?;
    out.write_all(part)?;
    Ok(())
}

/// The frames that were written to `bytes`, by node id.
pub fn read(mut bytes: &[u8]) -> PolarsResult<Vec<(String, DataFrame)>> {
    let mut frames = Vec::new();
    while !bytes.is_empty() {
        let id = String::from_utf8_lossy(part(&mut bytes)?).into_owned();
        let parquet = std::io::Cursor::new(part(&mut bytes)?);
        frames.push((id, ParquetReader::new(parquet).finish()?));
    }
    Ok(frames)
}

/// Takes a part off the front of `bytes`; `Err` if they end within it.
pub fn part<'a>(bytes: &mut &'a [u8]) -> PolarsResult<&'a [u8]> {
    let cut = || polars_err!(ComputeError: "the frames are cut short");
    let (len, rest) = bytes
        .split_first_chunk::<8>()
        .ok_or_else(cut)?;
    let (part, rest) = rest
        .split_at_checked(u64::from_le_bytes(*len) as usize)
        .ok_or_else(cut)?;
    *bytes = rest;
    Ok(part)
}

#[cfg(test)]
#[path = "../tests/unit/stream.rs"]
mod tests;
