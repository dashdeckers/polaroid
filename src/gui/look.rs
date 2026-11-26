//! How the editor looks: its sizes, its colours, and each kind of widget that it is made of.

use iced::widget::text::IntoFragment;
use iced::widget::text_input::TextInput;
use iced::widget::{
    Button,
    Text,
    button,
    checkbox,
    container,
    row,
    scrollable,
    text,
    text_input,
    tooltip,
};
use iced::{Center, Element, Fill, Font, Shrink, Theme, Widget, padding};
use polaroid_iced::{PickList, handle, pick_list, width_of};

/// Whether what the editor is made of says what it is for when hovered.
#[derive(Debug, Clone, Copy, Default)]
pub struct Help(pub bool);

impl Help {
    /// `content`, which `says` what it is for when hovered while help is on.
    pub fn of<'a, Message: 'a>(
        self,
        content: impl Widget<Message> + 'a,
        says: &str,
    ) -> Element<'a, Message> {
        let says = self
            .0
            .then(|| says.to_owned());
        // under it, where what Polars says of it, over it, is not hidden
        told_at(content, says, tooltip::Position::Bottom)
    }
}

/// The sizes of text: of a name to type, of an argument, and of what is said beside them.
pub const TITLE: f32 = 14.0;
pub const TEXT: f32 = 13.0;
pub const NOTE: f32 = 12.0;

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

/// The button that lays something open, or closes it if it is open.
pub fn toggle<'a, Message: Clone + 'a>(
    is_open: bool,
    message: Message,
) -> Button<'a, Message, &'static str> {
    let sign = match is_open {
        true => "−",
        false => "+",
    };
    button(sign)
        .style(button::text)
        .padding([0, 6])
        .on_press(message)
}

/// The button that deletes something.
pub fn cross<'a, Message: Clone + 'a>(message: Message) -> Button<'a, Message, &'static str> {
    button("×")
        .style(button::danger)
        .padding([0, 6])
        .on_press(message)
}

/// How thick the bars are that an error is scrolled by.
const BAR: f32 = 6.0;

/// The most that an error takes of the height of what shows it.
const ERROR: f32 = 120.0;

/// An error, which can be selected to copy it; one that is long or wide is scrolled through.
pub fn error<'a, Message: 'a>(message: &'a str) -> Element<'a, Message> {
    let written = text(message)
        .style(text::danger)
        .wrapping(text::Wrapping::None)
        .selectable(true);
    let bar = || {
        scrollable::Scrollbar::new()
            .width(BAR)
            .scroller_width(BAR)
            .margin(0)
    };
    let direction = scrollable::Direction::Both {
        vertical: bar(),
        horizontal: bar(),
    };
    // the bars lie over the edges of what is seen, so the text ends clear of them
    let clear = padding::right(BAR + 2.0).bottom(BAR + 2.0);
    scrollable(container(written).padding(clear))
        .direction(direction)
        .width(Fill)
        .height(Shrink.max(ERROR))
        .style(|theme: &Theme, status| {
            // and no text shows through them: it ends at a bar as at an edge
            let paper = theme
                .palette()
                .background
                .base
                .color;
            let mut style = scrollable::default(theme, status);
            style
                .vertical_rail
                .background = Some(paper.into());
            style
                .horizontal_rail
                .background = Some(paper.into());
            style.gap = Some(paper.into());
            style
        })
        .boxed()
}

/// A field for a name, given on Enter: what is `typed` until then, orange, or red if it is refused.
pub fn name_field<'a, Message: Clone + 'a>(
    placeholder: &'static str,
    typed: Option<&str>,
    name: &str,
    is_taken: impl Fn(&str) -> bool,
) -> text_input::TextInput<'a, Message> {
    let is_typed = is_unsaved(typed, name);
    let is_refused = is_refused(typed, name, is_taken);
    let shown = typed
        .unwrap_or(name)
        .to_owned();
    text_input(placeholder, shown).style(move |theme: &Theme, status| {
        let mut style = text_input::default(theme, status);
        if is_typed {
            let palette = theme.palette();
            let (refused, unsaved) = (palette.danger.base, palette.warning.base);
            style.border.color = match is_refused {
                true => refused.color,
                false => unsaved.color,
            };
            style.border.width = 2.0;
        }
        style
    })
}

/// Whether what is typed for a name is another than the name, which Enter is still to give.
pub fn is_unsaved(
    typed: Option<&str>,
    name: &str,
) -> bool {
    typed.is_some_and(|typed| typed != name)
}

/// Whether what is typed for a name is one that Enter does not give: empty, or `is_taken`.
pub fn is_refused(
    typed: Option<&str>,
    name: &str,
    is_taken: impl Fn(&str) -> bool,
) -> bool {
    is_unsaved(typed, name) && typed.is_some_and(|typed| typed.is_empty() || is_taken(typed))
}
