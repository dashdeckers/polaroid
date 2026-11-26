//! How a form looks: its sizes, its colours, and each kind of widget that it is made of.

use iced::widget::text::IntoFragment;
use iced::widget::text_input::TextInput;
use iced::widget::{Text, button, checkbox, container, row, text, text_input, tooltip};
use iced::{Center, Element, Font, Theme, Widget};
use polaroid_iced::{PickList, handle, pick_list, width_of};

use crate::gui::{Help, NOTE, TEXT};

/// The height of a line that holds a field.
const LINE: f32 = 26.0;

/// The size of text that reads as code.
const CODE: f32 = 16.0;

/// How far a call is set in from its root, and arguments from their call.
pub const INDENT: f32 = 20.0;

/// The width of the column before the lines of a chain, where a call is taken away.
pub const REMOVE: f32 = 22.0;

/// The room between the parts of a line, and between one that is set close to another.
pub const GAP: f32 = 5.0;
pub const CLOSE: f32 = 2.0;

/// What the handle beside a number says it is for.
const DRAG: &str = "Drag up for more and down for less; faster moves it by more.";

/// How a value stands to what its schema asks for, which tells how it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Given,
    /// It is the default, and shown in grey.
    Default,
    /// There is none where there has to be one, or it is not taken, and it is shown in red.
    Missing,
}

/// Whether text is that of an argument, or reads as code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Plain,
    Code,
}

impl Face {
    fn size(self) -> f32 {
        match self {
            Self::Plain => TEXT,
            Self::Code => CODE,
        }
    }

    fn font(self) -> Font {
        match self {
            Self::Plain => Font::DEFAULT,
            Self::Code => Font::MONOSPACE,
        }
    }

    /// The room around the text of a field.
    fn padding(self) -> [u16; 2] {
        match self {
            Self::Plain => [4, 4],
            Self::Code => [2, 6],
        }
    }
}

/// The style of a field to type in, by the tone of its value.
fn typed_in(tone: Tone) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme, status| {
        let palette = theme.palette();
        let mut style = text_input::default(theme, status);
        match tone {
            Tone::Given => {}
            Tone::Default => style.value = style.placeholder,
            Tone::Missing => {
                style.border.color = palette
                    .danger
                    .base
                    .color
            }
        }
        style
    }
}

/// The style of a list to pick from, by the tone of what is picked.
pub fn picked(tone: Tone) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |theme, status| {
        let palette = theme.palette();
        let mut style = pick_list::default(theme, status);
        match tone {
            Tone::Given => {}
            Tone::Default => style.text_color = style.placeholder_color,
            Tone::Missing => {
                style.border.color = palette
                    .danger
                    .base
                    .color
            }
        }
        style
    }
}

/// A field to type in, which shows `placeholder` while it is empty.
pub fn field<'a, Message: Clone + 'a>(
    placeholder: &'a str,
    value: impl IntoFragment<'a>,
    face: Face,
    tone: Tone,
) -> TextInput<'a, Message> {
    text_input(placeholder, value)
        .size(face.size())
        .font(face.font())
        .padding(face.padding())
        .style(typed_in(tone))
}

/// The width of a field that holds `typed`, with room for a few more.
pub fn width(
    typed: &str,
    face: Face,
) -> f32 {
    (width_of(typed, face.size(), face.font()) + 24.0).max(48.0)
}

/// A field as wide as what is `typed` in it.
pub fn fitted<'a, Message: Clone + 'a>(
    placeholder: &'a str,
    typed: String,
    face: Face,
    tone: Tone,
) -> TextInput<'a, Message> {
    let width = width(&typed, face);
    field(placeholder, typed, face, tone).width(width)
}

/// A list to pick one of `options` from, which shows `selected`, or else `placeholder`.
pub fn pick<'a, T: Clone + PartialEq + 'a, Message>(
    selected: Option<T>,
    options: Vec<T>,
    label: impl Fn(&T) -> String + 'a,
    placeholder: &str,
    face: Face,
    tone: Tone,
) -> PickList<'a, T, Message> {
    pick_list(selected, options, label)
        .placeholder(placeholder)
        .text_size(face.size())
        .font(face.font())
        .padding(face.padding())
        .style(picked(tone))
}

/// A box to tick, by the tone of whether it is ticked.
pub fn tick<'a, Message: 'a>(
    is_ticked: bool,
    tone: Tone,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    let style = move |theme: &Theme, status| {
        let palette = theme.palette();
        let mut style = checkbox::primary(theme, status);
        let grey = palette
            .background
            .strong
            .color;
        match tone {
            Tone::Given => {}
            Tone::Default => {
                style.border.color = grey;
                if is_ticked {
                    style.background = grey.into();
                }
            }
            Tone::Missing => {
                style.border.color = palette
                    .danger
                    .base
                    .color
            }
        }
        style
    };
    checkbox(is_ticked)
        .on_toggle(on_toggle)
        .style(style)
        .boxed()
}

/// `field` beside a handle that moves its number as a slider would, if `on_scrub` moves one.
pub fn scrubbed<'a, Message: Clone + 'a>(
    field: impl Widget<Message> + 'a,
    on_scrub: Option<impl Fn(i32) -> Message + 'a>,
    help: Help,
) -> Element<'a, Message> {
    let handle = on_scrub.map(|on_scrub| help.of(handle().on_scrub(on_scrub), DRAG));
    row![field, handle]
        .spacing(CLOSE)
        .align_y(Center)
        .boxed()
}

/// A small button that is a sign, as `+` or `×`.
pub fn small<'a, Message: Clone + 'a>(
    label: &'a str,
    message: Message,
) -> Element<'a, Message> {
    button(label)
        .padding([5, 8])
        .style(button::text)
        .on_press(message)
        .boxed()
}

/// The button before a line of a chain that takes its call away.
pub fn remove<'a, Message: Clone + 'a>(message: Message) -> Element<'a, Message> {
    button(code("×"))
        .padding([2, 6])
        .style(button::text)
        .on_press(message)
        .boxed()
}

/// Text that reads as code.
pub fn code<'a>(written: impl Into<String>) -> Text<'a> {
    text(written.into())
        .size(CODE)
        .font(Font::MONOSPACE)
}

/// What is said beside a field: in grey what it stands for, on one line that is cut short.
pub fn aside<'a>(said: impl Into<String>) -> Text<'a> {
    text(said.into())
        .size(NOTE)
        .wrapping(text::Wrapping::None)
        .ellipsis(text::Ellipsis::End)
        .style(text::secondary)
}

/// Why a value is not taken, in red.
pub fn note<'a>(why: impl Into<String>) -> Text<'a> {
    text(why.into())
        .size(NOTE)
        .style(text::danger)
}

/// The width of a column that holds the widest of the names of some arguments.
pub fn names<'a>(names: impl IntoIterator<Item = &'a str>) -> f32 {
    names
        .into_iter()
        .map(|name| width_of(name, TEXT, Font::DEFAULT))
        .fold(0.0, f32::max)
}

/// A description as its paragraphs, each on one line for the text to wrap as it fits.
pub(super) fn flowed(described: &str) -> String {
    described
        .split("\n\n")
        .map(|paragraph| paragraph.replace('\n', " "))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `content`, which tells what Polars `says` of it when hovered, if it says anything.
pub fn told<'a, Message: 'a>(
    content: impl Widget<Message> + 'a,
    says: Option<String>,
) -> Element<'a, Message> {
    told_at(content, says, tooltip::Position::Top)
}

/// `content`, which tells what is said of it when hovered, on this side of it.
pub fn told_at<'a, Message: 'a>(
    content: impl Widget<Message> + 'a,
    says: Option<String>,
    side: tooltip::Position,
) -> Element<'a, Message> {
    let Some(says) = says else {
        return content.boxed();
    };
    let says = container(text(says).size(NOTE))
        .width(280)
        .padding(6)
        .style(container::bordered_box);
    tooltip(content, says)
        .position(side)
        .boxed()
}

/// An argument on a line: its name in a column of the width `names`, then its value.
pub fn labelled<'a, Message: 'a>(
    name: &'a str,
    names: f32,
    described: Option<&'a str>,
    value: impl Widget<Message> + 'a,
) -> Element<'a, Message> {
    let name = text(name)
        .size(TEXT)
        .wrapping(text::Wrapping::None)
        .ellipsis(text::Ellipsis::End);
    // level with the first line of a value of many, as a list is
    let name = container(name)
        .width(names)
        .height(LINE)
        .align_y(Center);
    // its name tells what it is for when hovered
    let name = told(name, described.map(flowed));
    row![name, value]
        .spacing(8)
        .boxed()
}

/// `content` in a line of the height of a field, as the cells of a table are.
pub fn lined<'a, Message: 'a>(content: impl Widget<Message> + 'a) -> Element<'a, Message> {
    container(content)
        .height(LINE)
        .align_y(Center)
        .boxed()
}
