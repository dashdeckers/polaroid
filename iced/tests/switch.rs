//! A switch under a simulator: a press chooses a stop, and presses the button its thumb is there.

mod common;

use common::simulator;
use iced::widget::text;
use iced::{Event, Point, mouse};
use iced_test::Simulator;
use polaroid_iced::switch;
use polaroid_iced::theme::mode;

#[derive(Debug, Clone, PartialEq)]
enum Asked {
    Choose(usize),
    Press,
}

/// A switch at the stop of `chosen`, of two stops and a third that is a button.
fn scene(chosen: usize) -> Simulator<'static, Asked> {
    let button = match chosen {
        2 => "Go",
        _ => "Third",
    };
    let switch = switch(chosen)
        .stop(text("First"), Asked::Choose(0), mode::live)
        .stop(text("Second"), Asked::Choose(1), mode::delay)
        .button(text(button), Asked::Choose(2), Asked::Press, mode::manual);
    simulator(switch)
}

/// What a click on what reads `label` asks for, at the stop of `chosen`.
fn clicked(
    chosen: usize,
    label: &str,
) -> Vec<Asked> {
    let mut ui = scene(chosen);
    ui.click(label)
        .unwrap();
    ui.into_messages()
        .collect()
}

#[test]
fn a_press_on_another_stop_chooses_it() {
    assert_eq!(clicked(0, "Second"), [Asked::Choose(1)]);
    assert_eq!(clicked(1, "First"), [Asked::Choose(0)]);
    assert_eq!(clicked(0, "Third"), [Asked::Choose(2)]);
}

#[test]
fn a_press_on_the_chosen_stop_asks_nothing_unless_it_is_a_button() {
    assert_eq!(clicked(0, "First"), []);
    assert_eq!(clicked(1, "Second"), []);
    assert_eq!(clicked(2, "Go"), [Asked::Press]);
}

#[test]
fn a_press_let_go_off_its_stop_asks_nothing() {
    let mut ui = scene(0);
    let second = ui
        .find("Second")
        .unwrap()
        .bounds()
        .center();
    ui.point_at(second);
    ui.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    ui.point_at(Point::new(second.x, second.y + 200.0));
    ui.simulate([Event::Mouse(
        mouse::Event::ButtonReleased(mouse::Button::Left),
    )]);
    assert_eq!(
        ui.into_messages()
            .collect::<Vec<_>>(),
        []
    );
}

#[test]
fn the_thumb_dragged_across_goes_to_the_stop_it_is_let_go_nearest() {
    let mut ui = scene(0);
    let [first, third] = ["First", "Third"].map(|label| {
        ui.find(label)
            .unwrap()
            .bounds()
            .center()
    });
    let move_to = |ui: &mut Simulator<'static, Asked>, position: Point| {
        ui.point_at(position);
        ui.simulate([Event::Mouse(
            mouse::Event::CursorMoved { position },
        )]);
    };
    move_to(&mut ui, first);
    ui.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    move_to(&mut ui, Point::new(first.x + 20.0, first.y));
    // a little short of the last stop
    move_to(&mut ui, Point::new(third.x - 10.0, third.y));
    ui.simulate([Event::Mouse(
        mouse::Event::ButtonReleased(mouse::Button::Left),
    )]);
    assert_eq!(
        ui.into_messages()
            .collect::<Vec<_>>(),
        [Asked::Choose(2)]
    );
}
