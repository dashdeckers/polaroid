//! Polaroid lets us define and run Polars Pipelines in JSON without recompiling.
//!
//! A Pipeline file is a graph of Nodes. Each Node names a [`Block`], sets its
//! args and lists the Nodes it reads from. A Block is a Rust struct that
//! turns its input frames into one output frame.
//!
//! To add Blocks, implement [`Block`], register them on a [`Registry`] and
//! pass it to [`cli`]. With the `gui` feature, [`cli`] also opens a visual
//! editor for Pipelines.
//!
//! Expressions in a Pipeline file use Polars' internal JSON format, so a
//! Pipeline only runs on the exact Polars version it names. Use the
//! re-exported [`polars`] so that your Blocks use that version too.

mod cli;

pub use cli::cli;
pub use polaroid_core::*;
