//! The surface under a simulator: whose state a card keeps, who takes a press, drags and zoom.

mod common;

use common::simulator;
use iced::mouse::{self, ScrollDelta};
use iced::widget::{button, column, container, pick_list, space, text, text_input};
use iced::{Event, Fill, Point, Rectangle, Size, Transformation, Widget};
use iced_test::selector::id;
use iced_test::simulator::{Simulator, click};
use polaroid_iced::{card, surface};

const KINDS: [&str; 3] = ["filter", "select", "join"];

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Moved(u32, Point),
    Named(u32, String),
    Picked(u32, &'static str),
    Pressed(u32),
}

/// A card for each id at its point: a title, a name to type, a kind to pick and a button.
fn scene(cards: &[(u32, Point)]) -> impl Widget<Message> + use<> {
    let mut scene = surface().on_move(Message::Moved);
    for &(card, at) in cards {
        let content = column![
            text!("card {card}"),
            text_input("name", "")
                .id(format!("name {card}"))
                .on_input(move |name| Message::Named(card, name)),
            pick_list(None::<&str>, KINDS, |kind| kind.to_string())
                .on_select(move |kind| Message::Picked(card, kind))
                .placeholder(format!("kind {card}")),
            button(text!("press {card}")).on_press(Message::Pressed(card)),
        ];
        let content = container(content)
            .width(200)
            .padding(10)
            .style(container::bordered_box);
        scene = scene.push(polaroid_iced::card(card, at, content));
    }
    scene
}

#[test]
fn a_card_keeps_its_state_when_one_before_it_goes() {
    let second = (2, Point::new(300.0, 0.0));
    let mut simulator = simulator(scene(&[(1, Point::ORIGIN), second]));
    simulator
        .click(id("name 2"))
        .expect("the second card has a name");

    let mut simulator = simulator.rebuild(scene(&[second]));
    simulator.typewrite("x");

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(
        messages,
        [Message::Named(
            2,
            "x".to_owned()
        )]
    );
}

#[test]
fn only_the_top_card_takes_a_press() {
    let mut simulator = simulator(scene(&[
        (1, Point::ORIGIN),
        (2, Point::ORIGIN),
    ]));
    simulator
        .click("press 1")
        .expect("the first card has a button");

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(messages, [Message::Pressed(2)]);
}

#[test]
fn a_card_is_dragged_by_what_takes_no_press() {
    let mut simulator = simulator(scene(&[(1, Point::new(50.0, 50.0))]));
    let position = Point::new(155.0, 85.0);

    simulator.point_at(Point::new(55.0, 55.0));
    simulator.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    simulator.point_at(position);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position }),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Event::Mouse(mouse::Event::CursorMoved {
            position: Point::ORIGIN,
        }),
    ]);

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(
        messages,
        [Message::Moved(
            1,
            Point::new(150.0, 80.0)
        )]
    );
}

#[test]
fn what_a_card_opens_is_zoomed_with_it() {
    let mut simulator = simulator(scene(&[(1, Point::ORIGIN)]));
    // about the origin, so a point of a card times the scale is where it is on screen
    simulator.point_at(Point::ORIGIN);
    simulator.scroll(ScrollDelta::Lines { x: 0.0, y: 3.0 });
    let scale = Transformation::scale(1.1_f32.powi(3));

    for label in ["kind 1", "select"] {
        let found = simulator
            .find(label)
            .expect("the label is shown");
        let center = found
            .bounds()
            .center();
        simulator.point_at(center * scale);
        simulator.simulate(click());
    }

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(messages, [Message::Picked(1, "select")]);
}

#[derive(Debug, Clone, PartialEq)]
enum Wiring {
    Wired(u32, u32, usize),
    Unwired(u32, usize),
    Menu(Point, Point),
    Grown(u32, Size),
    Opened(u32),
}

/// Two blank cards of 100 by 60, a port halfway down each side; the second resizes, by a handle.
fn ports() -> impl Widget<Wiring> {
    let blank = || {
        container(space())
            .width(100)
            .height(60)
            .id("whole")
    };
    let halfway = || [iced::advanced::widget::Id::new("whole")];
    surface()
        .push(
            card(1, Point::new(20.0, 20.0), blank())
                .ports(halfway())
                .output(30.0),
        )
        .push(
            card(2, Point::new(300.0, 20.0), blank())
                .ports(halfway())
                .output(30.0)
                .resizable(true)
                .handle(Rectangle::with_size(Size::new(20.0, 20.0))),
        )
        .on_wire(Wiring::Wired)
        .on_unwire(Wiring::Unwired)
        .on_menu(Wiring::Menu)
        .on_resize(Wiring::Grown)
        .on_open(Wiring::Opened)
}

/// Presses a button at a point, moves to another and lets go there.
fn drag<Message: 'static>(
    simulator: &mut Simulator<'_, Message>,
    button: mouse::Button,
    from: Point,
    to: Point,
) {
    simulator.point_at(from);
    simulator.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(button),
    )]);
    simulator.point_at(to);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: to }),
        Event::Mouse(mouse::Event::ButtonReleased(button)),
    ]);
}

#[test]
fn a_wire_is_dragged_from_an_output_to_an_input_and_a_right_press_unwires() {
    let mut simulator = simulator(ports());
    let output = Point::new(120.0, 50.0);
    let input = Point::new(300.0, 50.0);

    drag(&mut simulator, mouse::Button::Left, output, input);
    // let go of on no input, a wire is nothing
    drag(
        &mut simulator,
        mouse::Button::Left,
        output,
        Point::new(200.0, 200.0),
    );
    drag(&mut simulator, mouse::Button::Right, input, input);

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(
        messages,
        [
            Wiring::Wired(1, 2, 0),
            Wiring::Unwired(2, 0)
        ]
    );
}

#[test]
fn a_right_press_on_no_card_asks_for_a_menu_and_a_grip_resizes() {
    let mut simulator = simulator(ports());
    let blank = Point::new(200.0, 200.0);
    let grip = Point::new(395.0, 75.0);

    // on a card there is no menu
    drag(
        &mut simulator,
        mouse::Button::Right,
        Point::new(60.0, 60.0),
        blank,
    );
    drag(&mut simulator, mouse::Button::Right, blank, blank);
    drag(
        &mut simulator,
        mouse::Button::Left,
        grip,
        Point::new(415.0, 85.0),
    );

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    let grown = Wiring::Grown(2, Size::new(120.0, 70.0));
    assert_eq!(
        messages,
        [
            Wiring::Menu(blank, blank),
            grown
        ]
    );
}

#[test]
fn a_card_with_a_handle_is_dragged_by_that_alone() {
    let blank = container(space())
        .width(100)
        .height(60);
    let handle = Rectangle::new(Point::ORIGIN, Size::new(20.0, 20.0));
    let held = card(1, Point::new(20.0, 20.0), blank).handle(handle);
    let scene = surface()
        .push(held)
        .on_move(Message::Moved);
    let mut simulator = simulator(scene);

    drag(
        &mut simulator,
        mouse::Button::Left,
        Point::new(80.0, 60.0),
        Point::new(90.0, 70.0),
    );
    drag(
        &mut simulator,
        mouse::Button::Left,
        Point::new(30.0, 30.0),
        Point::new(40.0, 45.0),
    );

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(
        messages,
        [Message::Moved(
            1,
            Point::new(30.0, 35.0)
        )]
    );
}

#[test]
fn a_double_press_opens_a_card_by_its_handle_alone() {
    let mut simulator = simulator(ports());
    // all of the first card is its handle, and of the second its corner
    for card in [
        Point::new(60.0, 60.0),
        Point::new(340.0, 60.0),
        Point::new(310.0, 30.0),
    ] {
        simulator.point_at(card);
        simulator.simulate(click().chain(click()));
    }

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(
        messages,
        [
            Wiring::Opened(1),
            Wiring::Opened(2)
        ]
    );
}

#[test]
fn a_card_made_taller_than_what_it_holds_gives_the_rest_to_its_fills() {
    let scene = |tall: f32| {
        let content = column![
            space().height(Fill),
            text("last"),
        ]
        .width(100);
        surface::<u32, ()>().push(card(1, Point::ORIGIN, content).tall(tall))
    };
    let bottom = |tall: f32| {
        let found = simulator(scene(tall))
            .find("last")
            .expect("the last line is shown");
        let bounds = found.bounds();
        bounds.y + bounds.height
    };

    // no taller than its one line, whatever it was made
    let line = bottom(0.0);
    assert!(line > 0.0 && line < 40.0);
    assert_eq!(bottom(200.0), 200.0);
}

#[test]
fn a_port_is_level_with_the_container_of_its_id() {
    // the second line of a card is its input, wherever the first puts it
    let content = column![
        container(space()).height(70),
        container(space())
            .height(20)
            .id("input"),
    ]
    .width(100);
    let wired = card(1, Point::new(20.0, 20.0), content)
        .ports([iced::advanced::widget::Id::new("input")])
        .output(10.0);
    let scene = surface()
        .push(wired)
        .on_unwire(Wiring::Unwired);
    let mut simulator = simulator(scene);

    let port = Point::new(20.0, 100.0);
    drag(&mut simulator, mouse::Button::Right, port, port);
    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(messages, [Wiring::Unwired(1, 0)]);
}

#[test]
fn text_on_a_zoomed_card_is_selected_where_it_is_pressed() {
    use iced::advanced::widget::Id;
    use iced::advanced::widget::operation::{Operation, Text};

    /// Gathers what is selected of every text.
    struct Copy(Vec<String>);
    impl Operation for Copy {
        fn traverse(
            &mut self,
            operate: &mut dyn FnMut(&mut dyn Operation),
        ) {
            operate(self);
        }

        fn text(
            &mut self,
            _id: Option<&Id>,
            _bounds: Rectangle,
            state: &mut dyn Text,
        ) {
            self.0
                .extend(state.copy());
        }
    }

    // two cards level with each other, as text is selected by where the lines are
    let lines = |key: u32, at: Point| {
        let lines = column![
            text!("first of {key}").selectable(true),
            text!("second of {key}").selectable(true),
        ];
        card(key, at, lines)
    };
    let scene = surface::<u32, ()>()
        .push(lines(1, Point::new(20.0, 40.0)))
        .push(lines(2, Point::new(300.0, 40.0)));
    let mut simulator = simulator(scene);
    simulator.point_at(Point::ORIGIN);
    simulator.scroll(ScrollDelta::Lines { x: 0.0, y: 3.0 });
    let scale = Transformation::scale(1.1_f32.powi(3));

    // from the start of the first line of the first card to the end of its second
    let mut line = |line: &str| {
        simulator
            .find(line)
            .expect("the line is shown")
            .bounds()
    };
    let (first, second) = (line("first of 1"), line("second of 1"));
    let left = Point::new(first.x, first.center_y()) * scale;
    let right = Point::new(second.x + second.width, second.center_y()) * scale;
    // moved there first, for the text to show that a press on it selects
    simulator.point_at(left);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: left }),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
    ]);
    simulator.point_at(right);
    simulator.simulate([Event::Mouse(
        mouse::Event::CursorMoved { position: right },
    )]);

    let mut copy = Copy(Vec::new());
    simulator.operate(&mut copy);
    // and of the card beside it nothing, though its lines are level with these
    assert_eq!(copy.0, ["first of 1", "second of 1"]);
}

#[test]
fn text_that_is_scrolled_on_a_zoomed_card_is_selected_where_it_is_pressed() {
    use iced::advanced::widget::Id;
    use iced::advanced::widget::operation::{Operation, Text};
    use iced::widget::scrollable;

    /// Gathers what is selected of every text.
    struct Copy(Vec<String>);
    impl Operation for Copy {
        fn traverse(
            &mut self,
            operate: &mut dyn FnMut(&mut dyn Operation),
        ) {
            operate(self);
        }

        fn text(
            &mut self,
            _id: Option<&Id>,
            _bounds: Rectangle,
            state: &mut dyn Text,
        ) {
            self.0
                .extend(state.copy());
        }
    }

    // twenty lines of 20 pixels, of which five are seen at a time
    let lines: Vec<String> = (0..20)
        .map(|line| format!("line {line}"))
        .collect();
    let written = text(lines.join("\n"))
        .size(16)
        .line_height(iced::Pixels(20.0))
        .wrapping(text::Wrapping::None)
        .selectable(true);
    let both = scrollable::Direction::Both {
        vertical: scrollable::Scrollbar::new(),
        horizontal: scrollable::Scrollbar::new(),
    };
    let scrolled = scrollable(written)
        .direction(both)
        .width(200)
        .height(100);
    let at = Point::new(20.0, 40.0);
    let scene = surface::<u32, ()>().push(card(1, at, scrolled));
    let mut simulator = simulator(scene);
    simulator.point_at(Point::ORIGIN);
    simulator.scroll(ScrollDelta::Lines { x: 0.0, y: 3.0 });
    let scale = Transformation::scale(1.1_f32.powi(3));

    // seven lines down, with the cursor over what scrolls
    let over = Point::new(at.x + 100.0, at.y + 50.0) * scale;
    simulator.point_at(over);
    simulator.scroll(ScrollDelta::Pixels { x: 0.0, y: -140.0 });
    // which is where it was scrolled to as of the next frame
    simulator.simulate([Event::Window(
        iced::window::Event::RedrawRequested(iced::time::Instant::now()),
    )]);

    // across the second of the lines that are seen now
    let left = Point::new(at.x, at.y + 30.0) * scale;
    let right = Point::new(at.x + 150.0, at.y + 30.0) * scale;
    simulator.point_at(left);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: left }),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
    ]);
    simulator.point_at(right);
    simulator.simulate([Event::Mouse(
        mouse::Event::CursorMoved { position: right },
    )]);

    let mut copy = Copy(Vec::new());
    simulator.operate(&mut copy);
    assert_eq!(copy.0, ["line 8"]);
}

#[derive(Debug, Clone, PartialEq)]
enum Placed {
    Moved(u32, Point),
    Swapped(u32, u32, Point),
    Pressed(u32),
}

/// Three blank cards of 100 by 60, of which the third swaps with none; each holds a button.
fn blanks() -> impl Widget<Placed> {
    let blank = |key: u32| {
        container(button(text!("press {key}")).on_press(Placed::Pressed(key)))
            .width(100)
            .height(60)
    };
    surface()
        .push(card(1, Point::new(20.0, 20.0), blank(1)))
        .push(card(2, Point::new(300.0, 20.0), blank(2)))
        .push(card(3, Point::new(300.0, 200.0), blank(3)))
        .on_move(Placed::Moved)
        .on_swap(|dragged, under, from| {
            (under != 3).then_some(Placed::Swapped(dragged, under, from))
        })
}

/// What is asked for when the first card is dragged by this much, left there this long, and let go.
fn dropped(
    by: iced::Vector,
    left: std::time::Duration,
) -> Vec<Placed> {
    let frame = |simulator: &mut Simulator<'_, Placed>, at| {
        simulator.simulate([Event::Window(
            iced::window::Event::RedrawRequested(at),
        )]);
    };
    let start = iced::time::Instant::now();
    let mut simulator = simulator(blanks());
    // by its lower edge, under its button
    let from = Point::new(70.0, 70.0);
    simulator.point_at(from);
    simulator.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    let to = from + by;
    simulator.point_at(to);
    simulator.simulate([Event::Mouse(
        mouse::Event::CursorMoved { position: to },
    )]);
    frame(&mut simulator, start);
    frame(&mut simulator, start + left);
    simulator.simulate([Event::Mouse(
        mouse::Event::ButtonReleased(mouse::Button::Left),
    )]);
    simulator
        .into_messages()
        .collect()
}

#[test]
fn a_card_left_on_another_for_a_while_swaps_with_it_when_dropped() {
    let long = std::time::Duration::from_millis(500);
    let short = std::time::Duration::from_millis(100);
    let first = Point::new(20.0, 20.0);
    let moved = |by: iced::Vector| Placed::Moved(1, first + by);

    // most of it on the second card
    let on_second = iced::Vector::new(260.0, 10.0);
    assert_eq!(
        dropped(on_second, long),
        [
            moved(on_second),
            Placed::Swapped(1, 2, first)
        ]
    );
    // let go of too soon, it is only moved there
    assert_eq!(dropped(on_second, short), [moved(on_second)]);
    // as is one that lies on the corner of another alone
    let on_corner = iced::Vector::new(200.0, 40.0);
    assert_eq!(dropped(on_corner, long), [moved(on_corner)]);
    // and one that cannot swap goes back to where it was
    let on_third = iced::Vector::new(270.0, 170.0);
    assert_eq!(
        dropped(on_third, long),
        [
            moved(on_third),
            Placed::Moved(1, first)
        ]
    );
}

#[test]
fn a_card_that_is_pressed_comes_to_the_top() {
    // the second card lies over the button of the first
    let scene = || {
        let blank = |key: u32| {
            container(button(text!("press {key}")).on_press(Placed::Pressed(key)))
                .width(100)
                .height(60)
        };
        surface::<u32, Placed>()
            .push(card(1, Point::new(20.0, 20.0), blank(1)))
            .push(card(2, Point::new(20.0, 40.0), blank(2)))
    };
    let button = Point::new(40.0, 30.0);
    let mut simulator = simulator(scene());
    // a press on the top of the first, which the second leaves free
    simulator.point_at(Point::new(110.0, 25.0));
    simulator.simulate(click());
    // it stays on top when the surface is built anew
    let mut simulator = simulator.rebuild(scene());
    simulator.point_at(button + iced::Vector::new(0.0, 12.0));
    simulator.simulate(click());
    let pressed: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(pressed, [Placed::Pressed(1)]);
}

/// A surface of one card, a field that tells the steps of a drag across it, and tells its zoom.
fn zoomed() -> impl Widget<f32> {
    let field = text_input("", "5").width(200);
    let field = polaroid_iced::scrub(field).on_scrub(|steps| steps as f32);
    surface()
        .on_zoom(|zoom| zoom)
        .push(card(1, Point::new(50.0, 50.0), field))
}

#[test]
fn a_zoom_is_told_once_it_has_settled() {
    use iced::time::{Duration, Instant};
    use iced::window;

    let frame = |after| {
        Event::Window(window::Event::RedrawRequested(
            Instant::now() + Duration::from_millis(after),
        ))
    };
    let mut simulator = simulator(zoomed());
    simulator.point_at(Point::new(100.0, 60.0));
    // two steps of the wheel, and a frame before they have settled
    let step = Event::Mouse(mouse::Event::WheelScrolled {
        delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
    });
    simulator.simulate([step.clone(), step, frame(0)]);
    simulator.simulate([frame(1000), frame(2000)]);

    let told: Vec<f32> = simulator
        .into_messages()
        .collect();
    assert_eq!(told.len(), 1);
    assert!((told[0] - 1.21).abs() < 1e-4);
}

#[test]
fn a_card_is_told_a_move_where_it_sees_it() {
    use iced::time::{Duration, Instant};
    use iced::window;

    let start = Instant::now();
    let frame = |after| {
        Event::Window(window::Event::RedrawRequested(
            start + Duration::from_secs(after),
        ))
    };
    let moved = |simulator: &mut Simulator<f32>, position: Point| {
        simulator.point_at(position);
        simulator.simulate([Event::Mouse(
            mouse::Event::CursorMoved { position },
        )]);
    };
    let mut simulator = simulator(zoomed());
    let from = Point::new(100.0, 60.0);
    // the cards are a tenth larger, about where the cursor is
    moved(&mut simulator, from);
    simulator.simulate([Event::Mouse(
        mouse::Event::WheelScrolled {
            delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
        },
    )]);
    moved(&mut simulator, from);
    simulator.simulate([
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        frame(0),
    ]);
    // 66 pixels of the window are 60 of the card, which are ten steps of a slow drag
    moved(&mut simulator, Point::new(from.x + 66.0, from.y));
    simulator.simulate([frame(1)]);

    let steps: Vec<f32> = simulator
        .into_messages()
        .filter(|told| told.fract() == 0.0)
        .collect();
    assert_eq!(steps, [10.0]);
}

#[test]
fn a_card_that_peeks_is_told_to_open_once_the_cursor_rested_on_it_and_comes_to_the_top() {
    use iced::time::{Duration, Instant};
    use iced::window;

    #[derive(Debug, Clone, PartialEq)]
    enum Peek {
        Peeked(Option<u32>),
        Pressed(u32),
    }
    // the second card lies over the button of the first, which peeks
    let blank = |key: u32| {
        container(button(text!("press {key}")).on_press(Peek::Pressed(key)))
            .width(100)
            .height(60)
    };
    let scene = surface()
        .push(card(1, Point::new(20.0, 20.0), blank(1)).peek(true))
        .push(card(2, Point::new(20.0, 40.0), blank(2)))
        .on_peek(Peek::Peeked);
    let mut simulator = simulator(scene);
    let moved = |simulator: &mut Simulator<'_, Peek>, position: Point| {
        simulator.point_at(position);
        simulator.simulate([Event::Mouse(
            mouse::Event::CursorMoved { position },
        )]);
    };
    let rested = |simulator: &mut Simulator<'_, Peek>| {
        let later = Instant::now() + Duration::from_secs(1);
        simulator.simulate([Event::Window(
            window::Event::RedrawRequested(later),
        )]);
    };
    let first = Point::new(110.0, 25.0);
    let second = Point::new(110.0, 90.0);
    let button = Point::new(40.0, 42.0);

    // a press before the cursor rested ends the wait until the cursor left the card
    moved(&mut simulator, first);
    simulator.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(mouse::Button::Right),
    )]);
    rested(&mut simulator);
    moved(&mut simulator, Point::new(first.x + 1.0, first.y));
    rested(&mut simulator);
    // and the second card does not peek
    moved(&mut simulator, second);
    rested(&mut simulator);
    assert_eq!(
        simulator
            .drain()
            .count(),
        0
    );

    // the first is told once, takes the presses where the second lay on it, and is told it is left
    moved(&mut simulator, Point::new(300.0, 300.0));
    moved(&mut simulator, first);
    rested(&mut simulator);
    rested(&mut simulator);
    moved(&mut simulator, button);
    simulator.simulate(click());
    moved(&mut simulator, Point::new(300.0, 300.0));
    let told: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(
        told,
        [
            Peek::Peeked(Some(1)),
            Peek::Pressed(1),
            Peek::Peeked(None)
        ]
    );
}

#[test]
fn a_card_that_waits_to_peek_asks_to_be_drawn_then_again_at_each_redraw_before() {
    use iced::time::Instant;
    use iced::window::{self, RedrawRequest};
    use iced_test::core::renderer::{Headless, Settings};
    use iced_test::core::shell;
    use iced_test::futures::futures::executor::block_on;
    use iced_test::runtime::user_interface::{Cache, State, UserInterface};

    let blank = container(space())
        .width(100)
        .height(60);
    let scene = surface::<u32, ()>().push(card(1, Point::new(20.0, 20.0), blank).peek(true));
    let mut renderer = block_on(<iced::Renderer as Headless>::new(Settings::default(), None))
        .expect("a headless renderer");
    let size = Size::new(400.0, 300.0);
    let mut interface = UserInterface::build(scene, size, Cache::default(), &mut renderer);
    let mut messages = shell::Bus::new();
    // when the surface asks to be drawn again after an event, with the cursor on the card
    let position = Point::new(60.0, 50.0);
    let mut asked = |event: Event| {
        let (state, _) = interface.update(
            &window::Headless,
            &shell::Waker::noop(),
            &[event],
            mouse::Cursor::Available(position),
            &mut renderer,
            &mut messages,
        );
        match state {
            State::Updated { redraw_request, .. } => redraw_request,
            State::Outdated => panic!("nothing was told"),
        }
    };
    let RedrawRequest::At(then) = asked(Event::Mouse(mouse::Event::CursorMoved { position }))
    else {
        panic!("it asks to be drawn once the cursor rested");
    };
    // a redraw before then, which lets go of what was asked for, has it asked for again
    let now = Event::Window(window::Event::RedrawRequested(Instant::now()));
    assert_eq!(asked(now), RedrawRequest::At(then));
}

#[test]
fn a_card_given_another_key_under_the_cursor_is_not_told_that_the_cursor_left() {
    use iced::time::{Duration, Instant};
    use iced::window;

    let scene = |key: u32| {
        let blank = container(space())
            .width(100)
            .height(60);
        surface()
            .push(card(key, Point::new(20.0, 20.0), blank).peek(true))
            .on_peek(std::convert::identity)
    };
    let moved = |simulator: &mut Simulator<'_, Option<u32>>, position: Point| {
        simulator.point_at(position);
        simulator.simulate([Event::Mouse(
            mouse::Event::CursorMoved { position },
        )]);
    };
    let mut simulator = simulator(scene(1));
    moved(&mut simulator, Point::new(60.0, 50.0));
    let later = Instant::now() + Duration::from_secs(1);
    simulator.simulate([Event::Window(
        window::Event::RedrawRequested(later),
    )]);
    assert_eq!(
        simulator
            .drain()
            .collect::<Vec<_>>(),
        [Some(1)]
    );

    // as one that is renamed while the cursor rests on it
    let mut simulator = simulator.rebuild(scene(2));
    moved(&mut simulator, Point::new(61.0, 50.0));
    assert_eq!(
        simulator
            .drain()
            .count(),
        0
    );
    moved(&mut simulator, Point::new(300.0, 200.0));
    let told: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(told, [None]);
}

#[test]
fn a_card_panned_from_under_the_cursor_is_told_that_the_cursor_left() {
    use iced::keyboard::{self, Modifiers};
    use iced::time::{Duration, Instant};
    use iced::window;

    let blank = container(space())
        .width(100)
        .height(60);
    let scene = surface()
        .push(card(1, Point::new(20.0, 20.0), blank).peek(true))
        .on_peek(std::convert::identity);
    let mut simulator = simulator(scene);
    let position = Point::new(60.0, 50.0);
    let later = Instant::now() + Duration::from_secs(1);
    simulator.point_at(position);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position }),
        Event::Window(window::Event::RedrawRequested(later)),
        Event::Keyboard(keyboard::Event::ModifiersChanged(Modifiers::CTRL)),
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: ScrollDelta::Lines { x: 0.0, y: -5.0 },
        }),
    ]);
    let told: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(told, [Some(1), None]);
}

#[test]
fn the_wheel_pans_while_ctrl_is_held_and_across_with_shift_too() {
    use iced::keyboard::{self, Modifiers};

    let mut simulator = simulator(ports());
    // where among the cards a point of the surface is, as a menu asked for there is told
    let among = |simulator: &mut Simulator<'_, Wiring>, held: Modifiers| {
        let blank = Point::new(200.0, 200.0);
        simulator.point_at(blank);
        simulator.simulate([
            Event::Keyboard(keyboard::Event::ModifiersChanged(held)),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        ]);
        match simulator
            .drain()
            .last()
        {
            Some(Wiring::Menu(_, among)) => among,
            _ => panic!("a menu is asked for"),
        }
    };
    // a line of the wheel moves the cards by 40, so the point is that much further up or left
    assert_eq!(
        among(&mut simulator, Modifiers::CTRL),
        Point::new(200.0, 160.0)
    );
    let across = Modifiers::CTRL | Modifiers::SHIFT;
    assert_eq!(among(&mut simulator, across), Point::new(160.0, 160.0));
    // over a card too, which takes no step of the wheel for itself then
    simulator.point_at(Point::new(60.0, 60.0));
    simulator.scroll(ScrollDelta::Lines { x: 0.0, y: 1.0 });
    assert_eq!(among(&mut simulator, across), Point::new(80.0, 160.0));
}

/// A row of pixels of `scene` in a window of 400 by 300, the wheel turned `steps` at its corner.
fn row_of(
    scene: impl Widget<Message>,
    steps: f32,
    row: usize,
) -> Vec<[u8; 4]> {
    let wheel = mouse::Event::WheelScrolled {
        delta: ScrollDelta::Lines { x: 0.0, y: steps },
    };
    row_after(scene, [(Point::ORIGIN, Event::Mouse(wheel))], row)
}

/// A row of pixels of `scene` in a window of 400 by 300, after each event with the cursor there.
fn row_after<Message>(
    scene: impl Widget<Message>,
    events: impl IntoIterator<Item = (Point, Event)>,
    row: usize,
) -> Vec<[u8; 4]> {
    use iced_test::core::renderer::{Headless, Settings, Style};
    use iced_test::core::shell;
    use iced_test::futures::futures::executor::block_on;
    use iced_test::runtime::user_interface::{Cache, UserInterface};

    let mut renderer = block_on(<iced::Renderer as Headless>::new(Settings::default(), None))
        .expect("a headless renderer");
    let size = Size::new(400.0, 300.0);
    let mut interface = UserInterface::build(scene, size, Cache::default(), &mut renderer);
    let mut cursor = mouse::Cursor::Unavailable;
    for (at, event) in events {
        cursor = mouse::Cursor::Available(at);
        let _ = interface.update(
            &iced::window::Headless,
            &shell::Waker::noop(),
            &[event],
            cursor,
            &mut renderer,
            &mut shell::Bus::new(),
        );
    }
    let style = Style {
        text_color: iced::Color::BLACK,
    };
    interface.draw(&mut renderer, &iced::Theme::Light, &style, cursor);
    let pixels = renderer.screenshot(Size::new(400, 300), 1.0, iced::Color::WHITE);
    pixels
        .chunks_exact(4)
        .skip(row * 400)
        .take(400)
        .map(|pixel| {
            [
                pixel[0], pixel[1], pixel[2], pixel[3],
            ]
        })
        .collect()
}

/// Where along a row the pixels are that are of the one of red, green and blue at `channel`.
fn run(
    row: &[[u8; 4]],
    channel: usize,
) -> Vec<usize> {
    row.iter()
        .enumerate()
        .filter(|(_, pixel)| {
            (0..3).all(|other| match other == channel {
                true => pixel[other] > 200,
                false => pixel[other] < 80,
            })
        })
        .map(|(x, _)| x)
        .collect()
}

/// A card of 100 by 60 at 40 from the corner: ringed in red, in green and blue around that.
fn ringed(is_haloed: bool) -> impl Widget<Message> + use<> {
    use iced::Color;

    let blank = container(space())
        .width(100)
        .height(60);
    let rings = [
        Color::from_rgb8(0, 255, 0),
        Color::from_rgb8(0, 0, 255),
    ];
    let card = card(1, Point::new(40.0, 40.0), blank)
        .ring(Some(Color::from_rgb8(255, 0, 0)))
        .rings(rings)
        .halo(is_haloed);
    surface::<u32, Message>().push(card)
}

#[test]
fn the_rings_of_a_card_are_no_thinner_on_the_screen_when_it_is_zoomed_far_out() {
    // as large as it is: a ring of three on its edge, and two of two around it, gaps of two
    let row = row_of(ringed(false), 0.0, 70);
    let [red, green, blue] = [0, 1, 2].map(|channel| run(&row[..90], channel));
    assert_eq!(red, [40, 41, 42]);
    assert_eq!(green, [36, 37]);
    assert_eq!(blue, [32, 33]);

    // a quarter as large, each ring is two pixels still, with a gap of one before it
    let row = row_of(ringed(false), -100.0, 17);
    let [red, green, blue] = [0, 1, 2].map(|channel| run(&row[..22], channel));
    assert_eq!(red, [10, 11]);
    assert_eq!(green, [7, 8]);
    assert_eq!(blue, [4, 5]);
}

#[test]
fn a_halo_tints_what_lies_around_a_card_and_its_rings_however_far_out() {
    let is_tinted = |is_haloed: bool, steps: f32, row: usize, x: usize| {
        let [r, g, b, _] = row_of(ringed(is_haloed), steps, row)[x];
        // faintly in the primary colour, which is no grey
        r != 255 && (r != g || g != b)
    };
    // left of the outermost ring, as large as it is and a quarter as large
    assert!(is_tinted(true, 0.0, 70, 26) && !is_tinted(false, 0.0, 70, 26));
    assert!(is_tinted(true, -100.0, 17, 2) && !is_tinted(false, -100.0, 17, 2));
}

#[test]
fn a_halo_singles_out_the_card_that_a_drop_swaps_the_dragged_one_with_or_cannot() {
    let scene = |swaps: bool| {
        let blank = || {
            container(space())
                .width(100)
                .height(60)
        };
        surface::<u32, ()>()
            .on_move(|_, _| ())
            .on_swap(move |_, _, _| swaps.then_some(()))
            .push(card(1, Point::new(20.0, 20.0), blank()))
            .push(card(2, Point::new(200.0, 100.0), blank()))
    };
    // the first card is dragged onto most of the second, and left there for a while
    let (from, to) = (Point::new(70.0, 50.0), Point::new(245.0, 135.0));
    let start = iced::time::Instant::now();
    let frame = |after: u64| {
        let at = start + std::time::Duration::from_millis(after);
        Event::Window(iced::window::Event::RedrawRequested(at))
    };
    let left_of_second = |swaps: bool, left: u64| {
        let events = [
            (
                from,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            ),
            (to, Event::Mouse(mouse::Event::CursorMoved { position: to })),
            (to, frame(0)),
            (to, frame(left)),
        ];
        row_after(scene(swaps), events, 130)[194]
    };
    let [r, g, b, _] = left_of_second(true, 500);
    assert!(b > r && r != 255, "{r} {g} {b}");
    // in the colour of an error if they cannot swap, and not before it lay there
    let [r, g, b, _] = left_of_second(false, 500);
    assert!(r > b && g != 255, "{r} {g} {b}");
    assert_eq!(left_of_second(true, 100), [255; 4]);
}
