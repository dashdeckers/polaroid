// we communicate with child processes via stdout
#![deny(clippy::print_stdout)]

mod block;
mod engine;
pub mod frames;
pub mod io;
pub mod methods;
mod registry;
pub mod schema;

pub use block::Block;
pub use engine::{Node, Pipeline, is_sink, marked, marker};
pub use polars;
pub use registry::Registry;
