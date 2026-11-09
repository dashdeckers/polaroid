//! Polaroid lets us define and run Polars Pipelines in JSON without recompiling.
//!
//! A Pipeline file is a graph of Nodes. Each Node names a [`Block`], sets its
//! args and lists the Nodes it reads from. A Block is a Rust struct that
//! turns its input frames into one output frame.
//!
//! To add Blocks, implement [`Block`], register them on a [`Registry`] and
//! pass it to [`cli`].
//!
//! Expressions in a Pipeline file use Polars' internal JSON format, so a
//! Pipeline only runs on the exact Polars version it names. Use the
//! re-exported [`polars`] so that your Blocks use that version too.
//!
//! Polaroid logs through the [`log`] crate. [`cli`] writes those lines to a
//! fresh `polaroid.log` in the working directory, created on startup.

mod cli;

pub use cli::cli;
pub use polaroid_core::*;
