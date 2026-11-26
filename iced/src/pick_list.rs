//! A list to pick one of some options from, that opens as wide as the widest of them.
//!
//! Closed, it is as wide as what it shows, unless it is told how wide to be. An option may be one
//! that cannot be picked: it is listed greyed, with why beside it. A list too long scrolls.

use std::rc::Rc;

use iced::advanced::layout::{self, Layout};
use iced::advanced::text::{self, Renderer as _};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Shell, Text, Widget, mouse, overlay, renderer};
use iced::alignment::Vertical;
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

use crate::theme::{self, Tone, spacing};
use crate::{handle, width_of};

/// The room an open list has over that of its widest option, for what rounding takes.
const SLACK: f32 = spacing::XS;

/// The room above the first option of an open list and below its last.
const EDGE: f32 = theme::menu::EDGE;

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
        disabled: None,
        placeholder: String::new(),
        width: Length::Shrink,
        padding: theme::FIELD,
        text_size: None,
        font: None,
        style: Box::new(theme::pick_list::toned(Tone::Given)),
        status: None,
    }
}

/// A list to pick from, and what it is drawn with.
pub struct PickList<'a, T, Message> {
    selected: Option<T>,
    options: Vec<T>,
    to_string: Box<dyn Fn(&T) -> String + 'a>,
    on_select: Option<Rc<dyn Fn(T) -> Message + 'a>>,
    /// Why an option cannot be picked, if it cannot.
    disabled: Option<Rc<dyn Fn(&T) -> Option<String> + 'a>>,
    placeholder: String,
    width: Length,
    padding: Padding,
    text_size: Option<Pixels>,
    font: Option<Font>,
    style: StyleFn<'a, Theme>,
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

    /// Which options cannot be picked, each with why: listed greyed, and passed over by a step.
    pub fn disabled(
        mut self,
        why: impl Fn(&T) -> Option<String> + 'a,
    ) -> Self {
        self.disabled = Some(Rc::new(why));
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
        // those that can be picked alone
        let options: Vec<T> = self
            .options
            .iter()
            .filter(|option| {
                self.why(option)
                    .is_none()
            })
            .cloned()
            .collect();
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
        .spacing(spacing::XXS)
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

    /// Why `option` cannot be picked, if it cannot.
    fn why(
        &self,
        option: &T,
    ) -> Option<String> {
        self.disabled
            .as_ref()
            .and_then(|why| why(option))
    }

    /// How tall the row of an option of the open list is, at this size of text.
    fn row(
        &self,
        size: Pixels,
    ) -> f32 {
        theme::LINE_HEIGHT
            .to_absolute(size)
            .0
            + self.padding.y()
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
    modifiers: keyboard::Modifiers,
    is_open: bool,
    hovered: Option<usize>,
    /// The width of the widest option and why it cannot be picked, as measured when it opened.
    widest: f32,
    /// How far the open list is scrolled down.
    offset: f32,
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
        let line = theme::LINE_HEIGHT.to_absolute(size);
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
                // scrolled to what is selected
                state.offset = state
                    .hovered
                    .map_or(0.0, |at| at as f32 * self.row(size));
                let wide = |text: String| width_of(&text, size.0, font);
                state.widest = self
                    .options
                    .iter()
                    .map(|option| {
                        let why = self.why(option);
                        wide((self.to_string)(option))
                            + why.map_or(0.0, |why| theme::menu::GAP + wide(why))
                    })
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
                // past those that cannot be picked
                let len = self.options.len();
                let free = |at: &usize| {
                    self.why(&self.options[*at])
                        .is_none()
                };
                let at = self
                    .selected
                    .as_ref()
                    .and_then(place);
                let next = match (at, *y < 0.0) {
                    _ if *y == 0.0 => None,
                    (Some(at), true) => (at + 1..len).find(free),
                    (Some(at), false) => (0..at)
                        .rev()
                        .find(free),
                    (None, true) => (0..len).find(free),
                    (None, false) => (0..len)
                        .rev()
                        .find(free),
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
                theme::LINE_HEIGHT
                    .to_absolute(size)
                    .0,
            ),
            size,
            line_height: theme::LINE_HEIGHT,
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
            .as_deref()
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

        // under it, or over it where there is more room, and scrolled where there is too little
        let row = self.row(size);
        let all = self.options.len() as f32 * row + 2.0 * EDGE;
        let below = window.height - (position.y + bounds.height);
        let height = all.min(below.max(position.y));
        let y = match all <= below || below >= position.y {
            true => position.y + bounds.height,
            false => position.y - height,
        };
        state.offset = state
            .offset
            .clamp(0.0, all - height);
        let list = List {
            state,
            options: &self.options,
            to_string: &*self.to_string,
            disabled: self
                .disabled
                .as_deref(),
            on_select,
            bounds: Rectangle::new(Point::new(position.x, y), Size::new(width, height)),
            row,
            padding: self.padding,
            font,
            size,
        };
        vec![overlay::Element::new(
            Box::new(list),
        )]
    }
}

/// The options of an open list, over all else: a row of each, the one under the cursor lit.
struct List<'a, 'b, T, Message> {
    state: &'b mut State,
    options: &'b [T],
    to_string: &'b (dyn Fn(&T) -> String + 'a),
    disabled: Option<&'b (dyn Fn(&T) -> Option<String> + 'a)>,
    on_select: &'b (dyn Fn(T) -> Message + 'a),
    /// Where it lies in the window.
    bounds: Rectangle,
    /// How tall a row is.
    row: f32,
    padding: Padding,
    font: Font,
    size: Pixels,
}

impl<T, Message> List<'_, '_, T, Message> {
    /// The option whose row is under the cursor, if any.
    fn under(
        &self,
        cursor: mouse::Cursor,
    ) -> Option<usize> {
        let point = cursor.position_over(self.bounds)?;
        let down = point.y - self.bounds.y - EDGE + self.state.offset;
        let at = (down / self.row) as usize;
        (down >= 0.0 && at < self.options.len()).then_some(at)
    }

    /// Where the row of the option at `at` lies, scrolled.
    fn row(
        &self,
        at: usize,
    ) -> Rectangle {
        let y = self.bounds.y + EDGE + at as f32 * self.row - self.state.offset;
        Rectangle::new(
            Point::new(self.bounds.x, y),
            Size::new(self.bounds.width, self.row),
        )
    }

    /// Why the option at `at` cannot be picked, if it cannot.
    fn why(
        &self,
        at: usize,
    ) -> Option<String> {
        self.disabled
            .and_then(|why| why(&self.options[at]))
    }
}

impl<T: Clone, Message> overlay::Overlay<Message, Theme, Renderer> for List<'_, '_, T, Message> {
    fn update(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let under = self.under(cursor);
        match event {
            // what cannot be picked is not lit
            Event::Mouse(mouse::Event::CursorMoved { .. }) if under.is_some() => {
                let lit = under.filter(|at| {
                    self.why(*at)
                        .is_none()
                });
                if self.state.hovered != lit {
                    self.state.hovered = lit;
                    shell.request_redraw();
                }
            }
            // and a press on it is taken, and does nothing
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(at) = under else {
                    return;
                };
                if self
                    .why(at)
                    .is_none()
                {
                    self.state.is_open = false;
                    shell.publish((self.on_select)(self.options[at].clone()));
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(self.bounds) => {
                let by = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y * self.row,
                    mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                let most = self.options.len() as f32 * self.row + 2.0 * EDGE - self.bounds.height;
                self.state.offset = (self.state.offset - by).clamp(0.0, most.max(0.0));
                shell.capture_event();
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn operate(
        &mut self,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        // the rows in view, for what looks for their text
        for at in 0..self.options.len() {
            let row = self.row(at);
            if row.y >= self.bounds.y && row.y + row.height <= self.bounds.y + self.bounds.height {
                operation.text(None, row, &mut (self.to_string)(&self.options[at]));
            }
        }
    }

    fn mouse_interaction(
        &self,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        match self.under(cursor) {
            Some(at)
                if self
                    .why(at)
                    .is_none() =>
            {
                mouse::Interaction::Pointer
            }
            Some(_) => mouse::Interaction::NotAllowed,
            None => mouse::Interaction::None,
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        _cursor: mouse::Cursor,
    ) {
        use iced::advanced::Renderer as _;

        let style = theme::menu::list(theme);
        let bounds = self.bounds;
        let hint = renderer.hint_factor();
        let line = |content: String, row: Rectangle, align_x: text::Alignment| Text {
            content,
            bounds: Size::new(row.width - self.padding.x(), row.height),
            size: self.size,
            line_height: theme::LINE_HEIGHT,
            font: self.font,
            align_x,
            align_y: Vertical::Center,
            shaping: text::Shaping::default(),
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::End,
            hint_factor: hint,
        };
        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    shadow: style.shadow,
                    ..renderer::Quad::default()
                },
                style.background,
            );
            // the rows in view alone
            let above = self.state.offset - EDGE;
            let first = (above / self.row) as usize;
            let last = ((above + bounds.height) / self.row).ceil() as usize;
            for at in first..last.min(self.options.len()) {
                let row = self.row(at);
                let why = self.why(at);
                let is_lit = self.state.hovered == Some(at);
                if is_lit {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: row.shrink([0.0, EDGE]),
                            border: style.mark,
                            ..renderer::Quad::default()
                        },
                        style.lit,
                    );
                }
                let color = match (&why, is_lit) {
                    (Some(_), _) => style.disabled,
                    (None, true) => style.on_lit,
                    (None, false) => style.text,
                };
                let option = (self.to_string)(&self.options[at]);
                let left = Point::new(row.x + self.padding.left, row.center_y());
                renderer.fill_text(
                    line(option, row, text::Alignment::Default),
                    left,
                    color,
                    bounds,
                );
                // why it cannot be picked, at the end of its row
                if let Some(why) = why {
                    let right = Point::new(row.x + row.width - self.padding.right, row.center_y());
                    renderer.fill_text(
                        line(why, row, text::Alignment::Right),
                        right,
                        style.aside,
                        bounds,
                    );
                }
            }
        });
    }
}
