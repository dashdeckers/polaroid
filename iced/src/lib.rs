//! Widgets of polaroid that know of iced alone: not a copy of iced, and nothing of pipelines.

pub mod split;
pub mod surface;

pub use split::{Split, split};
pub use surface::{Card, Surface, card, surface};
