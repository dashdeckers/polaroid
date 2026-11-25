// we communicate with child processes via stdout
#![deny(clippy::print_stdout)]

mod block;
mod engine;
pub mod frames;
pub mod io;
pub mod methods;
mod names;
#[cfg(feature = "plot")]
mod plot;
#[cfg(feature = "plot")]
pub mod plots;
mod registry;
pub mod schema;

pub use block::{Block, ColumnName};
pub use engine::{Node, Pipeline, is_sink};
pub use names::{marked, marker};
#[cfg(feature = "plot")]
pub use plot::{Drawer, Format, Options, Plot, numbers};
pub use polars;
pub use registry::{Catalog, Described, Registry};
