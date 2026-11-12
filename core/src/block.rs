use polars::prelude::*;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// A Block is a frame-level operation, with its fields as its parameters.
pub trait Block<const N: usize = 1>: DeserializeOwned + JsonSchema {
    const INPUTS: [&'static str; N] = ["frame"; N];

    /// One or more frames in, one frame out.
    fn apply(
        self,
        inputs: [LazyFrame; N],
    ) -> PolarsResult<LazyFrame>;
}
