//! An operation as the widgets of a card are to meet it: its points are where they see them.
//!
//! Text is selected on the card where the selection starts, and on no other.

use iced::advanced::text::{Fragment, Target};
use iced::advanced::widget::operation::{self, Focusable, Outcome, Scrollable, TextInput};
use iced::advanced::widget::{Id, Operation};
use iced::{Point, Rectangle, Size, Transformation, Vector};

/// An operation, and what takes a point of the screen to where the widgets it meets see it.
pub struct Seen<'a, T> {
    pub operation: &'a mut dyn Operation<T>,
    pub inverse: Transformation,
    /// Where the card of the widgets is, as they see it.
    pub card: Rectangle,
    /// How far what scrolls has moved the widgets it meets, which a selection adds to its points.
    pub scrolled: Vector,
}

impl<T> Operation<T> for Seen<'_, T> {
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn Operation<T>),
    ) {
        // what scrolls moves what comes after it and below, and nothing above
        let (inverse, card, scrolled) = (self.inverse, self.card, self.scrolled);
        self.operation
            .traverse(&mut |operation| {
                operate(&mut Seen {
                    operation,
                    inverse,
                    card,
                    scrolled,
                })
            });
    }

    fn container(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        viewport: &Rectangle,
    ) {
        self.operation
            .container(id, bounds, viewport);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content: Size,
        translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        self.scrolled += translation;
        self.operation
            .scrollable(id, bounds, content, translation, state);
    }

    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn Focusable,
    ) {
        self.operation
            .focusable(id, bounds, state);
    }

    fn text_input(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn TextInput,
    ) {
        self.operation
            .text_input(id, bounds, state);
    }

    fn text(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn operation::Text,
    ) {
        let mut text = Text {
            state,
            inverse: self.inverse,
            card: self.card,
            scrolled: self.scrolled,
        };
        self.operation
            .text(id, bounds, &mut text);
    }

    fn custom(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn std::any::Any,
    ) {
        self.operation
            .custom(id, bounds, state);
    }

    fn finish(&self) -> Outcome<T> {
        self.operation
            .finish()
    }
}

/// Text that is selected between points of the screen, which it sees elsewhere.
struct Text<'a> {
    state: &'a mut dyn operation::Text,
    inverse: Transformation,
    card: Rectangle,
    scrolled: Vector,
}

impl operation::Text for Text<'_> {
    fn text(&self) -> Fragment<'_> {
        self.state.text()
    }

    fn select(
        &mut self,
        start: Point,
        end: Point,
        target: Target,
    ) {
        // the points come with what was scrolled added, which is as the widgets see it already
        let seen = |point: Point| (point - self.scrolled) * self.inverse;
        let (start, end) = (seen(start), seen(end));
        match self
            .card
            .contains(start)
        {
            true => self
                .state
                .select(start + self.scrolled, end + self.scrolled, target),
            false => self
                .state
                .deselect(),
        }
    }

    fn select_all(&mut self) {
        self.state
            .select_all();
    }

    fn deselect(&mut self) {
        self.state
            .deselect();
    }

    fn copy(&mut self) -> Option<String> {
        self.state.copy()
    }
}
