//! What the tests build their simulators with: the faces of the editor, as the editor has them.

use iced_test::Simulator;

/// A simulator of `view`, set as the editor is.
pub fn simulator<'a, Message: 'a>(view: impl iced::Widget<Message> + 'a) -> Simulator<'a, Message> {
    Simulator::with_settings(polaroid_iced::theme::settings(), view)
}
