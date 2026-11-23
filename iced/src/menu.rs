//! A menu of groups at a point: a group under the cursor opens its items beside it.

use iced::advanced::layout::{self, Layout};
use iced::advanced::text::{self, Renderer as _};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Renderer as _, Shell, Text, Widget, mouse, renderer};
use iced::alignment::Vertical;
use iced::{Border, Color, Event, Fill, Font, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::width_of;

/// The height of a line, the size of its text, and the room left and right of that.
const LINE: f32 = 26.0;
const TEXT: f32 = 13.0;
const SIDE: f32 = 12.0;
/// The room above the first line of a list and below its last.
const EDGE: f32 = 4.0;
/// What marks a group as one that opens, and the room it takes at the end of its line.
const OPENS: &str = "›";
const MARK: f32 = 18.0;

/// A menu of groups, each a label over items of a label and a message, at a point of its room.
pub fn menu<Message>(
    at: Point,
    groups: Vec<(String, Vec<(String, Message)>)>,
) -> Menu<Message> {
    // a list is as wide as the widest of its labels
    let wide = |labels: &mut dyn Iterator<Item = &String>| {
        labels
            .map(|label| width_of(label, TEXT, Font::DEFAULT))
            .fold(0.0, f32::max)
            + 2.0 * SIDE
    };
    let width = wide(
        &mut groups
            .iter()
            .map(|(label, _)| label),
    ) + MARK;
    let widths = groups
        .iter()
        .map(|(_, items)| {
            wide(
                &mut items
                    .iter()
                    .map(|(label, _)| label),
            )
        })
        .collect();
    Menu {
        at,
        groups,
        width,
        widths,
        on_close: None,
    }
}

/// The groups in a list, and the items of the one under the cursor in a list beside it.
///
/// Both lists are moved to lie inside the room of the menu, which is all it is given.
pub struct Menu<Message> {
    at: Point,
    groups: Vec<(String, Vec<(String, Message)>)>,
    /// The width of the list of the groups, and of the list of the items of each.
    width: f32,
    widths: Vec<f32>,
    on_close: Option<Message>,
}

impl<Message> Menu<Message> {
    /// The message for a press beside the lists.
    pub fn on_close(
        mut self,
        on_close: Message,
    ) -> Self {
        self.on_close = Some(on_close);
        self
    }

    /// Publishes the message for a press beside the lists, if there is one.
    fn close(
        &self,
        shell: &mut Shell<'_, Message>,
    ) where
        Message: Clone,
    {
        if let Some(on_close) = &self.on_close {
            shell.publish(on_close.clone());
        }
    }

    /// The list of the groups, moved up and left as far as it takes to lie in `bounds`.
    fn list(
        &self,
        bounds: Rectangle,
    ) -> Rectangle {
        let size = Size::new(self.width, self.groups.len() as f32 * LINE + 2.0 * EDGE);
        let at = Point::new(bounds.x + self.at.x, bounds.y + self.at.y);
        within(Rectangle::new(at, size), bounds)
    }

    /// The list of the items of a group: beside its line, on the side that has the room.
    fn items(
        &self,
        group: usize,
        bounds: Rectangle,
    ) -> Rectangle {
        let list = self.list(bounds);
        let width = self.widths[group];
        let lines = self.groups[group]
            .1
            .len();
        let size = Size::new(width, lines as f32 * LINE + 2.0 * EDGE);
        let is_right = list.x + list.width + width <= bounds.x + bounds.width;
        let x = match is_right {
            true => list.x + list.width - 1.0,
            false => list.x - width + 1.0,
        };
        // its first item level with its group
        let at = Point::new(x, list.y + group as f32 * LINE);
        within(Rectangle::new(at, size), bounds)
    }

    /// What of the menu is at a point, with the group of this index open.
    fn hit(
        &self,
        open: Option<usize>,
        point: Point,
        bounds: Rectangle,
    ) -> Hit {
        let line = |list: Rectangle, lines: usize| {
            let line = ((point.y - list.y - EDGE) / LINE).floor();
            (line >= 0.0 && (line as usize) < lines).then_some(line as usize)
        };
        if let Some(group) = open {
            let items = self.items(group, bounds);
            if items.contains(point) {
                let lines = self.groups[group]
                    .1
                    .len();
                return match line(items, lines) {
                    Some(item) => Hit::Item(group, item),
                    None => Hit::Edge,
                };
            }
        }
        let list = self.list(bounds);
        match list.contains(point) {
            true => line(list, self.groups.len()).map_or(Hit::Edge, Hit::Group),
            false => Hit::Beside,
        }
    }
}

/// What of a menu is at a point.
#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Group(usize),
    Item(usize, usize),
    /// A list, but none of its lines.
    Edge,
    Beside,
}

/// The group whose items are shown, and where the cursor last was.
#[derive(Default)]
struct State {
    open: Option<usize>,
    last: Option<Point>,
}

impl<Message> widget::Meta for Menu<Message> {}

impl<Message: Clone> Widget<Message, Theme, Renderer> for Menu<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Fill, Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) {
        tree.size = limits.resolve(Fill, Fill, Size::ZERO);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree
            .state
            .downcast_mut::<State>();
        let Some(point) = cursor.position_over(bounds) else {
            return;
        };
        let hit = self.hit(state.open, point, bounds);
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Hit::Group(group) = hit
                    && state.open != Some(group)
                {
                    // on its way to the items of the open group, it may cross other groups
                    let is_on_its_way = state
                        .open
                        .zip(state.last)
                        .is_some_and(|(open, last)| {
                            let items = self.items(open, bounds);
                            let list = self.list(bounds);
                            let near = match items.x > list.x {
                                true => items.x,
                                false => items.x + items.width,
                            };
                            let corners = [
                                Point::new(near, items.y),
                                Point::new(near, items.y + items.height),
                            ];
                            is_between(point, last, corners)
                        });
                    if !is_on_its_way {
                        state.open = Some(group);
                    }
                }
                state.last = Some(point);
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                match hit {
                    Hit::Item(group, item) => {
                        let (_, message) = &self.groups[group].1[item];
                        shell.publish(message.clone());
                    }
                    Hit::Group(group) => state.open = Some(group),
                    Hit::Edge => {}
                    Hit::Beside => self.close(shell),
                }
                shell.capture_event();
                shell.request_redraw();
            }
            // a right press beside the lists is left to what lies there, as to open another
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
                if hit == Hit::Beside =>
            {
                self.close(shell);
            }
            Event::Mouse(mouse::Event::ButtonPressed(_) | mouse::Event::WheelScrolled { .. })
                if hit != Hit::Beside =>
            {
                shell.capture_event()
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let state = tree
            .state
            .downcast_ref::<State>();
        let hit = cursor
            .position_over(bounds)
            .map(|point| self.hit(state.open, point, bounds));
        match hit {
            Some(Hit::Group(_) | Hit::Item(..)) => mouse::Interaction::Pointer,
            // over a list, what lies under it is not under the cursor
            Some(Hit::Edge) => mouse::Interaction::Idle,
            _ => mouse::Interaction::None,
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree
            .state
            .downcast_ref::<State>();
        let hit = cursor
            .position_over(bounds)
            .map(|point| self.hit(state.open, point, bounds));
        let palette = theme.palette();
        let paper = palette
            .background
            .base;
        let lit = palette
            .background
            .strong
            .color;
        let rim = Border {
            color: lit,
            width: 1.0,
            radius: 4.0.into(),
        };

        let list = |renderer: &mut Renderer, list: Rectangle, lines: &[(&str, bool, bool)]| {
            let whole = renderer::Quad {
                bounds: list,
                border: rim,
                ..renderer::Quad::default()
            };
            renderer.fill_quad(whole, paper.color);
            for (index, (label, is_lit, opens)) in lines
                .iter()
                .enumerate()
            {
                let line = Rectangle {
                    x: list.x + 1.0,
                    y: list.y + EDGE + index as f32 * LINE,
                    width: list.width - 2.0,
                    height: LINE,
                };
                if *is_lit {
                    let under = renderer::Quad {
                        bounds: line,
                        ..renderer::Quad::default()
                    };
                    renderer.fill_quad(under, lit);
                }
                let middle = line.y + LINE / 2.0;
                let written = |content: &str| Text {
                    content: content.to_owned(),
                    bounds: line.size(),
                    size: TEXT.into(),
                    line_height: text::LineHeight::default(),
                    font: Font::DEFAULT,
                    align_x: text::Alignment::Left,
                    align_y: Vertical::Center,
                    shaping: text::Shaping::default(),
                    wrapping: text::Wrapping::None,
                    ellipsis: text::Ellipsis::None,
                    hint_factor: None,
                };
                let ink: Color = paper.text;
                renderer.fill_text(written(label), Point::new(line.x + SIDE, middle), ink, line);
                if *opens {
                    let mark = Point::new(line.x + line.width - MARK, middle);
                    renderer.fill_text(written(OPENS), mark, ink, line);
                }
            }
        };

        let groups: Vec<_> = self
            .groups
            .iter()
            .enumerate()
            .map(|(group, (label, _))| {
                let is_lit = state.open == Some(group) || hit == Some(Hit::Group(group));
                (label.as_str(), is_lit, true)
            })
            .collect();
        list(renderer, self.list(bounds), &groups);
        if let Some(group) = state.open {
            let items: Vec<_> = self.groups[group]
                .1
                .iter()
                .enumerate()
                .map(|(item, (label, _))| {
                    let is_lit = hit == Some(Hit::Item(group, item));
                    (label.as_str(), is_lit, false)
                })
                .collect();
            // over the list of the groups, where they meet
            renderer.with_layer(bounds, |renderer| {
                list(renderer, self.items(group, bounds), &items);
            });
        }
    }
}

/// A rectangle moved up and left as far as it takes to end inside `bounds`, but not out of it.
fn within(
    rectangle: Rectangle,
    bounds: Rectangle,
) -> Rectangle {
    let x = rectangle
        .x
        .min(bounds.x + bounds.width - rectangle.width)
        .max(bounds.x);
    let y = rectangle
        .y
        .min(bounds.y + bounds.height - rectangle.height)
        .max(bounds.y);
    Rectangle { x, y, ..rectangle }
}

/// Whether a point lies in the triangle of where the cursor was and two corners it heads for.
fn is_between(
    point: Point,
    from: Point,
    corners: [Point; 2],
) -> bool {
    let side = |a: Point, b: Point| (point.x - b.x) * (a.y - b.y) - (a.x - b.x) * (point.y - b.y);
    let sides = [
        side(from, corners[0]),
        side(corners[0], corners[1]),
        side(corners[1], from),
    ];
    let is_left = sides
        .iter()
        .all(|side| *side <= 0.0);
    let is_right = sides
        .iter()
        .all(|side| *side >= 0.0);
    // not where it was, which is a corner of every such triangle
    point != from && (is_left || is_right)
}
