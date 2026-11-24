//! A list to pick one of some options from, that opens as wide as the widest of them.
//!
//! Closed, it is as wide as what it shows, unless it is told how wide to be.

use std::rc::Rc;

use iced::advanced::layout::{self, Layout};
use iced::advanced::text::{self, Renderer as _};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Shell, Text, Widget, mouse, overlay, renderer};
use iced::alignment::Vertical;
use iced::overlay::menu::{self, Menu};
pub use iced::widget::pick_list::{Status, Style, StyleFn, default};
use iced::widget::row;
use iced::{
    Center,
    Element,
    Event,
    Font,
    Length,
    Padding,
    Pixels,
    Point,
    Rectangle,
    Renderer,
    Size,
    Theme,
    Vector,
    keyboard,
    window,
};

use crate::{handle, width_of};

/// The room an open list has over that of its widest option, for what rounding takes.
const SLACK: f32 = 4.0;

/// A list of `options` that shows the one `selected`, each as the text that `to_string` gives.
pub fn pick_list<'a, T: Clone + PartialEq, Message>(
    selected: Option<T>,
    options: Vec<T>,
    to_string: impl Fn(&T) -> String + 'a,
) -> PickList<'a, T, Message> {
    PickList {
        selected,
        options,
        to_string: Box::new(to_string),
        on_select: None,
        placeholder: String::new(),
        width: Length::Shrink,
        padding: Padding::new(5.0).horizontal(10),
        text_size: None,
        font: None,
        style: Box::new(default),
        menu_style: Box::new(menu::default),
        status: None,
    }
}

/// A list to pick from, and what it is drawn with.
pub struct PickList<'a, T, Message> {
    selected: Option<T>,
    options: Vec<T>,
    to_string: Box<dyn Fn(&T) -> String + 'a>,
    on_select: Option<Rc<dyn Fn(T) -> Message + 'a>>,
    placeholder: String,
    width: Length,
    padding: Padding,
    text_size: Option<Pixels>,
    font: Option<Font>,
    style: StyleFn<'a, Theme>,
    menu_style: menu::StyleFn<'a, Theme>,
    /// How it was at the latest frame, which tells when to draw it again.
    status: Option<Status>,
}

impl<'a, T, Message> PickList<'a, T, Message> {
    /// What to ask for when an option is picked; without it nothing opens.
    pub fn on_select(
        mut self,
        on_select: impl Fn(T) -> Message + 'a,
    ) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }

    /// It beside a handle, which a drag down steps through the options, and one up back.
    pub fn stepped(self) -> Element<'a, Message>
    where
        T: Clone + PartialEq + 'a,
        Message: 'a,
    {
        let Some(on_select) = self
            .on_select
            .clone()
        else {
            return self._boxed();
        };
        let options = self.options.clone();
        let at = self
            .selected
            .as_ref()
            .and_then(|selected| {
                options
                    .iter()
                    .position(|option| option == selected)
            });
        let step = move |steps: i32| {
            let last = options
                .len()
                .saturating_sub(1) as i64;
            // from before the first, if none is selected
            let from = at.map_or(-1, |at| at as i64);
            // down the list is down the screen
            let to = (from - i64::from(steps)).clamp(0, last) as usize;
            (Some(to) != at)
                .then(|| options.get(to))
                .flatten()
                .map(|option| on_select(option.clone()))
        };
        row![
            self,
            handle().on_scrub_maybe(step)
        ]
        .spacing(2)
        .align_y(Center)
        ._boxed()
    }

    /// What it shows while nothing is selected.
    pub fn placeholder(
        mut self,
        placeholder: impl Into<String>,
    ) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// How wide it is while closed; open, it is no narrower than its widest option.
    pub fn width(
        mut self,
        width: impl Into<Length>,
    ) -> Self {
        self.width = width.into();
        self
    }

    pub fn padding(
        mut self,
        padding: impl Into<Padding>,
    ) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn text_size(
        mut self,
        size: impl Into<Pixels>,
    ) -> Self {
        self.text_size = Some(size.into());
        self
    }

    pub fn font(
        mut self,
        font: impl Into<Font>,
    ) -> Self {
        self.font = Some(font.into());
        self
    }

    pub fn style(
        mut self,
        style: impl Fn(&Theme, Status) -> Style + 'a,
    ) -> Self {
        self.style = Box::new(style);
        self
    }

    /// The text it shows: that of what is selected, or else the placeholder.
    fn shown(&self) -> String {
        self.selected
            .as_ref()
            .map_or_else(
                || {
                    self.placeholder
                        .clone()
                },
                &self.to_string,
            )
    }

    /// The font and the size of its text.
    fn text(
        &self,
        renderer: &Renderer,
    ) -> (Font, Pixels) {
        (
            self.font
                .unwrap_or_else(|| renderer.font()),
            self.text_size
                .unwrap_or_else(|| renderer.text_size()),
        )
    }
}

#[derive(Default)]
struct State {
    menu: menu::State,
    modifiers: keyboard::Modifiers,
    is_open: bool,
    hovered: Option<usize>,
    /// The width of the widest option, as measured when it opened.
    widest: f32,
}

impl<T, Message> widget::Meta for PickList<'_, T, Message> {}

impl<T: Clone + PartialEq, Message> Widget<Message, Theme, Renderer> for PickList<'_, T, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) {
        let (font, size) = self.text(renderer);
        let line = text::LineHeight::default().to_absolute(size);
        // its text, and beside that the arrow
        let wide = match self.width {
            Length::Shrink => width_of(&self.shown(), size.0, font).ceil(),
            _ => 0.0,
        };
        let intrinsic = Size::new(wide + size.0 + self.padding.left, line.0);
        tree.size = limits
            .width(self.width)
            .shrink(self.padding)
            .resolve(self.width, Length::Shrink, intrinsic)
            .expand(self.padding);
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout,
        _viewport: &Rectangle,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.text(None, layout.bounds(), &mut self.shown());
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree
            .state
            .downcast_mut::<State>();
        let is_over = cursor.is_over(layout.bounds());
        let place = |selected: &T| {
            self.options
                .iter()
                .position(|option| option == selected)
        };

        match event {
            // a press that the open list did not take is one beside it, which closes it
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if state.is_open => {
                state.is_open = false;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if is_over
                    && self
                        .on_select
                        .is_some() =>
            {
                let (font, size) = self.text(renderer);
                state.is_open = true;
                state.hovered = self
                    .selected
                    .as_ref()
                    .and_then(place);
                state.widest = self
                    .options
                    .iter()
                    .map(|option| width_of(&(self.to_string)(option), size.0, font))
                    .fold(0.0, f32::max);
                shell.capture_event();
            }
            // the wheel with Ctrl steps through the options without opening it
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { y, .. },
            }) if is_over
                && !state.is_open
                && state
                    .modifiers
                    .command() =>
            {
                let Some(on_select) = &self.on_select else {
                    return;
                };
                let last = self
                    .options
                    .len()
                    .saturating_sub(1);
                let at = self
                    .selected
                    .as_ref()
                    .and_then(place);
                let next = match (at, *y < 0.0) {
                    _ if *y == 0.0 => None,
                    (Some(at), true) => Some(at + 1).filter(|next| *next <= last),
                    (Some(at), false) => at.checked_sub(1),
                    (None, true) => Some(0),
                    (None, false) => Some(last),
                };
                if let Some(option) = next.and_then(|next| {
                    self.options
                        .get(next)
                }) {
                    shell.publish(on_select(option.clone()));
                }
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
            }
            _ => {}
        }

        let status = match (&self.on_select, state.is_open) {
            (None, _) => Status::Disabled,
            (Some(_), true) => Status::Opened {
                is_hovered: is_over,
            },
            (Some(_), false) if is_over => Status::Hovered,
            (Some(_), false) => Status::Active,
        };
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            self.status = Some(status);
        } else if self
            .status
            .is_some_and(|last| last != status)
        {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        match (cursor.is_over(layout.bounds()), &self.on_select) {
            (true, Some(_)) => mouse::Interaction::Pointer,
            (true, None) => mouse::Interaction::Idle,
            (false, _) => mouse::Interaction::default(),
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;

        let bounds = layout.bounds();
        let (font, size) = self.text(renderer);
        let status = self
            .status
            .unwrap_or(Status::Active);
        let style = (self.style)(theme, status);
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let hint = renderer.hint_factor();
        let line = |content: String, font: Font, align_x: text::Alignment| Text {
            content,
            bounds: Size::new(
                bounds.width - self.padding.x(),
                text::LineHeight::default()
                    .to_absolute(size)
                    .0,
            ),
            size,
            line_height: text::LineHeight::default(),
            font,
            align_x,
            align_y: Vertical::Center,
            shaping: text::Shaping::default(),
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::End,
            hint_factor: hint,
        };
        let arrow = line(
            Renderer::ARROW_DOWN_ICON.to_string(),
            Renderer::ICON_FONT,
            text::Alignment::Right,
        );
        let right = bounds.x + bounds.width - self.padding.right;
        let at = Point::new(right, bounds.center_y());
        renderer.fill_text(arrow, at, style.handle_color, *viewport);

        let color = match self.selected {
            Some(_) => style.text_color,
            None => style.placeholder_color,
        };
        let shown = line(self.shown(), font, text::Alignment::Default);
        let at = Point::new(bounds.x + self.padding.left, bounds.center_y());
        renderer.fill_text(shown, at, color, *viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree
            .state
            .downcast_mut::<State>();
        let Some(on_select) = self
            .on_select
            .as_ref()
            .filter(|_| state.is_open)
        else {
            return Vec::new();
        };
        let (font, size) = self.text(renderer);
        let bounds = layout.bounds();
        // as wide as its widest option, and moved left for as much as the window cuts off
        let width = bounds
            .width
            .max(state.widest + self.padding.x() + SLACK);
        let mut position = layout.position() + translation;
        position.x = position
            .x
            .min(window.width - width)
            .max(0.0);

        let menu = Menu::new(
            &mut state.menu,
            &self.options,
            &mut state.hovered,
            &self.to_string,
            |option| {
                state.is_open = false;
                on_select(option)
            },
            None,
            &self.menu_style,
        )
        .width(width)
        .padding(self.padding)
        .font(font)
        .text_size(size)
        .ellipsis(text::Ellipsis::End);
        vec![menu.overlay(renderer, position, window, bounds.height)]
    }
}
