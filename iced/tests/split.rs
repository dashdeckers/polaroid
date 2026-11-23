//! The split under a simulator: its edge is dragged, its side hidden, and its parts left alone.

use iced::event::Status;
use iced::widget::text;
use iced::{Event, Point, Widget, mouse};
use iced_test::simulator::{Simulator, simulator};
use polaroid_iced::split;

fn scene(is_shown: bool) -> impl Widget<()> {
    split(text("side"), text("main"))
        .width(200.0)
        .shown(is_shown)
}

/// Where the main part starts.
fn main_at(simulator: &mut Simulator<'_, ()>) -> f32 {
    simulator
        .find("main")
        .expect("the main part is shown")
        .bounds()
        .x
}

fn mouse(event: mouse::Event) -> Event {
    Event::Mouse(event)
}

#[test]
fn its_edge_is_dragged_to_resize_the_side() {
    let mut simulator = simulator(scene(true));
    assert_eq!(main_at(&mut simulator), 201.0);

    simulator.point_at(Point::new(200.0, 100.0));
    simulator.simulate([mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    let position = Point::new(300.0, 120.0);
    simulator.point_at(position);
    simulator.simulate([
        mouse(mouse::Event::CursorMoved { position }),
        mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]);
    assert_eq!(main_at(&mut simulator), 301.0);
}

#[test]
fn a_hidden_side_leaves_all_the_room_to_the_main_part() {
    let mut simulator = simulator(scene(false));
    assert_eq!(main_at(&mut simulator), 0.0);
    assert!(
        simulator
            .find("side")
            .is_err()
    );
}

#[test]
fn a_press_in_a_part_is_left_to_it() {
    // one that nothing takes starts a selection of text, so the split must not take it
    let mut simulator = simulator(scene(true));
    simulator.point_at(Point::new(400.0, 100.0));
    let taken = simulator.simulate([mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    assert_eq!(taken, [Status::Ignored]);
}

#[test]
fn a_press_beside_the_edge_goes_to_the_part_that_takes_it() {
    use iced::Fill;
    use iced::widget::button;

    // a button up to the edge of the side, as a scrollbar would be
    let side = button("side")
        .width(Fill)
        .on_press(());
    let mut simulator = simulator(split(side, text("main")).width(200.0));
    let beside = Point::new(198.0, 10.0);
    simulator.point_at(beside);
    simulator.simulate([
        mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        mouse(mouse::Event::CursorMoved {
            position: Point::new(300.0, 10.0),
        }),
        mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]);
    assert_eq!(main_at(&mut simulator), 201.0);
}
