// we communicate with child processes via stdout
#![deny(clippy::print_stdout)]

mod block;
mod engine;
pub mod frames;
pub mod io;
pub mod methods;
#[cfg(feature = "plot")]
mod plot;
#[cfg(feature = "plot")]
pub mod plots;
mod registry;
pub mod schema;

pub use block::{Block, ColumnName};
pub use engine::{Node, Pipeline, is_sink, marked, marker};
#[cfg(feature = "plot")]
pub use plot::{Format, Options, Plot, numbers};
pub use polars;
pub use registry::{Catalog, Described, Registry};
