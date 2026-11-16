//! Polaroid lets us define and run Polars Pipelines in JSON without recompiling.

// we communicate with child processes via stdout
#![deny(clippy::print_stdout)]

mod cli;
#[cfg(feature = "gui")]
mod gui;
mod stream;

pub use cli::cli;
pub use polaroid_core::*;
