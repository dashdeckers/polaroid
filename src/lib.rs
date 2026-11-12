//! Polaroid lets us define and run Polars Pipelines in JSON without recompiling.

// we communicate with child processes via stdout
#![deny(clippy::print_stdout)]

mod cli;

pub use cli::cli;
pub use polaroid_core::*;
