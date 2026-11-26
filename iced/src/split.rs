//! A side beside a main part, with an edge between them to drag the side wider or narrower.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Renderer as _, Shell, Widget, mouse, overlay, renderer};
use iced::{Element, Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector};

use crate::theme::{Scheme, extent, spacing, stroke};

/// The width of the edge, and how far beside it a press still takes it.
const EDGE: f32 = stroke::THIN;
const REACH: f32 = spacing::XS;
/// The least that the side and the main part are wide.
const LEAST: f32 = extent::S;

/// A side on the left of a main part.
pub fn split<'a, Message>(
    side: impl Widget<Message, Theme, Renderer> + 'a,
    main: impl Widget<Message, Theme, Renderer> + 'a,
) -> Split<'a, Message> {
    Split {
        parts: [side._boxed(), main._boxed()],
        width: extent::XL,
        is_shown: true,
    }
}

/// A side and a main part; what the main part keeps stays while the side is hidden.
pub struct Split<'a, Message> {
    parts: [Element<'a, Message>; 2],
    width: f32,
    is_shown: bool,
}

impl<Message> Split<'_, Message> {
    /// How wide the side is until its edge is dragged.
    pub fn width(
        mut self,
        width: f32,
    ) -> Self {
        self.width = width;
        self
    }

    /// Whether the side is shown; hidden, the main part has all the room.
    pub fn shown(
        mut self,
        is_shown: bool,
    ) -> Self {
        self.is_shown = is_shown;
        self
    }

    /// The edge, and the room beside it that takes a press for it.
    fn edge(
        &self,
        tree: &Tree,
        layout: Layout,
    ) -> Option<Rectangle> {
        let bounds = layout.bounds();
        let side = tree.children[0]
            .size
            .width;
        self.is_shown
            .then_some(Rectangle {
                x: bounds.x + side - REACH,
                width: EDGE + 2.0 * REACH,
                ..bounds
            })
    }
}

/// The width of the side once its edge was dragged, and what the cursor is doing to the edge.
#[derive(Default)]
struct State {
    width: Option<f32>,
    is_dragged: bool,
    /// Whether the cursor is where a press would take the edge.
    is_lit: bool,
}

impl<Message> widget::Meta for Split<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Split<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Fill, Fill)
    }

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
        tree.diff_children(&mut self.parts);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) {
        let size = limits.resolve(Fill, Fill, Size::ZERO);
        let state = tree
            .state
            .downcast_ref::<State>();
        let most = (size.width - LEAST - EDGE).max(0.0);
        let side = match self.is_shown {
            true => state
                .width
                .unwrap_or(self.width)
                .max(LEAST)
                .min(most),
            false => 0.0,
        };
        let edge = match self.is_shown {
            true => EDGE,
            false => 0.0,
        };
        let widths = [side, size.width - side - edge];
        let lefts = [0.0, side + edge];

        for (index, part) in self
            .parts
            .iter_mut()
            .enumerate()
        {
            let part_size = Size::new(widths[index], size.height);
            let tree = &mut tree.children[index];
            part.layout(tree, renderer, &layout::Limits::new(part_size, part_size));
            tree.size = part_size;
            tree.translation = Vector::new(lefts[index], 0.0);
        }
        tree.size = size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let shown = usize::from(!self.is_shown);
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            self.parts
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .skip(shown)
                .for_each(|(part, (layout, tree))| {
                    part.operate(tree, layout, viewport, renderer, operation);
                });
        });
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
        let bounds = layout.bounds();
        let is_on_edge = self
            .edge(tree, layout)
            .is_some_and(|edge| cursor.is_over(edge));
        let state = tree
            .state
            .downcast_ref::<State>();
        // the parts see nothing of a drag of the edge
        if !state.is_dragged {
            let shown = usize::from(!self.is_shown);
            self.parts
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .skip(shown)
                .for_each(|(part, (layout, tree))| {
                    part.update(tree, event, layout, cursor, renderer, shell, viewport);
                });
        }

        let state = tree
            .state
            .downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.is_dragged => {
                state.width = Some(position.x - bounds.x);
                shell.invalidate_layout();
                shell.capture_event();
            }
            // the edge lights up under the cursor
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.is_lit != is_on_edge => {
                state.is_lit = is_on_edge;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.is_dragged => {
                state.is_dragged = false;
            }
            // a press beside the edge is a part's if it took it, as a scrollbar there does
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if is_on_edge && !shell.is_event_captured() =>
            {
                state.is_dragged = true;
                shell.capture_event();
            }
            _ => return,
        }
        shell.request_redraw();
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
        let is_on_edge = self
            .edge(tree, layout)
            .is_some_and(|edge| cursor.is_over(edge));
        if state.is_dragged || is_on_edge {
            return mouse::Interaction::ResizingColumn;
        }
        self.parts
            .iter()
            .zip(layout.iter(&tree.children))
            .skip(usize::from(!self.is_shown))
            .map(|(part, (layout, tree))| {
                part.mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .find(|interaction| *interaction != mouse::Interaction::None)
            .unwrap_or_default()
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
        for (part, (layout, tree)) in self
            .parts
            .iter()
            .zip(layout.iter(&tree.children))
            .skip(usize::from(!self.is_shown))
        {
            // each clipped to its own room, as a part may draw past it
            if let Some(clip) = layout
                .bounds()
                .intersection(viewport)
            {
                renderer.with_layer(clip, |renderer| {
                    part.draw(tree, renderer, theme, style, layout, cursor, &clip);
                });
            }
        }
        if let Some(edge) = self.edge(tree, layout) {
            let state = tree
                .state
                .downcast_ref::<State>();
            let scheme = Scheme::of(theme);
            // wider and in colour where a press would take it
            let (grown, color) = match state.is_dragged || state.is_lit {
                true => (stroke::THIN, scheme.primary.color),
                false => (0.0, scheme.outline_variant),
            };
            let line = Rectangle {
                x: edge.x + REACH - grown,
                width: EDGE + 2.0 * grown,
                ..edge
            };
            let quad = renderer::Quad {
                bounds: line,
                ..renderer::Quad::default()
            };
            renderer.fill_quad(quad, color);
        }
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
        let shown = usize::from(!self.is_shown);
        self.parts
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
            .skip(shown)
            .flat_map(|(part, (layout, tree))| {
                part.overlay(tree, layout, renderer, viewport, translation, window)
            })
            .collect()
    }
}
