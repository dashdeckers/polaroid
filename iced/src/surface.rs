//! A surface to pan and zoom, with keyed cards to drag about on it and wires between their ports.

use std::time::Duration;

use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Overlay, Renderer as _, Shell, Widget, mouse, overlay, renderer};
use iced::mouse::ScrollDelta;
use iced::time::Instant;
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
    window,
};

use crate::seen::Seen;
use crate::theme::{self, HALO, Scheme, motion, spacing, stroke};

/// How much one step of the wheel zooms.
const STEP: f32 = theme::surface::ZOOM_STEP;
/// How far out and in a surface zooms.
const ZOOM: (f32, f32) = theme::surface::ZOOM;
/// How long after the last step of the wheel a zoom is settled, and is told.
const SETTLE: Duration = motion::SHORT;
/// The radius of a port; a press twice as far from its middle still takes it.
const PORT: f32 = theme::surface::PORT;
/// The corner of a card that resizes it.
const GRIP: f32 = theme::surface::GRIP;
/// The radius of a dot of that corner, and how far in from its edges the dots lie.
const SPECK: f32 = theme::surface::SPECK;
const NEAR: f32 = spacing::S;
const FAR: f32 = NEAR + PORT;
/// How much of the smaller of two cards lies on the other for a drop to swap them.
const OVERLAP: f32 = theme::surface::OVERLAP;
/// How long a dragged card lies on another before a drop swaps them, which a halo then shows.
const HOVER: Duration = motion::MEDIUM;
/// How long the cursor rests on a card that peeks before it is told to open.
const REST: Duration = motion::LONG;

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
        on_peek: None,
        on_swap: None,
        on_zoom: None,
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
        anchors: Vec::new(),
        output: None,
        is_resizable: false,
        tall: None,
        handle: None,
        peeks: false,
    }
}

/// A card of a surface, and the ports on its edges.
pub struct Card<'a, Key, Message> {
    key: Key,
    at: Vector,
    content: Element<'a, Message>,
    /// How far below its top each input port is, as of the latest layout.
    inputs: Vec<f32>,
    /// The ids of the containers in it that its input ports are level with.
    anchors: Vec<widget::Id>,
    output: Option<f32>,
    is_resizable: bool,
    /// The height it was made, which what it holds may exceed.
    tall: Option<f32>,
    /// The part of it where a press that nothing takes drags it, and a double one opens it.
    handle: Option<Rectangle>,
    /// Whether it is told to open once the cursor rested on it, until the cursor leaves it.
    peeks: bool,
}

impl<Key, Message> Card<'_, Key, Message> {
    /// A port on its left edge level with each container of these ids in it, to end wires at.
    pub fn ports(
        mut self,
        ports: impl IntoIterator<Item = widget::Id>,
    ) -> Self {
        self.anchors = ports
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

    /// The part of it, from its corner, that drags and opens it; all of it does unless told.
    pub fn handle(
        mut self,
        handle: Rectangle,
    ) -> Self {
        self.handle = Some(handle);
        self
    }

    /// Whether the cursor resting on it a while has it told to open until the cursor leaves it.
    pub fn peek(
        mut self,
        peeks: bool,
    ) -> Self {
        self.peeks = peeks;
        self
    }

    /// Makes it this tall if what it holds is less, whose fills in height then take the rest.
    pub fn tall(
        mut self,
        tall: f32,
    ) -> Self {
        self.tall = Some(tall);
        self
    }

    /// Whether its lower right corner is a grip to resize it by.
    pub fn resizable(
        mut self,
        is_resizable: bool,
    ) -> Self {
        self.is_resizable = is_resizable;
        self
    }
}

/// Cards at their points, the last pressed on top; what a card keeps follows its key.
pub struct Surface<'a, Key, Message> {
    cards: Vec<Card<'a, Key, Message>>,
    /// From the output of a card to an input of another, by its slot.
    wires: Vec<(Key, Key, usize)>,
    on_move: Option<Box<dyn Fn(Key, Point) -> Message + 'a>>,
    on_resize: Option<Box<dyn Fn(Key, Size) -> Message + 'a>>,
    on_wire: Option<OnWire<'a, Key, Message>>,
    on_unwire: Option<Box<dyn Fn(Key, usize) -> Message + 'a>>,
    on_menu: Option<Box<dyn Fn(Point, Point) -> Message + 'a>>,
    on_open: Option<Box<dyn Fn(Key) -> Message + 'a>>,
    on_peek: Option<Box<dyn Fn(Option<Key>) -> Message + 'a>>,
    on_swap: Option<OnSwap<'a, Key, Message>>,
    on_zoom: Option<Box<dyn Fn(f32) -> Message + 'a>>,
}

impl<'a, Key, Message> Surface<'a, Key, Message> {
    /// Adds a card, on top of those before it until one of them is pressed.
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

    /// The message for a card whose grip was dragged to make it this size.
    pub fn on_resize(
        mut self,
        on_resize: impl Fn(Key, Size) -> Message + 'a,
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

    /// The message for a double press on the handle of a card that nothing in it takes.
    pub fn on_open(
        mut self,
        on_open: impl Fn(Key) -> Message + 'a,
    ) -> Self {
        self.on_open = Some(Box::new(on_open));
        self
    }

    /// The message for a card that peeks once the cursor rested on it, and for none as it leaves.
    pub fn on_peek(
        mut self,
        on_peek: impl Fn(Option<Key>) -> Message + 'a,
    ) -> Self {
        self.on_peek = Some(Box::new(on_peek));
        self
    }
}

impl<'a, Key, Message> Surface<'a, Key, Message> {
    /// The message for a card dropped on another after it lay there, if they can swap places.
    ///
    /// It is given the card that was dragged, the one under it, and where the drag started from;
    /// if there is none to give, the card goes back there.
    pub fn on_swap(
        mut self,
        on_swap: impl Fn(Key, Key, Point) -> Option<Message> + 'a,
    ) -> Self {
        self.on_swap = Some(Box::new(on_swap));
        self
    }
}

impl<'a, Key, Message> Surface<'a, Key, Message> {
    /// The message for how many times their size the cards are drawn, once a zoom has settled.
    ///
    /// A card that draws for the pixels of the screen, as a plot does, is to draw for this many more.
    pub fn on_zoom(
        mut self,
        on_zoom: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        self.on_zoom = Some(Box::new(on_zoom));
        self
    }
}

/// What makes the message for a card dropped on another, if there is one to make.
type OnSwap<'a, Key, Message> = Box<dyn Fn(Key, Key, Point) -> Option<Message> + 'a>;

/// What makes the message for a wire from a card to an input of another.
type OnWire<'a, Key, Message> = Box<dyn Fn(Key, Key, usize) -> Message + 'a>;

/// What a surface keeps: its view, the drag under way, and whose state each child tree is.
struct State<Key> {
    keys: Vec<Key>,
    pan: Vector,
    scale: f32,
    drag: Option<Drag<Key>>,
    /// The latest press on the handle of a card, which tells a double one.
    pressed: Option<mouse::Click>,
    /// The card that the dragged one lies on.
    swap: Option<Swap<Key>>,
    /// When the zoom is settled, if it is not told yet.
    settles: Option<Instant>,
    /// The card that the cursor is on, and what it waits for there.
    rest: Option<Rest<Key>>,
}

/// A card that the cursor is on, and what it waits for there.
struct Rest<Key> {
    key: Key,
    wait: Wait,
}

/// What a card that the cursor is on waits for.
#[derive(Clone, Copy, PartialEq)]
enum Wait {
    /// The cursor to have rested on it until then, when it is told to open if it peeks.
    Until(Instant),
    /// The cursor to leave it, which it is told as it was told to open.
    Told,
    /// The cursor to leave it: it was pressed, and is not told to open before.
    Spent,
}

/// A card that the dragged one lies on, which a drop swaps it with once it lay there a while.
struct Swap<Key> {
    target: Key,
    /// The first frame it lay there.
    since: Option<Instant>,
    is_armed: bool,
    /// Whether there is a message for the swap, or else the drag is undone.
    is_possible: bool,
}

enum Drag<Key> {
    /// The view, from where the cursor last was.
    Pan(Point),
    /// A card, held this far from its corner, and where it was.
    Card(Key, Vector, Point),
    /// The grip of a card, held this far from its lower right corner.
    Grip(Key, Vector),
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

impl Hit {
    /// The index of the card that it is of.
    fn card(self) -> usize {
        match self {
            Self::Input(index, _)
            | Self::Output(index)
            | Self::Grip(index)
            | Self::Handle(index)
            | Self::Card(index) => index,
        }
    }

    /// Whether it is on what the card of this index holds, which the cursor is then over.
    fn is_on(
        hit: Option<Self>,
        index: usize,
    ) -> bool {
        matches!(hit, Some(Self::Handle(on) | Self::Card(on)) if on == index)
    }
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
    /// Brings the card of an index to the top, over all the others.
    fn raise(
        &mut self,
        tree: &mut Tree,
        index: usize,
    ) where
        Key: 'static,
    {
        self.cards[index..].rotate_left(1);
        tree.children[index..].rotate_left(1);
        let state = tree
            .state
            .downcast_mut::<State<Key>>();
        state.keys[index..].rotate_left(1);
    }

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
                    .map_or(bounds, |handle| {
                        handle + (bounds.position() - Point::ORIGIN)
                    });
                match (input, output) {
                    (Some(slot), _) => Some(Hit::Input(index, slot)),
                    (None, Some(_)) => Some(Hit::Output(index)),
                    _ if card.is_resizable && grip.contains(point) => Some(Hit::Grip(index)),
                    _ if handle.contains(point) => Some(Hit::Handle(index)),
                    _ => bounds
                        .contains(point)
                        .then_some(Hit::Card(index)),
                }
            })
    }

    /// The card that most of the card of a key lies on, or most of which lies under it, at `at`.
    fn under(
        &self,
        children: &[Tree],
        view: View,
        key: &Key,
        at: Point,
    ) -> Option<&Key> {
        let cards = || {
            self.cards
                .iter()
                .zip(children)
        };
        let area = |size: Size| size.width * size.height;
        let (_, dragged) = cards().find(|(card, _)| card.key == *key)?;
        let dragged = Rectangle::new(at + view.origin, dragged.size);
        cards()
            .filter(|(card, _)| card.key != *key)
            .filter_map(|(card, tree)| {
                let bounds = view
                    .layout(card.at, tree)
                    .bounds();
                let shared = dragged.intersection(&bounds)?;
                let smaller = area(dragged.size()).min(area(bounds.size()));
                let share = area(shared.size()) / smaller;
                (share >= OVERLAP).then_some((share, &card.key))
            })
            .max_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, key)| key)
    }

    /// The cursor that the card of an index shows for what it holds under a cursor it sees.
    fn interaction(
        &self,
        index: usize,
        tree: &Tree,
        view: View,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let card = &self.cards[index];
        let layout = view.layout(card.at, tree);
        let viewport = *viewport * view.inverse();
        card.content
            .mouse_interaction(tree, layout, cursor, &viewport, renderer)
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
            swap: None,
            settles: None,
            rest: None,
        })
    }

    fn diff(
        &mut self,
        tree: &mut Tree,
    ) {
        let state = tree
            .state
            .downcast_mut::<State<Key>>();
        // in the order they were, which a press changes, and those that are new on top
        let place = |key: &Key| {
            state
                .keys
                .iter()
                .position(|old| old == key)
                .unwrap_or(usize::MAX)
        };
        self.cards
            .sort_by_cached_key(|card| place(&card.key));
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
        // a card is as large as what it holds, whatever the surface is: its fills take nothing
        let all = Size::new(true, true);
        let least = layout::Limits::with_flags(Size::ZERO, Size::INFINITE, all, all);
        for (card, tree) in self
            .cards
            .iter_mut()
            .zip(&mut tree.children)
        {
            card.content
                .layout(tree, renderer, &least);
            let made = card
                .tall
                .filter(|tall| *tall > tree.size.height);
            if let Some(tall) = made {
                let tall = layout::Limits::with_flags(
                    Size::new(0.0, tall),
                    Size::new(f32::INFINITY, tall),
                    Size::new(true, false),
                    Size::new(true, false),
                );
                card.content
                    .layout(tree, renderer, &tall);
            }
            // the ports are where the content was laid out, so they cannot drift from it
            if !card
                .anchors
                .is_empty()
            {
                let whole = Rectangle::with_size(tree.size);
                let mut found = Anchors {
                    ids: &card.anchors,
                    heights: vec![0.0; card.anchors.len()],
                };
                let layout = Layout::new(tree.size);
                card.content
                    .operate(tree, layout, &whole, renderer, &mut found);
                card.inputs = found.heights;
            }
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
                // a point of the screen, as where text is selected from, is elsewhere to a card
                let mut seen = Seen {
                    operation: &mut *operation,
                    inverse: view.inverse(),
                    card: layout.bounds(),
                    scrolled: Vector::ZERO,
                };
                card.content
                    .operate(tree, layout, viewport, renderer, &mut seen);
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
        // a card that is pressed comes to the top, before anything else is made of the press
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && cursor.is_over(bounds)
            && let Some(hit) = self.hit(tree, view, local)
        {
            self.raise(tree, hit.card());
        }
        // as does one that the cursor rested on, which is told to open over the others
        let state = tree
            .state
            .downcast_mut::<State<Key>>();
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && let Some(rest) = &mut state.rest
            && let Wait::Until(until) = rest.wait
        {
            let index = state
                .keys
                .iter()
                .position(|key| *key == rest.key);
            match index {
                // a redraw before then lets go of the one asked for, so it is asked for again
                _ if *now < until => shell.request_redraw_at(until),
                Some(index) if self.cards[index].peeks => {
                    rest.wait = Wait::Told;
                    if let Some(on_peek) = &self.on_peek {
                        shell.publish(on_peek(Some(rest.key.clone())));
                    }
                    self.raise(tree, index);
                }
                _ => {}
            }
        }
        let state = tree
            .state
            .downcast_ref::<State<Key>>();
        let hit = self
            .hit(tree, view, local)
            .filter(|_| cursor.is_over(bounds));
        // only the top card under the cursor is under it, and none during a drag
        let is_free = state.drag.is_none();
        // a move tells the cards where the cursor is as they see it, as their cursor does
        let moved = match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let position = *position * view.inverse();
                Some(Event::Mouse(mouse::Event::CursorMoved { position }))
            }
            _ => None,
        };
        let seen = moved
            .as_ref()
            .unwrap_or(event);

        for (index, (card, tree)) in self
            .cards
            .iter_mut()
            .zip(&mut tree.children)
            .enumerate()
        {
            let cursor = match is_free && Hit::is_on(hit, index) {
                true => local,
                false => local.levitate(),
            };
            let layout = view.layout(card.at, tree);
            let viewport = *viewport * view.inverse();
            card.content
                .update(tree, seen, layout, cursor, renderer, shell, &viewport);
        }

        let key = |index: usize| {
            self.cards[index]
                .key
                .clone()
        };
        let state = tree
            .state
            .downcast_mut::<State<Key>>();
        // a card that lay on another for a while is to swap with it, which a halo shows
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && let Some(swap) = &mut state.swap
            && !swap.is_armed
        {
            let since = *swap
                .since
                .get_or_insert(*now);
            swap.is_armed = *now >= since + HOVER;
            shell.request_redraw_at(since + HOVER);
        }
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && let Some(settles) = state.settles
        {
            match *now >= settles {
                true => {
                    state.settles = None;
                    if let Some(on_zoom) = &self.on_zoom {
                        shell.publish(on_zoom(state.scale));
                    }
                }
                false => shell.request_redraw_at(settles),
            }
        }
        let Event::Mouse(event) = event else {
            return;
        };
        // the card that the cursor is on, which a drag or what lies over the cards leaves as it is
        let is_seen = is_free
            && cursor
                .position()
                .is_some();
        let on = match event {
            mouse::Event::CursorMoved { .. } if is_seen => Some(hit.map(Hit::card)),
            mouse::Event::CursorLeft => Some(None),
            _ => None,
        };
        let on = on.map(|on| on.map(|index| &self.cards[index].key));
        let is_gone = |rest: &Rest<Key>| {
            !self
                .cards
                .iter()
                .any(|card| card.key == rest.key)
        };
        match (&mut state.rest, on) {
            (rest, Some(on))
                if rest
                    .as_ref()
                    .map(|rest| &rest.key)
                    == on => {}
            // a card that is gone from under the cursor was given another key, and is the same
            (Some(rest), Some(Some(on))) if is_gone(rest) => rest.key = on.clone(),
            (rest, Some(on)) => {
                let is_left = rest
                    .take()
                    .is_some_and(|rest| rest.wait == Wait::Told);
                if let (true, Some(on_peek)) = (is_left, &self.on_peek) {
                    shell.publish(on_peek(None));
                }
                let until = Instant::now() + REST;
                *rest = on.map(|key| Rest {
                    key: key.clone(),
                    wait: Wait::Until(until),
                });
                if rest.is_some() {
                    shell.request_redraw_at(until);
                }
            }
            (_, None) => {}
        }
        // one that is pressed before it was told to open is not told before the cursor left it
        if let mouse::Event::ButtonPressed(_) = event
            && let Some(rest) = &mut state.rest
            && rest.wait != Wait::Told
        {
            rest.wait = Wait::Spent;
        }
        match event {
            // a drag ends wherever the cursor is, and whoever took the release
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                let swap = state
                    .swap
                    .take()
                    .filter(|swap| swap.is_armed);
                let message = match (state.drag.take(), hit, swap) {
                    (Some(Drag::Wire(from, _)), Some(Hit::Input(to, slot)), _) => self
                        .on_wire
                        .as_ref()
                        .map(|on_wire| on_wire(from, key(to), slot)),
                    // the two swap places, or the one that was dragged goes back to its own
                    (Some(Drag::Card(key, _, from)), _, Some(swap)) => self
                        .on_swap
                        .as_ref()
                        .and_then(|on_swap| on_swap(key.clone(), swap.target, from))
                        .or_else(|| {
                            self.on_move
                                .as_ref()
                                .map(|on_move| on_move(key, from))
                        }),
                    _ => None,
                };
                if let Some(message) = message {
                    shell.publish(message);
                }
                shell.request_redraw();
            }
            _ if shell.is_event_captured() => {}
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let (Some(at), Some(point)) = (cursor.position_over(bounds), local.position())
                else {
                    return;
                };
                // the cursor that a card shows for what it holds where the press is
                let shown = |index: usize| {
                    let tree = &tree.children[index];
                    self.interaction(index, tree, view, local, viewport, renderer)
                };
                state.drag = Some(match hit {
                    Some(Hit::Output(index)) => Drag::Wire(key(index), point),
                    Some(Hit::Grip(index)) => {
                        let card = view
                            .layout(self.cards[index].at, &tree.children[index])
                            .bounds();
                        let corner = Point::new(card.x + card.width, card.y + card.height);
                        Drag::Grip(key(index), corner - point)
                    }
                    // a card is dragged and opened by its handle alone
                    Some(Hit::Card(_)) => return,
                    Some(Hit::Input(index, _))
                        if self.cards[index]
                            .handle
                            .is_some() =>
                    {
                        return;
                    }
                    // and not by what shows a cursor of its own there, as text to select does
                    Some(Hit::Handle(index)) if shown(index) != mouse::Interaction::None => {
                        return;
                    }
                    Some(Hit::Handle(index)) if is_double(&mut state.pressed, at) => {
                        if let Some(on_open) = &self.on_open {
                            shell.publish(on_open(key(index)));
                        }
                        shell.capture_event();
                        return;
                    }
                    Some(Hit::Handle(index) | Hit::Input(index, _)) => {
                        let at = Point::ORIGIN + self.cards[index].at;
                        Drag::Card(key(index), point - (at + view.origin), at)
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
                    Some(Drag::Card(key, held, from)) => {
                        let at = point - view.origin - *held;
                        if let Some(on_move) = &self.on_move {
                            shell.publish(on_move(key.clone(), at));
                        }
                        // the card it lies on now, if that is another than before
                        let under = self.under(&tree.children, view, key, at);
                        let is_same = match (&state.swap, under) {
                            (Some(swap), Some(under)) => swap.target == *under,
                            (None, None) => true,
                            _ => false,
                        };
                        if !is_same {
                            state.swap = under.map(|under| Swap {
                                target: under.clone(),
                                since: None,
                                is_armed: false,
                                is_possible: self
                                    .on_swap
                                    .as_ref()
                                    .is_some_and(|on_swap| {
                                        on_swap(key.clone(), under.clone(), *from).is_some()
                                    }),
                            });
                        }
                    }
                    Some(Drag::Grip(key, held)) => {
                        let card = self
                            .cards
                            .iter()
                            .find(|card| card.key == *key);
                        if let (Some(card), Some(on_resize)) = (card, &self.on_resize) {
                            let Vector { x, y } =
                                point + *held - view.origin - card.at - Point::ORIGIN;
                            let size = Size::new(x.max(0.0), y.max(0.0));
                            shell.publish(on_resize(key.clone(), size));
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
                let settles = Instant::now() + SETTLE;
                state.settles = Some(settles);
                shell.capture_event();
                shell.request_redraw_at(settles);
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
            (Some(Drag::Grip(..)), _) | (None, Some(Hit::Grip(_))) => {
                mouse::Interaction::ResizingDiagonallyDown
            }
            (Some(Drag::Wire(..)), _) | (None, Some(Hit::Input(..) | Hit::Output(_))) => {
                mouse::Interaction::Crosshair
            }
            (Some(_), _) => mouse::Interaction::Grabbing,
            (None, Some(hit @ (Hit::Handle(index) | Hit::Card(index)))) => {
                let tree = &tree.children[index];
                let inner = self.interaction(index, tree, view, local, viewport, renderer);
                match (inner, hit) {
                    (mouse::Interaction::None, Hit::Handle(_)) => mouse::Interaction::Grab,
                    _ => inner,
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
        let scheme = Scheme::of(theme);
        let muted = scheme.outline;

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
            wire(&mut wires, from, *to, scheme.primary.color);
        }

        renderer.with_layer(clip, |renderer| {
            renderer.with_transformation(view.scale, |renderer| {
                renderer.with_translation(seen.position() - Point::ORIGIN, |renderer| {
                    renderer.draw_geometry(wires.into_geometry());
                });
                for (index, card, tree, layout) in self.laid_out(tree, view) {
                    let cursor = match Hit::is_on(hit, index) {
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
                        if card.is_resizable {
                            // three dots along the corner
                            for (x, y) in [
                                (NEAR, NEAR),
                                (FAR, NEAR),
                                (NEAR, FAR),
                            ] {
                                let corner = Point::new(
                                    bounds.x + bounds.width - x,
                                    bounds.y + bounds.height - y,
                                );
                                let dot = Rectangle::new(corner, Size::ZERO).expand(SPECK);
                                renderer.fill_quad(quad(dot, SPECK), muted);
                            }
                        }
                    });
                }
                // around the card that a drop swaps the dragged one with, or cannot
                let halo = state
                    .swap
                    .as_ref()
                    .filter(|swap| swap.is_armed)
                    .and_then(|swap| {
                        let (_, _, _, layout) = self
                            .laid_out(tree, view)
                            .find(|(_, card, ..)| card.key == swap.target)?;
                        let color = match swap.is_possible {
                            true => scheme.primary.color,
                            false => scheme.error.color,
                        };
                        Some((layout.bounds(), color))
                    });
                if let Some((bounds, color)) = halo {
                    // round as the card it lies around
                    let halo = renderer::Quad {
                        bounds: bounds.expand(HALO),
                        border: border::rounded(theme::surface::CORNER + HALO)
                            .color(color)
                            .width(HALO),
                        ..renderer::Quad::default()
                    };
                    renderer.with_layer(seen, |renderer| {
                        renderer.fill_quad(halo, Color::TRANSPARENT);
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

/// Notes a left press at a point, and tells whether it is the second of a double one.
fn is_double(
    pressed: &mut Option<mouse::Click>,
    at: Point,
) -> bool {
    let click = mouse::Click::new(at, mouse::Button::Left, *pressed);
    *pressed = Some(click);
    click.kind() == mouse::click::Kind::Double
}

/// Finds how far below the top of what it operates on the containers of some ids are.
struct Anchors<'a> {
    ids: &'a [widget::Id],
    heights: Vec<f32>,
}

impl widget::Operation for Anchors<'_> {
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn widget::Operation),
    ) {
        operate(self);
    }

    fn container(
        &mut self,
        id: Option<&widget::Id>,
        bounds: Rectangle,
        _viewport: &Rectangle,
    ) {
        let slot = self
            .ids
            .iter()
            .position(|anchor| Some(anchor) == id);
        if let Some(slot) = slot {
            self.heights[slot] = bounds.center_y();
        }
    }
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
    let bend = Vector::new(((to.x - from.x).abs() / 2.0).max(theme::surface::BEND), 0.0);
    let path = Path::new(|path| {
        path.move_to(from);
        path.bezier_curve_to(from + bend, to - bend, to);
    });
    let line = Stroke::default()
        .with_color(color)
        .with_width(stroke::MEDIUM);
    frame.stroke(&path, line);
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
