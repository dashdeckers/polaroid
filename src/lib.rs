//! Pipelines of building blocks, written as JSON and run on Polars.
//!
//! A block is a struct whose fields are its parameters and which turns the
//! frames it reads into one frame: an `impl` [`Block`]. A domain crate writes
//! its blocks, registers them by name in a [`Registry`] beside the Polars
//! ones, and hands that to [`cli`]:
//!
//! ```ignore
//! fn main() -> polaroid::polars::prelude::PolarsResult<()> {
//!     let registry = polaroid::Registry::with_polars()
//!         .register::<my_blocks::Smooth>("smooth")
//!         .register::<my_blocks::Score>("score");
//!     polaroid::cli(registry)
//! }
//! ```
//!
//! A [`Pipeline`] file then names those blocks and fills in their fields, and
//! changing it needs no recompile. Polars is pinned to one exact version,
//! because a pipeline's expressions are in its internal JSON form; use it as
//! [`polars`] so a domain crate shares it.

//! Polaroid lets us define and run Polars pipelines in JSON without recompiling.
//!
//! We do this by 

mod cli;

pub use cli::cli;
pub use polaroid_core::*;
