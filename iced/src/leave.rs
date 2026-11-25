//! A field that asks for something when it is left: when it loses the focus that it had.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::operation::{self, Operation, Outcome, focusable};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Shell, Widget, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

/// `field`, which asks for `on_leave` when it loses the focus.
pub fn leave<'a, Message: Clone + 'a>(
    field: impl Widget<Message, Theme, Renderer> + 'a,
    on_leave: Message,
) -> Leave<'a, Message> {
    Leave {
        field: field._boxed(),
        on_leave,
    }
}

/// A field, and what it asks for when it is left; to all else it is the field, tree and all.
pub struct Leave<'a, Message> {
    field: Element<'a, Message>,
    on_leave: Message,
}

impl<Message> Leave<'_, Message> {
    /// Whether the field has the focus.
    fn is_focused(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> bool {
        let mut count = focusable::count();
        self.field.operate(
            tree,
            layout,
            viewport,
            renderer,
            &mut operation::black_box(&mut count),
        );
        matches!(
            count.finish(),
            Outcome::Some(focusable::Count {
                focused: Some(_),
                ..
            })
        )
    }
}

impl<Message> widget::Meta for Leave<'_, Message> {}

impl<Message: Clone> Widget<Message, Theme, Renderer> for Leave<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.field.size()
    }

    fn tag(&self) -> tree::Tag {
        self.field.tag()
    }

    fn state(&self) -> tree::State {
        self.field.state()
    }

    fn diff(
        &mut self,
        tree: &mut Tree,
    ) {
        self.field
            .diff(tree);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) {
        self.field
            .layout(tree, renderer, limits);
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
            .operate(tree, layout, viewport, renderer, operation);
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
        let was_focused = self.is_focused(tree, layout, viewport, renderer);
        self.field
            .update(tree, event, layout, cursor, renderer, shell, viewport);
        if was_focused && !self.is_focused(tree, layout, viewport, renderer) {
            shell.publish(
                self.on_leave
                    .clone(),
            );
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
        self.field
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        self.field
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
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
        self.field
            .overlay(tree, layout, renderer, viewport, translation, window)
    }
}
