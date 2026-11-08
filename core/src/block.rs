//! What a building block is: a struct whose fields are its parameters, which
//! turns the frames it reads into one frame. The same struct is what a
//! pipeline file names and fills in, and what a GUI renders a form for.

use polars::prelude::*;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// Derive `Deserialize` and `JsonSchema` for it, and its doc comments become
/// the catalog's; `#[serde(deny_unknown_fields)]` catches a misspelled arg.
pub trait Block: DeserializeOwned + JsonSchema {
    /// The frames the block reads, by role, in order. Most read one; a
    /// source reads none, a join two.
    const INPUTS: &'static [&'static str] = &["frame"];

    /// One frame in, one frame out: what most blocks are.
    fn apply(
        &self,
        _lf: LazyFrame,
    ) -> PolarsResult<LazyFrame> {
        polars_bail!(
            ComputeError: "this block reads {:?}: call `apply_all`", Self::INPUTS
        )
    }

    /// Every frame of `INPUTS` in, one frame out.
    fn apply_all(
        &self,
        inputs: Vec<LazyFrame>,
    ) -> PolarsResult<LazyFrame> {
        let [lf] = take(inputs)?;
        self.apply(lf)
    }
}

/// The `N` frames a block reads, or an error saying how many it got.
pub fn take<const N: usize>(inputs: Vec<LazyFrame>) -> PolarsResult<[LazyFrame; N]> {
    let got = inputs.len();
    inputs
        .try_into()
        .map_err(|_| polars_err!(ComputeError: "expected {N} input frames, got {got}"))
}
