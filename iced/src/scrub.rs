//! A field that a drag across it adjusts, as a slider is dragged; a press alone is the field's.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Shell, Widget, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

/// How far a press is dragged before it adjusts, and how far then for each step.
const START: f32 = 4.0;
const STEP: f32 = 6.0;

/// A field that nothing adjusts until it is told what a drag asks for.
pub fn scrub<'a, Message>(field: impl Widget<Message, Theme, Renderer> + 'a) -> Scrub<'a, Message> {
    Scrub {
        field: field._boxed(),
        on_scrub: None,
    }
}

/// A field, and what a drag across it asks for.
pub struct Scrub<'a, Message> {
    field: Element<'a, Message>,
    on_scrub: Option<Box<dyn Fn(i32) -> Message + 'a>>,
}

impl<'a, Message> Scrub<'a, Message> {
    /// What to ask for when a drag has gone this many steps further: to the right, or the left.
    pub fn on_scrub(
        mut self,
        on_scrub: impl Fn(i32) -> Message + 'a,
    ) -> Self {
        self.on_scrub = Some(Box::new(on_scrub));
        self
    }
}

#[derive(Default)]
struct State {
    /// Where the press was, or the latest step of its drag.
    pressed: Option<f32>,
    is_dragged: bool,
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
            *state = State::default();
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
                state.pressed = cursor
                    .position_over(layout.bounds())
                    .map(|position| position.x);
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let Some(from) = state.pressed else {
                    return;
                };
                let moved = position.x - from;
                if !state.is_dragged && moved.abs() < START {
                    return;
                }
                state.is_dragged = true;
                let steps = (moved / STEP).trunc();
                if steps != 0.0 {
                    state.pressed = Some(from + steps * STEP);
                    shell.publish(on_scrub(steps as i32));
                }
                shell.capture_event();
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
        if state.is_dragged {
            return mouse::Interaction::ResizingHorizontally;
        }
        self.field
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
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
