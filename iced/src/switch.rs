//! A switch of stops along a pill: its thumb slides to the chosen stop, glows, and may be a button.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Renderer as _, Shell, Widget, mouse, overlay, renderer};
use iced::time::Instant;
use iced::{
    Animation,
    Color,
    Element,
    Event,
    Length,
    Rectangle,
    Renderer,
    Size,
    Theme,
    Vector,
    border,
    window,
};

use crate::theme::{self, Role, Scheme, control, motion, opacity, over, radius, spacing, stroke};

/// How far the thumb lies inside the track.
const INSET: f32 = spacing::XS;
/// How far a press on the thumb is dragged before the thumb follows it.
const START: f32 = theme::DRAG;

/// A switch whose thumb is at the stop of this index, and which has no stops yet.
pub fn switch<'a, Message>(chosen: usize) -> Switch<'a, Message> {
    Switch {
        chosen,
        contents: Vec::new(),
        stops: Vec::new(),
    }
}

/// Stops side by side in a track, each as wide as the widest, and a thumb at the chosen one.
pub struct Switch<'a, Message> {
    chosen: usize,
    /// What each stop shows, which is drawn in the colour of the thumb where the thumb is.
    contents: Vec<Element<'a, Message>>,
    stops: Vec<Stop<Message>>,
}

/// What a stop asks for when it is chosen, and pressed once it is, and the role of its colours.
struct Stop<Message> {
    on_choose: Message,
    on_press: Option<Message>,
    role: fn(&Scheme) -> Role,
}

impl<'a, Message> Switch<'a, Message> {
    /// A stop that shows `content`, asks for `on_choose` when pressed, and glows in `role`.
    pub fn stop(
        self,
        content: impl Widget<Message, Theme, Renderer> + 'a,
        on_choose: Message,
        role: fn(&Scheme) -> Role,
    ) -> Self {
        self.push(content, on_choose, None, role)
    }

    /// A stop as [`Self::stop`] whose thumb, once there, is a button that asks for `on_press`.
    pub fn button(
        self,
        content: impl Widget<Message, Theme, Renderer> + 'a,
        on_choose: Message,
        on_press: Message,
        role: fn(&Scheme) -> Role,
    ) -> Self {
        self.push(content, on_choose, Some(on_press), role)
    }

    /// Adds a stop, whose thumb is a button if there is something for a press on it to ask.
    fn push(
        mut self,
        content: impl Widget<Message, Theme, Renderer> + 'a,
        on_choose: Message,
        on_press: Option<Message>,
        role: fn(&Scheme) -> Role,
    ) -> Self {
        self.contents
            .push(content._boxed());
        self.stops
            .push(Stop {
                on_choose,
                on_press,
                role,
            });
        self
    }

    /// The room of the stop at `at`, which may lie between two as the thumb slides.
    fn cell(
        &self,
        bounds: Rectangle,
        at: f32,
    ) -> Rectangle {
        let width = (bounds.width - 2.0 * INSET) / self.stops.len() as f32;
        Rectangle {
            x: bounds.x + INSET + at * width,
            y: bounds.y + INSET,
            width,
            height: bounds.height - 2.0 * INSET,
        }
    }

    /// The stop under `cursor`, if it is over the switch.
    fn stop_under(
        &self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<usize> {
        let point = cursor.position_over(bounds)?;
        let width = self
            .cell(bounds, 0.0)
            .width;
        let at = ((point.x - bounds.x - INSET) / width).floor();
        Some((at.max(0.0) as usize).min(self.stops.len() - 1))
    }

    /// The fill of the thumb at a stop, what is written on it, and its glow.
    fn thumb(
        &self,
        scheme: &Scheme,
        stop: usize,
        layer: f32,
    ) -> [Color; 3] {
        let stop = &self.stops[stop];
        let role = (stop.role)(scheme);
        // as a button is filled, and a stop of a switch in the quieter container
        match stop.on_press {
            Some(_) => [
                over(role.color, role.on, layer),
                role.on,
                scheme.glow(role),
            ],
            None => [
                role.container,
                role.on_container,
                scheme.glow(role),
            ],
        }
    }
}

/// Where the thumb is shown, and what a press on the switch is doing.
struct State {
    /// The index of the stop that the thumb slides to, from where it was.
    slide: Animation<f32>,
    /// When the latest frame was, at which the slide is drawn.
    now: Instant,
    /// The stop a press went down on, and where across.
    pressed: Option<(usize, f32)>,
    /// How far the thumb is dragged from its stop.
    dragged: Option<f32>,
    /// The stop that was under the cursor when it last moved.
    lit: Option<usize>,
}

impl<Message> widget::Meta for Switch<'_, Message> {}

impl<Message: Clone> Widget<Message, Theme, Renderer> for Switch<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fixed(control::M))
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            slide: Animation::new(self.chosen as f32).duration(motion::SHORT),
            now: Instant::now(),
            pressed: None,
            dragged: None,
            lit: None,
        })
    }

    fn diff(
        &mut self,
        tree: &mut Tree,
    ) {
        tree.diff_children(&mut self.contents);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        _limits: &layout::Limits,
    ) {
        let height = control::M - 2.0 * INSET;
        // every stop is as wide as what the widest shows, and its content fills it
        let loose = layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, height));
        let widest = self
            .contents
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(content, tree)| {
                content.layout(tree, renderer, &loose);
                tree.size.width
            })
            .fold(0.0, f32::max);
        let cell = Size::new(widest + theme::BUTTON.x(), height);
        for (index, (content, tree)) in self
            .contents
            .iter_mut()
            .zip(&mut tree.children)
            .enumerate()
        {
            content.layout(tree, renderer, &layout::Limits::new(cell, cell));
            tree.size = cell;
            tree.translation = Vector::new(INSET + index as f32 * cell.width, INSET);
        }
        let stops = self.stops.len() as f32;
        tree.size = Size::new(stops * cell.width + 2.0 * INSET, control::M);
    }

    /// Tells of what each stop shows, which may be found by its text.
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            self.contents
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .for_each(|(content, (layout, tree))| {
                    content.operate(tree, layout, viewport, renderer, operation);
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
        let under = self.stop_under(bounds, cursor);
        // what the stops show sees nothing of a drag of the thumb, but the release that ends it
        let state = tree
            .state
            .downcast_ref::<State>();
        let is_release = matches!(event, Event::Mouse(mouse::Event::ButtonReleased(_)));
        if state
            .dragged
            .is_none()
            || is_release
        {
            self.contents
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .for_each(|(content, (layout, tree))| {
                    content.update(tree, event, layout, cursor, renderer, shell, viewport);
                });
        }

        let state = tree
            .state
            .downcast_mut::<State>();
        let chosen = self.chosen as f32;
        match event {
            Event::Window(window::Event::RedrawRequested(now)) => {
                state.now = *now;
                if state.slide.value() != chosen {
                    state
                        .slide
                        .go_mut(chosen, *now);
                }
                if state
                    .slide
                    .is_animating(*now)
                {
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let (Some(stop), Some(point)) = (under, cursor.position()) {
                    state.pressed = Some((stop, point.x));
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                // the thumb follows a press on it across, unless a stop took the drag
                if let Some((stop, from)) = state.pressed
                    && stop == self.chosen
                    && !shell.is_event_captured()
                {
                    let moved = position.x - from;
                    if state
                        .dragged
                        .is_some()
                        || moved.abs() >= START
                    {
                        state.dragged = Some(moved);
                        shell.capture_event();
                        shell.request_redraw();
                    }
                }
                if state.lit != under {
                    state.lit = under;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let Some((pressed, _)) = state.pressed.take() else {
                    return;
                };
                shell.request_redraw();
                // a dragged thumb goes to the stop nearest where it was let go, and slides there
                if let Some(moved) = state.dragged.take() {
                    let width = self
                        .cell(bounds, 0.0)
                        .width;
                    let last = (self.stops.len() - 1) as f32;
                    let dropped = (chosen + moved / width).clamp(0.0, last);
                    state.slide = Animation::new(dropped).duration(motion::SHORT);
                    let stop = dropped.round() as usize;
                    if stop != self.chosen {
                        shell.publish(
                            self.stops[stop]
                                .on_choose
                                .clone(),
                        );
                    }
                    return;
                }
                if under != Some(pressed) {
                    return;
                }
                let stop = &self.stops[pressed];
                let asked = match pressed == self.chosen {
                    true => stop
                        .on_press
                        .clone(),
                    false => Some(
                        stop.on_choose
                            .clone(),
                    ),
                };
                if let Some(asked) = asked {
                    shell.publish(asked);
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
        if state
            .dragged
            .is_some()
        {
            return mouse::Interaction::Grabbing;
        }
        let under = self.stop_under(layout.bounds(), cursor);
        match under {
            Some(stop)
                if stop != self.chosen
                    || self.stops[stop]
                        .on_press
                        .is_some() =>
            {
                mouse::Interaction::Pointer
            }
            // what a stop shows may say how it is dragged, as Delay does
            _ => self
                .contents
                .iter()
                .zip(layout.iter(&tree.children))
                .map(|(content, (layout, tree))| {
                    content.mouse_interaction(tree, layout, cursor, viewport, renderer)
                })
                .find(|interaction| *interaction != mouse::Interaction::None)
                .unwrap_or_default(),
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree
            .state
            .downcast_ref::<State>();
        let scheme = Scheme::of(theme);
        let under = self.stop_under(bounds, cursor);
        let last = (self.stops.len() - 1) as f32;
        let width = self
            .cell(bounds, 0.0)
            .width;
        let at = match state.dragged {
            Some(moved) => (self.chosen as f32 + moved / width).clamp(0.0, last),
            None => state
                .slide
                .interpolate(state.now),
        };
        let pill = |bounds| renderer::Quad {
            bounds,
            border: border::rounded(radius::FULL),
            ..renderer::Quad::default()
        };
        // how much a press or the cursor lies over what it is on
        let layer = |stop| match (under == Some(stop), state.pressed) {
            (true, Some((pressed, _))) if pressed == stop => opacity::PRESSED,
            (true, _) => opacity::HOVERED,
            (false, _) => 0.0,
        };

        renderer.fill_quad(pill(bounds), scheme.surface_container_highest);
        // a stop that the thumb is not at is lit under the cursor, as a button would be
        if let Some(stop) = under.filter(|stop| {
            *stop != self.chosen
                && state
                    .dragged
                    .is_none()
        }) {
            let lit = over(
                scheme.surface_container_highest,
                scheme.on_surface,
                layer(stop),
            );
            renderer.fill_quad(pill(self.cell(bounds, stop as f32)), lit);
        }
        // a stop that is a button is outlined as one while the thumb is elsewhere
        for (index, stop) in self
            .stops
            .iter()
            .enumerate()
            .filter(|(index, stop)| {
                *index != self.chosen
                    && stop
                        .on_press
                        .is_some()
            })
        {
            let line = (stop.role)(scheme)
                .color
                .scale_alpha(opacity::DISABLED);
            let outline = renderer::Quad {
                border: border::rounded(radius::FULL)
                    .color(line)
                    .width(stroke::THIN),
                ..pill(self.cell(bounds, index as f32))
            };
            renderer.fill_quad(outline, Color::TRANSPARENT);
        }

        // between two stops the thumb is of both their colours, as far as it is from each
        let (from, to) = (at.floor() as usize, at.ceil() as usize);
        let [fill_from, _, glow_from] = self.thumb(scheme, from, layer(from));
        let [fill_to, _, glow_to] = self.thumb(scheme, to, layer(to));
        let toward = at.fract();
        let thumb = renderer::Quad {
            shadow: theme::halo(over(glow_from, glow_to, toward)),
            ..pill(self.cell(bounds, at))
        };
        renderer.fill_quad(thumb, over(fill_from, fill_to, toward));

        for (stop, (content, (layout, tree))) in self
            .contents
            .iter()
            .zip(layout.iter(&tree.children))
            .enumerate()
        {
            let quiet = match under == Some(stop) {
                true => scheme.on_surface,
                false => scheme.on_surface_variant,
            };
            let [_, on_thumb, _] = self.thumb(scheme, stop, 0.0);
            let covered = (1.0 - (at - stop as f32).abs()).max(0.0);
            let written = renderer::Style {
                text_color: over(quiet, on_thumb, covered),
            };
            content.draw(tree, renderer, theme, &written, layout, cursor, viewport);
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
        self.contents
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
            .flat_map(|(content, (layout, tree))| {
                content.overlay(tree, layout, renderer, viewport, translation, window)
            })
            .collect()
    }
}
