//! What the editor is built of: each kind of widget, set and drawn as the theme says.

use iced::widget::text::IntoFragment;
use iced::widget::text_input::TextInput;
use iced::widget::{
    Button,
    Component,
    Text,
    button,
    center,
    checkbox,
    component,
    container,
    mouse_area,
    popover,
    row,
    scrollable,
    space,
    text,
    text_input,
    tooltip,
};
use iced::{Center, Color, Element, Fill, Font, Renderer, Shrink, Widget, padding};
use polaroid_iced::theme::{self, Tone, control, extent, font, sign, size, spacing};
use polaroid_iced::{PickList, handle, leave, pick_list, width_of};

/// What the handle beside a number says it is for.
const DRAG: &str = "Drag up for more and down for less; faster moves it by more.";

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

/// A field to type in, which shows `placeholder` while it is empty.
pub fn field<'a, Message: Clone + 'a>(
    placeholder: &'a str,
    value: impl IntoFragment<'a>,
    font: Font,
    tone: Tone,
) -> TextInput<'a, Message> {
    text_input(placeholder, value)
        .font(font)
        .padding(theme::FIELD)
        .style(theme::text_input::toned(tone))
}

/// The width of a field that holds `typed`, with room for a few more.
pub fn width(
    typed: &str,
    font: Font,
) -> f32 {
    (width_of(typed, size::TEXT, font) + theme::FIELD.x() + spacing::L).max(extent::XXS)
}

/// A field as wide as what is `typed` in it.
pub fn fitted<'a, Message: Clone + 'a>(
    placeholder: &'a str,
    typed: String,
    font: Font,
    tone: Tone,
) -> TextInput<'a, Message> {
    let width = width(&typed, font);
    field(placeholder, typed, font, tone).width(width)
}

/// A field for a name, which keeps what is typed in it until Enter gives that or it is left.
pub fn name<'a, Message: 'a>(
    placeholder: &'static str,
    name: &'a str,
    is_taken: impl Fn(&str) -> bool + 'a,
    on_rename: impl Fn(String) -> Message + 'a,
) -> impl Widget<Message> + 'a {
    component(Name {
        placeholder,
        name,
        is_taken: Box::new(is_taken),
        on_rename: Box::new(on_rename),
    })
}

/// The field of a name: orange while another is typed in it, and red while that one is refused.
struct Name<'a, Message> {
    placeholder: &'static str,
    name: &'a str,
    is_taken: Box<dyn Fn(&str) -> bool + 'a>,
    on_rename: Box<dyn Fn(String) -> Message + 'a>,
}

/// What is done in the field of a name.
#[derive(Debug, Clone)]
enum Typing {
    Typed(String),
    Entered,
    /// The field lost the focus.
    Left,
}

impl<Message> Name<'_, Message> {
    /// What is being typed for its name: nothing that was typed for another whose field this was.
    fn typed<'s>(
        &self,
        typed: &'s Option<(String, String)>,
    ) -> Option<&'s str> {
        typed
            .as_ref()
            .filter(|(name, _)| name == self.name)
            .map(|(_, typed)| typed.as_str())
    }
}

impl<'a, Message> Component<'a, Message> for Name<'a, Message> {
    /// The name that is being typed for, and what is typed, until its field is left.
    type State = Option<(String, String)>;
    type Event = Typing;

    fn update(
        &self,
        typed: &mut Option<(String, String)>,
        event: Typing,
        _renderer: &Renderer,
    ) -> Option<Message> {
        match event {
            Typing::Typed(new) => *typed = Some((self.name.to_owned(), new)),
            Typing::Left => *typed = None,
            // one that is refused stays typed
            Typing::Entered => {
                let new = self.typed(typed);
                let is_given =
                    is_unsaved(new, self.name) && !is_refused(new, self.name, &self.is_taken);
                return new
                    .filter(|_| is_given)
                    .map(str::to_owned)
                    .map(&self.on_rename);
            }
        }
        None
    }

    fn view(
        &self,
        typed: &Option<(String, String)>,
    ) -> impl Widget<Typing> + 'a {
        let typed = self.typed(typed);
        let is_refused = is_refused(typed, self.name, &self.is_taken);
        let tone = match (is_unsaved(typed, self.name), is_refused) {
            (_, true) => Tone::Refused,
            (true, false) => Tone::Unsaved,
            (false, false) => Tone::Given,
        };
        let shown = typed
            .unwrap_or(self.name)
            .to_owned();
        let field = field(self.placeholder, shown, font::SANS, tone)
            .size(size::TITLE)
            .on_input(Typing::Typed)
            .on_submit(Typing::Entered);
        leave(field, Typing::Left)
    }
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

/// A list to pick one of `options` from, which shows `selected`, or else `placeholder`.
pub fn pick<'a, T: Clone + PartialEq + 'a, Message>(
    selected: Option<T>,
    options: Vec<T>,
    label: impl Fn(&T) -> String + 'a,
    placeholder: &str,
    font: Font,
    tone: Tone,
) -> PickList<'a, T, Message> {
    pick_list(selected, options, label)
        .placeholder(placeholder)
        .font(font)
        .style(theme::pick_list::toned(tone))
}

/// A box to tick, by the tone of whether it is ticked.
pub fn tick<'a, Message: 'a>(
    is_ticked: bool,
    tone: Tone,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    checkbox(is_ticked)
        .on_toggle(on_toggle)
        .style(theme::checkbox::toned(tone))
        .boxed()
}

/// A line to tick one of many on: a dot of its hue if it has one, the box, and its name.
pub fn ticked<'a, Message: 'a>(
    hue: Option<Color>,
    name: &'a str,
    is_ticked: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    let tick = checkbox(is_ticked)
        .label(name)
        .on_toggle(on_toggle)
        .style(theme::checkbox::toned(Tone::Given));
    row![
        hue.map(|hue| dot(hue, theme::dot::M, true)),
        tick
    ]
    .spacing(spacing::S)
    .align_y(Center)
    .boxed()
}

/// A dot of `hue`, this wide: filled, or else hollow.
pub fn dot<'a, Message: 'a>(
    hue: Color,
    side: f32,
    is_filled: bool,
) -> Element<'a, Message> {
    container(
        space()
            .width(side)
            .height(side),
    )
    .style(theme::hue::dot(hue, is_filled))
    .boxed()
}

/// `content`, which a right press on it opens `popup` under, until a press elsewhere closes that.
pub fn popped<'a, Message: Clone + 'a>(
    content: impl Widget<Message> + 'a,
    popup: Option<impl Widget<Message> + 'a>,
    on_open: Message,
    on_close: Message,
) -> Element<'a, Message> {
    popover(mouse_area(content).on_right_press(on_open), popup)
        .position(popover::Position::Bottom)
        .on_close(on_close)
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
        .spacing(spacing::XXS)
        .align_y(Center)
        .boxed()
}

/// A small button that is its label alone, among the lines of a card.
pub fn small<'a, Message: Clone + 'a>(
    label: &'a str,
    message: Message,
) -> Button<'a, Message, Text<'a>> {
    button(text(label).size(size::TITLE))
        .padding(theme::DENSE)
        .style(theme::button::text)
        .on_press(message)
}

/// A button of the toolbar around `content`, as tall as a pill.
pub fn pill<'a, Message: Clone + 'a>(content: impl Widget<Message> + 'a) -> Button<'a, Message> {
    button(
        container(content)
            .center_y(Fill)
            .boxed(),
    )
    .height(control::M)
    .padding([0.0, theme::BUTTON.left])
}

/// What a pill of the toolbar says.
pub fn title<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into())
        .size(size::TITLE)
        .center()
}

/// A round button of the size of a sign, around `content` in the middle of it.
pub fn round<'a, Message: Clone + 'a>(content: impl Widget<Message> + 'a) -> Button<'a, Message> {
    button(center(content).boxed())
        .width(control::S)
        .height(control::S)
        .padding(0)
}

/// A round button that is a sign.
fn signed<'a, Message: Clone + 'a>(
    sign: &'static str,
    message: Message,
) -> Button<'a, Message> {
    round(text(sign).size(size::HEADING)).on_press(message)
}

/// The button that lays something open, or closes it if it is open.
pub fn toggle<'a, Message: Clone + 'a>(
    is_open: bool,
    message: Message,
) -> Button<'a, Message> {
    let open = match is_open {
        true => sign::CLOSE,
        false => sign::OPEN,
    };
    signed(open, message).style(theme::button::icon)
}

/// The button that adds one more.
pub fn add<'a, Message: Clone + 'a>(message: Message) -> Button<'a, Message> {
    signed(sign::ADD, message).style(theme::button::text)
}

/// The button that clears a value, or takes one of many out.
pub fn clear<'a, Message: Clone + 'a>(message: Message) -> Button<'a, Message> {
    signed(sign::DELETE, message).style(theme::button::icon)
}

/// The room of a round button, on a line that has none where the lines beside it have one.
pub fn slot<'a, Message: 'a>() -> Element<'a, Message> {
    space()
        .width(control::S)
        .boxed()
}

/// The button that deletes what has a name.
pub fn cross<'a, Message: Clone + 'a>(message: Message) -> Button<'a, Message> {
    signed(sign::DELETE, message).style(theme::button::danger)
}

/// Text that reads as code.
pub fn code<'a>(written: impl Into<String>) -> Text<'a> {
    text(written.into()).font(font::MONO)
}

/// What is said beside a field: quietly what it stands for, on one line that is cut short.
pub fn aside<'a>(said: impl Into<String>) -> Text<'a> {
    text(said.into())
        .size(size::NOTE)
        .wrapping(text::Wrapping::None)
        .ellipsis(text::Ellipsis::End)
        .style(theme::text::muted)
}

/// `content` set in to start under the name on a line that a sign begins.
pub fn under_name<'a, Message: 'a>(content: impl Widget<Message> + 'a) -> Element<'a, Message> {
    container(content)
        .padding(padding::left(control::S + spacing::XS))
        .boxed()
}

/// Why a value is not taken, in the colour of its `tone`.
pub fn note<'a>(
    why: impl Into<String>,
    tone: Tone,
) -> Text<'a> {
    text(unmarked(&why.into()))
        .size(size::NOTE)
        .style(theme::text::toned(tone))
}

/// Why something is refused as it is shown: without the backticks the core marks names in.
pub fn unmarked(why: &str) -> String {
    why.replace('`', "")
}

/// The width of a column that holds the widest of the names of some arguments.
pub fn label_width<'a>(names: impl IntoIterator<Item = &'a str>) -> f32 {
    names
        .into_iter()
        .map(|name| width_of(name, size::TEXT, font::SANS))
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
fn told_at<'a, Message: 'a>(
    content: impl Widget<Message> + 'a,
    says: Option<String>,
    side: tooltip::Position,
) -> Element<'a, Message> {
    let Some(says) = says else {
        return content.boxed();
    };
    let says = container(text(says).size(size::NOTE))
        .width(extent::L)
        .padding(spacing::S)
        .style(theme::container::tooltip);
    tooltip(content, says)
        .position(side)
        .gap(spacing::XS)
        .boxed()
}

/// An argument on a line: its name in a column of this `width`, then its value.
pub fn labelled<'a, Message: 'a>(
    name: &'a str,
    width: f32,
    described: Option<&'a str>,
    value: impl Widget<Message> + 'a,
) -> Element<'a, Message> {
    let name = text(name)
        .wrapping(text::Wrapping::None)
        .ellipsis(text::Ellipsis::End);
    // its name tells what it is for when hovered
    let name = told(container(name).width(width), described.map(flowed));
    row![lined(name), value]
        .spacing(spacing::S)
        .boxed()
}

/// `content` in a line of the height of a field, as the cells of a table are.
pub fn lined<'a, Message: 'a>(content: impl Widget<Message> + 'a) -> Element<'a, Message> {
    container(content)
        .height(theme::LINE)
        .align_y(Center)
        .boxed()
}

/// A thin bar that scrolls what is too long for its box.
pub fn bar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(theme::BAR)
        .scroller_width(theme::BAR)
}

/// An error in its box, which can be selected to copy it; one long or wide is scrolled through.
pub fn error<'a, Message: 'a>(message: &'a str) -> Element<'a, Message> {
    let written = text(message)
        .style(theme::text::error)
        .wrapping(text::Wrapping::None)
        .selectable(true);
    let direction = scrollable::Direction::Both {
        vertical: bar(),
        horizontal: bar(),
    };
    // the bars lie over the edges of what is seen, so the text ends clear of them
    let room = padding::right(theme::BAR + spacing::XXS).bottom(theme::BAR + spacing::XXS);
    let scrolled = scrollable(container(written).padding(room))
        .direction(direction)
        .width(Fill)
        .height(Shrink.max(extent::S))
        // and no text shows through them: it ends at a bar as at an edge of its box
        .style(theme::scrollable::on_error);
    container(scrolled)
        .padding(spacing::S)
        .width(Fill)
        .style(theme::container::error)
        .boxed()
}
