//! Widgets of polaroid that know of iced alone: not a copy of iced, and nothing of pipelines.

mod leave;
mod measure;
mod menu;
pub mod pick_list;
mod scrub;
mod seen;
mod split;
mod surface;
mod table;
pub mod theme;

pub use leave::{Leave, leave};
pub use measure::width_of;
pub use menu::{Menu, menu};
pub use pick_list::{PickList, pick_list};
pub use scrub::{HANDLE, Scrub, handle, scrub};
pub use split::{Split, split};
pub use surface::{Card, Surface, card, surface};
pub use table::{Asked, Heading, Table, table};
