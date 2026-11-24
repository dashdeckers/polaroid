//! The surface under a simulator: whose state a card keeps, who takes a press, drags and zoom.

use iced::mouse::{self, ScrollDelta};
use iced::widget::{button, column, container, pick_list, space, text, text_input};
use iced::{Event, Fill, Point, Rectangle, Size, Transformation, Widget};
use iced_test::selector::id;
use iced_test::simulator::{Simulator, click, simulator};
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

/// Two blank cards of 100 by 60 with a port halfway down each side; the second resizes, not loose.
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
                .loose(false),
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
    let held = card(1, Point::new(20.0, 20.0), blank)
        .handle(handle)
        .loose(false);
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
fn a_double_press_opens_a_loose_card_alone() {
    let mut simulator = simulator(ports());
    for card in [
        Point::new(60.0, 60.0),
        Point::new(340.0, 60.0),
    ] {
        simulator.point_at(card);
        simulator.simulate(click().chain(click()));
    }

    let messages: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(messages, [Wiring::Opened(1)]);
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
