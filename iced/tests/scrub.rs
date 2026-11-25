//! A scrubbed field under a simulator: a drag across it steps, and a press alone is the field's.

use std::time::Duration;

use iced::time::Instant;
use iced::widget::text_input;
use iced::{Event, Point, Widget, mouse, window};
use iced_test::{Simulator, simulator};
use polaroid_iced::scrub;

/// A field that asks for 0 when typed in, and for the steps of a drag across it.
fn scene() -> impl Widget<i32> {
    let field = text_input("", "5")
        .on_input(|_| 0)
        .width(200);
    scrub(field).on_scrub(|steps| steps)
}

fn press(
    simulator: &mut Simulator<i32>,
    position: Point,
) {
    // a cursor is moved to where it presses
    move_to(simulator, position);
    simulator.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
}

fn move_to(
    simulator: &mut Simulator<i32>,
    position: Point,
) {
    simulator.point_at(position);
    simulator.simulate([Event::Mouse(
        mouse::Event::CursorMoved { position },
    )]);
}

/// A frame that comes this long after `start`.
fn frame(
    simulator: &mut Simulator<i32>,
    start: Instant,
    after: Duration,
) {
    simulator.simulate([Event::Window(
        window::Event::RedrawRequested(start + after),
    )]);
}

/// What is asked for when a press at the left of the field is dragged to these places in turn.
///
/// A frame comes when it is pressed, and `apart` after each place it is dragged to.
fn dragged(
    to: &[f32],
    apart: Duration,
) -> Vec<i32> {
    let start = Instant::now();
    let mut simulator = simulator(scene());
    press(&mut simulator, Point::new(50.0, 10.0));
    frame(&mut simulator, start, Duration::ZERO);
    for (i, x) in to
        .iter()
        .enumerate()
    {
        move_to(&mut simulator, Point::new(*x, 10.0));
        frame(&mut simulator, start, apart * (i as u32 + 1));
    }
    simulator.simulate([Event::Mouse(
        mouse::Event::ButtonReleased(mouse::Button::Left),
    )]);
    simulator
        .into_messages()
        .collect()
}

/// Long enough between frames for any drag across the field to be a slow one.
const SLOWLY: Duration = Duration::from_secs(1);

#[test]
fn a_slow_drag_across_it_asks_for_a_step_every_few_pixels() {
    // three whole steps of six pixels, then one more, then five back
    assert_eq!(dragged(&[70.0, 75.0, 44.0], SLOWLY), [3, 1, -5]);
}

#[test]
fn a_faster_drag_asks_for_more_steps_of_the_same_pixels() {
    let millis = Duration::from_millis;
    // sixty pixels are ten steps at 200 pixels a second and below
    assert_eq!(dragged(&[110.0], millis(300)), [10]);
    // at three times that they are some five times as many
    assert_eq!(dragged(&[110.0], millis(100)), [51]);
    // and never more than forty times
    assert_eq!(dragged(&[110.0], millis(10)), [400]);
    assert_eq!(dragged(&[110.0], millis(1)), [400]);
    // back as forth
    assert_eq!(dragged(&[110.0, 50.0], millis(100)), [51, -51]);
}

#[test]
fn it_asks_once_a_frame_however_often_it_is_moved() {
    let start = Instant::now();
    let mut simulator = simulator(scene());
    press(&mut simulator, Point::new(50.0, 10.0));
    for x in [56.0, 62.0, 70.0] {
        move_to(&mut simulator, Point::new(x, 10.0));
    }
    frame(&mut simulator, start, Duration::ZERO);
    // a frame that is drawn again asks nothing again
    frame(&mut simulator, start, Duration::ZERO);
    let asked: Vec<i32> = simulator
        .into_messages()
        .collect();
    assert_eq!(asked, [3]);
}

#[test]
fn a_press_that_hardly_moves_is_left_to_the_field() {
    assert_eq!(dragged(&[52.0, 49.0], SLOWLY), []);
    assert_eq!(dragged(&[], SLOWLY), []);
}

#[test]
fn one_that_is_dragged_upward_steps_up_for_more() {
    let button = iced::widget::button("wait").on_press(0);
    let start = Instant::now();
    let mut simulator = simulator(
        scrub(button)
            .upward()
            .on_scrub(|steps| steps),
    );
    press(&mut simulator, Point::new(10.0, 10.0));
    // across it is no drag, and up from the window it still is one
    let places = [
        Point::new(60.0, 10.0),
        Point::new(60.0, -3.0),
        Point::new(60.0, 12.0),
    ];
    for (i, position) in places
        .into_iter()
        .enumerate()
    {
        move_to(&mut simulator, position);
        frame(&mut simulator, start, SLOWLY * i as u32);
    }
    let asked: Vec<i32> = simulator
        .into_messages()
        .collect();
    assert_eq!(asked, [2, -2]);
}

#[test]
fn one_that_is_scrolled_is_dragged_from_where_it_is_pressed() {
    use iced::widget::{column, scrollable, space};

    let start = Instant::now();
    let field = scrub(text_input("", "5"))
        .upward()
        .on_scrub(|steps| steps);
    let content = column![
        space::vertical().height(500),
        field
    ];
    // scrolled to its end, the cursor is told further down than the window has it
    let scrolled = scrollable(content)
        .height(100)
        .anchor_bottom();
    let mut simulator = simulator(scrolled);
    // which a frame has it do
    frame(&mut simulator, start, Duration::ZERO);
    press(&mut simulator, Point::new(50.0, 85.0));
    frame(&mut simulator, start, Duration::ZERO);
    move_to(&mut simulator, Point::new(50.0, 73.0));
    frame(&mut simulator, start, SLOWLY);
    let asked: Vec<i32> = simulator
        .into_messages()
        .collect();
    assert_eq!(asked, [2]);
}

#[test]
fn a_handle_is_dragged_from_where_it_is() {
    use iced::widget::row;
    use polaroid_iced::{HANDLE, handle};

    let field = text_input("", "5").width(100);
    let start = Instant::now();
    let mut simulator = simulator(row![
        field,
        handle().on_scrub(|steps| steps)
    ]);
    let at = simulator
        .find(HANDLE)
        .expect("the handle is shown")
        .bounds()
        .center();
    press(&mut simulator, at);
    // up for more
    move_to(&mut simulator, Point::new(at.x, at.y - 30.0));
    frame(&mut simulator, start, Duration::ZERO);
    let asked: Vec<i32> = simulator
        .into_messages()
        .collect();
    assert_eq!(asked, [5]);
}

/// What a drag up from a handle, or `beside` it to its right, asks for, and the text it selects.
fn selected_by(beside: f32) -> (Vec<i32>, String) {
    use iced::Vector;
    use iced::advanced::widget::operation::{self, Operation, Outcome};
    use iced::widget::{column, row, text};
    use polaroid_iced::{HANDLE, handle};

    let start = Instant::now();
    let mut simulator = simulator(column![
        text("a line of text above, which a drag across it selects").selectable(true),
        row![
            text_input("", "5").width(100),
            handle().on_scrub(|steps| steps)
        ],
    ]);
    let at = simulator
        .find(HANDLE)
        .expect("the handle is shown")
        .bounds()
        .center()
        + Vector::new(beside, 0.0);
    press(&mut simulator, at);
    // up into the line above
    move_to(&mut simulator, Point::new(at.x, at.y - 24.0));
    frame(&mut simulator, start, Duration::ZERO);
    let mut copy = operation::text::copy();
    simulator.operate(&mut operation::black_box(&mut copy));
    let Outcome::Some(selected) = copy.finish() else {
        unreachable!("a copy tells what is selected, if only nothing");
    };
    let asked = simulator
        .into_messages()
        .collect();
    (asked, selected)
}

#[test]
fn a_drag_on_a_handle_selects_no_text() {
    // the value moves four steps, and nothing is selected
    assert_eq!(selected_by(0.0), (vec![4], String::new()));
    // which the same drag beside it does select, where nothing takes the press
    assert_ne!(selected_by(40.0).1, "");
}
