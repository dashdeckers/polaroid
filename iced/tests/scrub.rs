//! A scrubbed field under a simulator: a drag across it steps, and a press alone is the field's.

use iced::widget::text_input;
use iced::{Event, Point, Widget, mouse};
use iced_test::simulator;
use polaroid_iced::scrub;

/// A field that asks for 0 when typed in, and for the steps of a drag across it.
fn scene() -> impl Widget<i32> {
    let field = text_input("", "5")
        .on_input(|_| 0)
        .width(200);
    scrub(field).on_scrub(|steps| steps)
}

/// What is asked for when a press at the left of the field is dragged to these places in turn.
fn dragged(to: &[f32]) -> Vec<i32> {
    let mouse = Event::Mouse;
    let mut simulator = simulator(scene());
    simulator.point_at(Point::new(50.0, 10.0));
    simulator.simulate([mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    for x in to {
        let position = Point::new(*x, 10.0);
        simulator.point_at(position);
        simulator.simulate([mouse(
            mouse::Event::CursorMoved { position },
        )]);
    }
    simulator.simulate([mouse(
        mouse::Event::ButtonReleased(mouse::Button::Left),
    )]);
    simulator
        .into_messages()
        .collect()
}

#[test]
fn a_drag_across_it_asks_for_a_step_every_few_pixels() {
    // three whole steps of six pixels, then one more, then five back
    assert_eq!(dragged(&[70.0, 75.0, 44.0]), [3, 1, -5]);
}

#[test]
fn a_press_that_hardly_moves_is_left_to_the_field() {
    assert_eq!(dragged(&[52.0, 49.0]), []);
    assert_eq!(dragged(&[]), []);
}
