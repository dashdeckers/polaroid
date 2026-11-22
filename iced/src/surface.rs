//! A surface to pan and zoom, with keyed cards to drag about on it and wires between their ports.

use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Overlay, Renderer as _, Shell, Widget, mouse, overlay, renderer};
use iced::mouse::ScrollDelta;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{
    Color,
    Element,
    Event,
    Fill,
    Length,
    Point,
    Rectangle,
    Renderer,
    Size,
    Theme,
    Transformation,
    Vector,
    border,
};

/// How much one step of the wheel zooms.
const STEP: f32 = 1.1;
/// How far out and in a surface zooms.
const ZOOM: (f32, f32) = (0.25, 2.5);
/// The radius of a port; a press twice as far from its middle still takes it.
const PORT: f32 = 5.0;
/// The corner of a card that resizes it.
const GRIP: f32 = 14.0;

/// An empty surface.
pub fn surface<'a, Key, Message>() -> Surface<'a, Key, Message> {
    Surface {
        cards: Vec::new(),
        wires: Vec::new(),
        on_move: None,
        on_resize: None,
        on_wire: None,
        on_unwire: None,
        on_menu: None,
        on_open: None,
    }
}

/// A card at a point of a surface, as large as its content, which must not fill.
pub fn card<'a, Key, Message>(
    key: Key,
    at: Point,
    content: impl Widget<Message, Theme, Renderer> + 'a,
) -> Card<'a, Key, Message> {
    Card {
        key,
        at: at - Point::ORIGIN,
        content: content._boxed(),
        inputs: Vec::new(),
        output: None,
        grows: (false, false),
        handle: None,
        is_loose: true,
    }
}

/// A card of a surface, and the ports on its edges.
pub struct Card<'a, Key, Message> {
    key: Key,
    at: Vector,
    content: Element<'a, Message>,
    inputs: Vec<f32>,
    output: Option<f32>,
    /// Whether its grip makes it wider, and taller.
    grows: (bool, bool),
    handle: Option<Rectangle>,
    /// Whether a press that nothing in it takes drags it, and a double one opens it.
    is_loose: bool,
}

impl<Key, Message> Card<'_, Key, Message> {
    /// A port on its left edge for each height below its top, to end wires at.
    pub fn inputs(
        mut self,
        inputs: impl IntoIterator<Item = f32>,
    ) -> Self {
        self.inputs = inputs
            .into_iter()
            .collect();
        self
    }

    /// A port on its right edge this far below its top, to start wires from.
    pub fn output(
        mut self,
        output: f32,
    ) -> Self {
        self.output = Some(output);
        self
    }

    /// Whether a press that nothing in it takes drags it, and a double one opens it; it is so.
    pub fn loose(
        mut self,
        is_loose: bool,
    ) -> Self {
        self.is_loose = is_loose;
        self
    }

    /// A part of it, from its corner, that drags it whatever else does.
    pub fn handle(
        mut self,
        handle: Rectangle,
    ) -> Self {
        self.handle = Some(handle);
        self
    }

    /// Whether its lower right corner is a grip that makes it wider, and taller.
    pub fn resizable(
        mut self,
        wider: bool,
        taller: bool,
    ) -> Self {
        self.grows = (wider, taller);
        self
    }
}

/// Cards at their points, the last on top; what a card keeps follows its key.
pub struct Surface<'a, Key, Message> {
    cards: Vec<Card<'a, Key, Message>>,
    /// From the output of a card to an input of another, by its slot.
    wires: Vec<(Key, Key, usize)>,
    on_move: Option<Box<dyn Fn(Key, Point) -> Message + 'a>>,
    on_resize: Option<Box<dyn Fn(Key, Vector) -> Message + 'a>>,
    on_wire: Option<OnWire<'a, Key, Message>>,
    on_unwire: Option<Box<dyn Fn(Key, usize) -> Message + 'a>>,
    on_menu: Option<Box<dyn Fn(Point, Point) -> Message + 'a>>,
    on_open: Option<Box<dyn Fn(Key) -> Message + 'a>>,
}

impl<'a, Key, Message> Surface<'a, Key, Message> {
    /// Adds a card on top.
    pub fn push(
        mut self,
        card: Card<'a, Key, Message>,
    ) -> Self {
        self.cards
            .push(card);
        self
    }

    /// Draws a wire from the output of a card to the input `slot` of another.
    pub fn wire(
        mut self,
        from: Key,
        to: Key,
        slot: usize,
    ) -> Self {
        self.wires
            .push((from, to, slot));
        self
    }

    /// The message for a card dragged to a point; without it cards stay put.
    pub fn on_move(
        mut self,
        on_move: impl Fn(Key, Point) -> Message + 'a,
    ) -> Self {
        self.on_move = Some(Box::new(on_move));
        self
    }

    /// The message for a card whose grip was dragged this much further.
    pub fn on_resize(
        mut self,
        on_resize: impl Fn(Key, Vector) -> Message + 'a,
    ) -> Self {
        self.on_resize = Some(Box::new(on_resize));
        self
    }

    /// The message for a wire dragged from the output of a card to an input of another.
    pub fn on_wire(
        mut self,
        on_wire: impl Fn(Key, Key, usize) -> Message + 'a,
    ) -> Self {
        self.on_wire = Some(Box::new(on_wire));
        self
    }

    /// The message for a right press on an input of a card.
    pub fn on_unwire(
        mut self,
        on_unwire: impl Fn(Key, usize) -> Message + 'a,
    ) -> Self {
        self.on_unwire = Some(Box::new(on_unwire));
        self
    }

    /// The message for a right press on no card: where in the surface, and where among the cards.
    pub fn on_menu(
        mut self,
        on_menu: impl Fn(Point, Point) -> Message + 'a,
    ) -> Self {
        self.on_menu = Some(Box::new(on_menu));
        self
    }

    /// The message for a double press on a loose card that nothing in it takes.
    pub fn on_open(
        mut self,
        on_open: impl Fn(Key) -> Message + 'a,
    ) -> Self {
        self.on_open = Some(Box::new(on_open));
        self
    }
}

/// What makes the message for a wire from a card to an input of another.
type OnWire<'a, Key, Message> = Box<dyn Fn(Key, Key, usize) -> Message + 'a>;

/// What a surface keeps: its view, the drag under way, and whose state each child tree is.
struct State<Key> {
    keys: Vec<Key>,
    pan: Vector,
    scale: f32,
    drag: Option<Drag<Key>>,
    /// The latest press on a loose card, which tells a double one.
    pressed: Option<mouse::Click>,
}

enum Drag<Key> {
    /// The view, from where the cursor last was.
    Pan(Point),
    /// A card, held this far from its corner.
    Card(Key, Vector),
    /// The grip of a card, from where the cursor last was, and whether it grows each way.
    Grip(Key, Point, (bool, bool)),
    /// A wire from the output of a card to where the cursor is, as the cards see it.
    Wire(Key, Point),
}

/// What of the cards is at a point, by the index of the card.
#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Input(usize, usize),
    Output(usize),
    Grip(usize),
    Handle(usize),
    Card(usize),
}

/// Where the cards are on screen: a card's bounds times `scale` are its pixels.
#[derive(Clone, Copy)]
struct View {
    scale: Transformation,
    /// Where the origin of the surface lies, before scaling.
    origin: Vector,
}

impl View {
    fn new<Key>(
        state: &State<Key>,
        bounds: Rectangle,
    ) -> Self {
        let scale = Transformation::scale(state.scale);
        let origin = bounds.position() + state.pan - Point::ORIGIN;
        Self {
            scale,
            origin: origin * scale.inverse(),
        }
    }

    /// Takes a point or rectangle of the screen to where the cards see it.
    fn inverse(&self) -> Transformation {
        self.scale.inverse()
    }

    fn layout(
        &self,
        at: Vector,
        tree: &Tree,
    ) -> Layout {
        Layout::new(tree.size).move_to(Point::ORIGIN + self.origin + at)
    }
}

impl<'a, Key: PartialEq, Message> Surface<'a, Key, Message> {
    /// Each card with its tree and its layout, from the bottom one up.
    fn laid_out<'b>(
        &'b self,
        tree: &'b Tree,
        view: View,
    ) -> impl DoubleEndedIterator<Item = (usize, &'b Card<'a, Key, Message>, &'b Tree, Layout)>
    {
        self.cards
            .iter()
            .zip(&tree.children)
            .enumerate()
            .map(move |(index, (card, tree))| (index, card, tree, view.layout(card.at, tree)))
    }

    /// The top thing under a cursor that the cards see: a port before its card.
    fn hit(
        &self,
        tree: &Tree,
        view: View,
        cursor: mouse::Cursor,
    ) -> Option<Hit> {
        let point = cursor.position()?;
        self.laid_out(tree, view)
            .rev()
            .find_map(|(index, card, _, layout)| {
                let bounds = layout.bounds();
                let is_near = |port: Point| port.distance(point) <= 2.0 * PORT;
                let input = card
                    .inputs
                    .iter()
                    .position(|y| is_near(Point::new(bounds.x, bounds.y + y)));
                let output = card
                    .output
                    .filter(|y| is_near(Point::new(bounds.x + bounds.width, bounds.y + y)));
                let grip = Rectangle::new(
                    Point::new(
                        bounds.x + bounds.width - GRIP,
                        bounds.y + bounds.height - GRIP,
                    ),
                    Size::new(GRIP, GRIP),
                );
                let handle = card
                    .handle
                    .map(|handle| handle + (bounds.position() - Point::ORIGIN));
                match (input, output) {
                    (Some(slot), _) => Some(Hit::Input(index, slot)),
                    (None, Some(_)) => Some(Hit::Output(index)),
                    _ if card.grows != (false, false) && grip.contains(point) => {
                        Some(Hit::Grip(index))
                    }
                    _ if handle.is_some_and(|handle| handle.contains(point)) => {
                        Some(Hit::Handle(index))
                    }
                    _ => bounds
                        .contains(point)
                        .then_some(Hit::Card(index)),
                }
            })
    }

    /// Where the wire of an input `slot` of the card of a key ends, or that of its output starts.
    fn port(
        &self,
        tree: &Tree,
        view: View,
        key: &Key,
        slot: Option<usize>,
    ) -> Option<Point> {
        let (_, card, _, layout) = self
            .laid_out(tree, view)
            .find(|(_, card, ..)| card.key == *key)?;
        let bounds = layout.bounds();
        match slot {
            Some(slot) => {
                let y = card
                    .inputs
                    .get(slot)?;
                Some(Point::new(bounds.x, bounds.y + y))
            }
            None => Some(Point::new(bounds.x + bounds.width, bounds.y + card.output?)),
        }
    }
}

impl<Key, Message> widget::Meta for Surface<'_, Key, Message> {}

impl<Key, Message> Widget<Message, Theme, Renderer> for Surface<'_, Key, Message>
where
    Key: Clone + PartialEq + 'static,
{
    fn size(&self) -> Size<Length> {
        Size::new(Fill, Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Key>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Key> {
            keys: Vec::new(),
            pan: Vector::ZERO,
            scale: 1.0,
            drag: None,
            pressed: None,
        })
    }

    fn diff(
        &mut self,
        tree: &mut Tree,
    ) {
        let state = tree
            .state
            .downcast_mut::<State<Key>>();
        let mut old: Vec<(Key, Tree)> = state
            .keys
            .drain(..)
            .zip(
                tree.children
                    .drain(..),
            )
            .collect();
        for card in &mut self.cards {
            // a card keeps its state wherever it is in the order
            let kept = old
                .iter()
                .position(|(key, _)| *key == card.key)
                .map(|index| {
                    old.swap_remove(index)
                        .1
                });
            let mut child = kept.unwrap_or_else(|| Tree::new(&card.content));
            child.diff(&mut card.content);
            tree.children
                .push(child);
            state
                .keys
                .push(card.key.clone());
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) {
        tree.size = limits.resolve(Fill, Fill, Size::ZERO);
        let loose = layout::Limits::new(Size::ZERO, tree.size);
        for (card, tree) in self
            .cards
            .iter_mut()
            .zip(&mut tree.children)
        {
            card.content
                .layout(tree, renderer, &loose);
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let state = tree
            .state
            .downcast_ref::<State<Key>>();
        let view = View::new(state, layout.bounds());
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            for (card, tree) in self
                .cards
                .iter_mut()
                .zip(&mut tree.children)
            {
                let layout = view.layout(card.at, tree);
                card.content
                    .operate(tree, layout, viewport, renderer, operation);
            }
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
        let state = tree
            .state
            .downcast_ref::<State<Key>>();
        let view = View::new(state, bounds);
        let local = cursor * view.inverse();
        let hit = self
            .hit(tree, view, local)
            .filter(|_| cursor.is_over(bounds));
        // only the top card under the cursor is under it, and none during a drag
        let is_free = state.drag.is_none();

        for (index, (card, tree)) in self
            .cards
            .iter_mut()
            .zip(&mut tree.children)
            .enumerate()
        {
            let cursor = match is_free && hit == Some(Hit::Card(index)) {
                true => local,
                false => local.levitate(),
            };
            let layout = view.layout(card.at, tree);
            let viewport = *viewport * view.inverse();
            card.content
                .update(tree, event, layout, cursor, renderer, shell, &viewport);
        }

        let Event::Mouse(event) = event else {
            return;
        };
        let key = |index: usize| {
            self.cards[index]
                .key
                .clone()
        };
        let state = tree
            .state
            .downcast_mut::<State<Key>>();
        match event {
            // a drag ends wherever the cursor is, and whoever took the release
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                let drag = state.drag.take();
                if let (Some(Drag::Wire(from, _)), Some(Hit::Input(to, slot))) = (drag, hit)
                    && let Some(on_wire) = &self.on_wire
                {
                    shell.publish(on_wire(from, key(to), slot));
                }
                shell.request_redraw();
            }
            _ if shell.is_event_captured() => {}
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let (Some(at), Some(point)) = (cursor.position_over(bounds), local.position())
                else {
                    return;
                };
                state.drag = Some(match hit {
                    Some(Hit::Output(index)) => Drag::Wire(key(index), point),
                    Some(Hit::Grip(index)) => Drag::Grip(key(index), at, self.cards[index].grows),
                    // a card that is not loose is dragged by its handle alone
                    Some(Hit::Card(index) | Hit::Input(index, _))
                        if !self.cards[index].is_loose =>
                    {
                        return;
                    }
                    Some(Hit::Card(index)) if is_double(&mut state.pressed, at) => {
                        if let Some(on_open) = &self.on_open {
                            shell.publish(on_open(key(index)));
                        }
                        shell.capture_event();
                        return;
                    }
                    Some(Hit::Handle(index) | Hit::Card(index) | Hit::Input(index, _)) => {
                        let corner = Point::ORIGIN + view.origin + self.cards[index].at;
                        Drag::Card(key(index), point - corner)
                    }
                    None => Drag::Pan(at),
                });
                shell.capture_event();
            }
            mouse::Event::ButtonPressed(mouse::Button::Right) => {
                let (Some(at), Some(point)) = (cursor.position_in(bounds), local.position()) else {
                    return;
                };
                let message = match hit {
                    Some(Hit::Input(index, slot)) => self
                        .on_unwire
                        .as_ref()
                        .map(|on_unwire| on_unwire(key(index), slot)),
                    Some(_) => None,
                    None => self
                        .on_menu
                        .as_ref()
                        .map(|on_menu| on_menu(at, point - view.origin)),
                };
                if let Some(message) = message {
                    shell.publish(message);
                    shell.capture_event();
                }
            }
            mouse::Event::CursorMoved { position } => {
                let point = *position * view.inverse();
                match &mut state.drag {
                    Some(Drag::Pan(from)) => {
                        state.pan += *position - *from;
                        *from = *position;
                    }
                    Some(Drag::Card(key, held)) => {
                        if let Some(on_move) = &self.on_move {
                            shell.publish(on_move(key.clone(), point - view.origin - *held));
                        }
                    }
                    Some(Drag::Grip(key, from, (wider, taller))) => {
                        let Vector { x, y } = (*position - *from) * view.inverse();
                        let grown = Vector::new(
                            x * f32::from(u8::from(*wider)),
                            y * f32::from(u8::from(*taller)),
                        );
                        *from = *position;
                        if let Some(on_resize) = &self.on_resize {
                            shell.publish(on_resize(key.clone(), grown));
                        }
                    }
                    Some(Drag::Wire(_, end)) => *end = point,
                    None => return,
                }
                shell.request_redraw();
            }
            mouse::Event::WheelScrolled { delta } => {
                let Some(at) = cursor.position_over(bounds) else {
                    return;
                };
                let steps = match *delta {
                    ScrollDelta::Lines { y, .. } => y,
                    ScrollDelta::Pixels { y, .. } => y / 50.0,
                };
                let scale = (state.scale * STEP.powf(steps)).clamp(ZOOM.0, ZOOM.1);
                // what is under the cursor stays under it
                let held = at - bounds.position();
                state.pan = held - (held - state.pan) * (scale / state.scale);
                state.scale = scale;
                shell.capture_event();
                shell.request_redraw();
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
            .downcast_ref::<State<Key>>();
        let view = View::new(state, layout.bounds());
        let local = cursor * view.inverse();
        let hit = self
            .hit(tree, view, local)
            .filter(|_| cursor.is_over(layout.bounds()));
        match (&state.drag, hit) {
            (Some(Drag::Grip(.., grows)), _) => resizing(*grows),
            (None, Some(Hit::Grip(index))) => resizing(self.cards[index].grows),
            (Some(Drag::Wire(..)), _) | (None, Some(Hit::Input(..) | Hit::Output(_))) => {
                mouse::Interaction::Crosshair
            }
            (Some(_), _) => mouse::Interaction::Grabbing,
            (None, Some(Hit::Handle(_))) => mouse::Interaction::Grab,
            (None, Some(Hit::Card(index))) => {
                let tree = &tree.children[index];
                let card = &self.cards[index];
                let layout = view.layout(card.at, tree);
                let viewport = *viewport * view.inverse();
                let inner = card
                    .content
                    .mouse_interaction(tree, layout, local, &viewport, renderer);
                match inner {
                    mouse::Interaction::None if card.is_loose => mouse::Interaction::Grab,
                    inner => inner,
                }
            }
            (None, None) => mouse::Interaction::None,
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
        let bounds = layout.bounds();
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };
        let state = tree
            .state
            .downcast_ref::<State<Key>>();
        let view = View::new(state, bounds);
        let local = cursor * view.inverse();
        let is_free = state.drag.is_none();
        let hit = self
            .hit(tree, view, local)
            .filter(|_| is_free && cursor.is_over(bounds));
        let seen = clip * view.inverse();
        let palette = theme.palette();
        let muted = palette
            .background
            .strongest
            .color;

        let mut wires = Frame::new(renderer, seen.size());
        wires.translate(Point::ORIGIN - seen.position());
        for (from, to, slot) in &self.wires {
            let ends = self
                .port(tree, view, from, None)
                .zip(self.port(tree, view, to, Some(*slot)));
            if let Some((from, to)) = ends {
                wire(&mut wires, from, to, muted);
            }
        }
        if let Some(Drag::Wire(from, to)) = &state.drag
            && let Some(from) = self.port(tree, view, from, None)
        {
            let active = palette
                .primary
                .base
                .color;
            wire(&mut wires, from, *to, active);
        }

        renderer.with_layer(clip, |renderer| {
            renderer.with_transformation(view.scale, |renderer| {
                renderer.with_translation(seen.position() - Point::ORIGIN, |renderer| {
                    renderer.draw_geometry(wires.into_geometry());
                });
                for (index, card, tree, layout) in self.laid_out(tree, view) {
                    let cursor = match hit == Some(Hit::Card(index)) {
                        true => local,
                        false => local.levitate(),
                    };
                    // a layer each, as the text of a layer is over all its shapes
                    renderer.with_layer(seen, |renderer| {
                        card.content
                            .draw(tree, renderer, theme, style, layout, cursor, &seen);
                        let bounds = layout.bounds();
                        let inputs = card
                            .inputs
                            .iter()
                            .map(|y| Point::new(bounds.x, bounds.y + y));
                        let output = card
                            .output
                            .map(|y| Point::new(bounds.x + bounds.width, bounds.y + y));
                        for port in inputs.chain(output) {
                            let dot = Rectangle::new(port, Size::ZERO).expand(PORT);
                            renderer.fill_quad(quad(dot, PORT), muted);
                        }
                        if let Some(handle) = card.handle {
                            // six dots in its middle
                            let middle = bounds.position() + (handle.center() - Point::ORIGIN);
                            for (x, y) in [-2.5, 2.5]
                                .into_iter()
                                .flat_map(|x| [-5.0, 0.0, 5.0].map(|y| (x, y)))
                            {
                                let dot = Rectangle::new(middle + Vector::new(x, y), Size::ZERO);
                                renderer.fill_quad(quad(dot.expand(1.5), 1.5), muted);
                            }
                        }
                        if card.grows != (false, false) {
                            // three dots along the corner
                            for (x, y) in [
                                (8.0, 8.0),
                                (13.0, 8.0),
                                (8.0, 13.0),
                            ] {
                                let corner = Point::new(
                                    bounds.x + bounds.width - x,
                                    bounds.y + bounds.height - y,
                                );
                                let dot = Rectangle::new(corner, Size::ZERO).expand(1.5);
                                renderer.fill_quad(quad(dot, 1.5), muted);
                            }
                        }
                    });
                }
            });
        });
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
        let state = tree
            .state
            .downcast_ref::<State<Key>>();
        let view = View::new(state, layout.bounds() + translation);
        let seen = *viewport * view.inverse();
        // the cards see a window from the origin, so what opens stays inside it
        let window = window * view.inverse();
        self.cards
            .iter_mut()
            .zip(&mut tree.children)
            .flat_map(|(card, tree)| {
                let layout = view.layout(card.at, tree);
                card.content
                    .overlay(tree, layout, renderer, &seen, Vector::ZERO, window)
            })
            .map(|inner| scaled(inner, view.scale))
            .collect()
    }
}

/// The cursor over a grip that makes a card wider, and taller.
fn resizing(grows: (bool, bool)) -> mouse::Interaction {
    match grows {
        (true, false) => mouse::Interaction::ResizingHorizontally,
        (false, true) => mouse::Interaction::ResizingVertically,
        _ => mouse::Interaction::ResizingDiagonallyDown,
    }
}

/// Notes a left press at a point, and tells whether it is the second of a double one.
fn is_double(
    pressed: &mut Option<mouse::Click>,
    at: Point,
) -> bool {
    let click = mouse::Click::new(at, mouse::Button::Left, *pressed);
    *pressed = Some(click);
    click.kind() == mouse::click::Kind::Double
}

/// A rounded quad.
fn quad(
    bounds: Rectangle,
    radius: f32,
) -> renderer::Quad {
    renderer::Quad {
        bounds,
        border: border::rounded(radius),
        ..renderer::Quad::default()
    }
}

/// Draws a wire that leaves and enters level.
fn wire(
    frame: &mut Frame,
    from: Point,
    to: Point,
    color: Color,
) {
    let bend = Vector::new(((to.x - from.x).abs() / 2.0).max(40.0), 0.0);
    let path = Path::new(|path| {
        path.move_to(from);
        path.bezier_curve_to(from + bend, to - bend, to);
    });
    let stroke = Stroke::default()
        .with_color(color)
        .with_width(2.0);
    frame.stroke(&path, stroke);
}

/// What a card opens over the window, zoomed as its card is.
struct Scaled<'a, Message> {
    inner: overlay::Element<'a, Message, Theme, Renderer>,
    scale: Transformation,
}

fn scaled<'a, Message: 'a>(
    inner: overlay::Element<'a, Message, Theme, Renderer>,
    scale: Transformation,
) -> overlay::Element<'a, Message, Theme, Renderer> {
    overlay::Element::new(Box::new(Scaled { inner, scale }))
}

impl<Message> Overlay<Message, Theme, Renderer> for Scaled<'_, Message> {
    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: mouse::Cursor,
    ) {
        let inverse = self.scale.inverse();
        renderer.with_transformation(self.scale, |renderer| {
            self.inner
                .as_overlay()
                .draw(renderer, theme, style, cursor * inverse);
        });
    }

    fn operate(
        &mut self,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.inner
            .as_overlay_mut()
            .operate(renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let inverse = self.scale.inverse();
        self.inner
            .as_overlay_mut()
            .update(event, cursor * inverse, renderer, shell);
    }

    fn mouse_interaction(
        &self,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let inverse = self.scale.inverse();
        self.inner
            .as_overlay()
            .mouse_interaction(cursor * inverse, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        renderer: &Renderer,
    ) -> Vec<overlay::Element<'a, Message, Theme, Renderer>> {
        self.inner
            .as_overlay_mut()
            .overlay(renderer)
            .into_iter()
            .map(|inner| scaled(inner, self.scale))
            .collect()
    }

    fn index(&self) -> f32 {
        self.inner
            .as_overlay()
            .index()
    }
}
