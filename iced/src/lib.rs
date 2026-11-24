//! Widgets of polaroid that know of iced alone: not a copy of iced, and nothing of pipelines.

mod measure;
mod menu;
mod scrub;
mod split;
mod surface;
mod table;

pub use measure::width_of;
pub use menu::{Menu, menu};
pub use scrub::{Scrub, scrub};
pub use split::{Split, split};
pub use surface::{Card, Surface, card, surface};
pub use table::{Heading, Table, table};
