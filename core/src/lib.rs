//! The engine of `polaroid`: blocks, the registry that names them, and
//! pipelines of them read from JSON and run on Polars. Use it through the
//! `polaroid` crate.

mod block;
mod engine;
pub mod frames;
pub mod io;
mod registry;

pub use block::{Block, take};
pub use engine::{Node, Pipeline};
pub use polars;
pub use registry::Registry;
