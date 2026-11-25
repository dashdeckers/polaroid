//! A field that a drag adjusts, as a slider is dragged; a press alone is the field's.
//!
//! A slow drag steps evenly, to set a value exactly; a faster one steps more to the pixel.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Shell, Widget, mouse, overlay, renderer};
use iced::time::Instant;
use iced::widget::{container, text};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector, window};

/// How far a press is dragged before it adjusts, and how far then for each step.
const START: f32 = 4.0;
const STEP: f32 = 6.0;

/// The pixels a second up to which a drag steps evenly.
const SLOW: f32 = 200.0;
/// How steeply a drag steps more as it gets faster than that, and the most it ever does.
const POWER: f32 = 1.5;
const MOST: f32 = 40.0;

/// How many times the steps of a slow drag one at `speed` makes, in pixels a second.
fn gain(speed: f32) -> f32 {
    (speed / SLOW)
        .max(1.0)
        .powf(POWER)
        .min(MOST)
}

/// What a handle shows: arrows up and down.
pub const HANDLE: &str = "↕";

/// A small handle to set beside a field, which a drag up or down from the handle is to adjust.
pub fn handle<'a, Message: 'a>() -> Scrub<'a, Message> {
    scrub(container(text(HANDLE).size(14)).padding([0, 3])).upward()
}

/// A field that nothing adjusts until it is told what a drag asks for.
pub fn scrub<'a, Message>(field: impl Widget<Message, Theme, Renderer> + 'a) -> Scrub<'a, Message> {
    Scrub {
        field: field._boxed(),
        on_scrub: None,
        is_upward: false,
    }
}

/// A field, and what a drag across it asks for.
pub struct Scrub<'a, Message> {
    field: Element<'a, Message>,
    on_scrub: Option<Box<dyn Fn(i32) -> Option<Message> + 'a>>,
    /// Whether it is dragged up for more, and not to the right.
    is_upward: bool,
}

impl<'a, Message> Scrub<'a, Message> {
    /// Has a drag up and down adjust it, and not one across it.
    pub fn upward(mut self) -> Self {
        self.is_upward = true;
        self
    }

    /// How far along a drag is at `point`: more to the right, or higher up if it is dragged up.
    fn along(
        &self,
        point: Point,
    ) -> f32 {
        match self.is_upward {
            true => -point.y,
            false => point.x,
        }
    }

    /// What to ask for when a drag has gone this many steps further, or back if they are fewer.
    ///
    /// It is asked once a frame at the most, so that what is built of the answer is there to see.
    pub fn on_scrub(
        self,
        on_scrub: impl Fn(i32) -> Message + 'a,
    ) -> Self {
        self.on_scrub_maybe(move |steps| Some(on_scrub(steps)))
    }

    /// As [`Self::on_scrub`], but there may be nothing to ask for, as at the end of a range.
    pub fn on_scrub_maybe(
        mut self,
        on_scrub: impl Fn(i32) -> Option<Message> + 'a,
    ) -> Self {
        self.on_scrub = Some(Box::new(on_scrub));
        self
    }
}

#[derive(Default)]
struct State {
    is_pressed: bool,
    /// How far along the press was, or its drag at the latest frame.
    from: Option<f32>,
    /// How far along the cursor was last seen, in the window as every move tells it.
    ///
    /// The cursor itself is told as what holds the field sees it, panned, zoomed or scrolled.
    along: Option<f32>,
    is_dragged: bool,
    /// The part of a step that is not asked for yet.
    carried: f32,
    /// When the latest frame was, from which the speed of the drag is told.
    frame: Option<Instant>,
}

impl<Message> widget::Meta for Scrub<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Scrub<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(
        &mut self,
        tree: &mut Tree,
    ) {
        tree.diff_children(std::slice::from_mut(&mut self.field));
    }

    fn size(&self) -> Size<Length> {
        self.field.size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) {
        self.field
            .layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.field
            .operate(&mut tree.children[0], layout, viewport, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree
            .state
            .downcast_mut::<State>();
        // the field sees the release that ends a drag, as it saw the press
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            *state = State {
                along: state.along,
                ..State::default()
            };
        }
        // and nothing of the drag itself
        if !state.is_dragged {
            let field = &mut tree.children[0];
            self.field
                .update(field, event, layout, cursor, renderer, shell, viewport);
        }

        let Some(on_scrub) = &self.on_scrub else {
            return;
        };
        let state = tree
            .state
            .downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.is_pressed = cursor.is_over(layout.bounds());
                state.from = state.along;
                // the window selects text from a press that nothing takes, as far as it is dragged
                if state.is_pressed {
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let along = self.along(*position);
                state.along = Some(along);
                if !state.is_pressed {
                    return;
                }
                // a press before any move is dragged from where it is first moved to
                let from = *state
                    .from
                    .get_or_insert(along);
                if !state.is_dragged && (along - from).abs() < START {
                    return;
                }
                state.is_dragged = true;
                shell.capture_event();
                shell.request_redraw();
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                let since = state
                    .frame
                    .replace(*now)
                    .map(|frame| (*now - frame).as_secs_f32());
                let (true, Some(from), Some(along)) = (state.is_dragged, state.from, state.along)
                else {
                    return;
                };
                let moved = along - from;
                state.from = Some(along);
                // the first frame of a drag tells no speed, and steps as a slow one
                let speed = since
                    .filter(|since| *since > 0.0)
                    .map_or(0.0, |since| moved.abs() / since);
                state.carried += moved / STEP * gain(speed);
                let steps = state
                    .carried
                    .trunc();
                if steps != 0.0 {
                    state.carried -= steps;
                    if let Some(message) = on_scrub(steps as i32) {
                        shell.publish(message);
                    }
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree
            .state
            .downcast_ref::<State>();
        match (state.is_dragged, self.is_upward) {
            (true, true) => return mouse::Interaction::ResizingVertically,
            (true, false) => return mouse::Interaction::ResizingHorizontally,
            (false, _) => {}
        }
        let shown = self
            .field
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer);
        // what shows no cursor of its own, as a handle, shows that it is to be dragged
        let is_plain = shown == mouse::Interaction::default();
        match self.is_upward {
            _ if !is_plain
                || self
                    .on_scrub
                    .is_none()
                || !cursor.is_over(layout.bounds()) =>
            {
                shown
            }
            true => mouse::Interaction::ResizingVertically,
            false => mouse::Interaction::ResizingHorizontally,
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.field.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'a, Message, Theme, Renderer>> {
        self.field.overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }
}
