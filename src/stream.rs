//! Frames as bytes between the editor and its worker: an id and an Arrow file, each after its length.

use std::io::{Read, Write};

use crate::polars::prelude::*;

/// Writes the frame of node `id` to `out`, as Arrow lays it out in memory: nothing is compressed.
pub fn write(
    out: &mut impl Write,
    id: &str,
    frame: &mut DataFrame,
) -> PolarsResult<()> {
    let mut arrow = Vec::new();
    IpcWriter::new(&mut arrow).finish(frame)?;
    put(out, id.as_bytes())?;
    put(out, &arrow)
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
        let arrow = std::io::Cursor::new(part(&mut bytes)?);
        frames.push((id, IpcReader::new(arrow).finish()?));
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

/// Reads the next part from `input`, or none if it has ended; `Err` if it ends within one.
pub fn next(input: &mut impl Read) -> PolarsResult<Option<Vec<u8>>> {
    let mut len = [0; 8];
    // the end before a part is the end, and within one it is an error
    let mut got = 0;
    while got < len.len() {
        match input.read(&mut len[got..])? {
            0 if got == 0 => return Ok(None),
            0 => polars_bail!(ComputeError: "the parts are cut short"),
            more => got += more,
        }
    }
    let mut part = vec![0; u64::from_le_bytes(len) as usize];
    input.read_exact(&mut part)?;
    Ok(Some(part))
}

#[cfg(test)]
#[path = "../tests/unit/stream.rs"]
mod tests;
