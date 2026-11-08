use polars::prelude::*;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// A Block is a frame-level operation, with its fields as its parameters.
pub trait Block: DeserializeOwned + JsonSchema {
    const INPUTS: &'static [&'static str] = &["frame"];

    /// One or more frames in, one frame out.
    fn apply(
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame>;
}

pub fn take<const N: usize>(inputs: Vec<LazyFrame>) -> PolarsResult<[LazyFrame; N]> {
    let got = inputs.len();
    inputs
        .try_into()
        .map_err(|_| polars_err!(ComputeError: "expected {N} input frames, got {got}"))
}
