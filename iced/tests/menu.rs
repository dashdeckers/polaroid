//! The menu under a simulator: a group opens beside it, on the cursor's way or not, in its room.

mod common;

use common::simulator;
use iced::{Event, Point, mouse};
use iced_test::simulator::{Simulator, click};
use polaroid_iced::menu;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Message {
    Item(&'static str),
    Close,
}

/// A group of three items over one of a single item, at a point of the window.
fn scene(at: Point) -> impl iced::Widget<Message> {
    let items = |labels: &[&'static str]| {
        labels
            .iter()
            .map(|label| (label.to_string(), Message::Item(label)))
            .collect()
    };
    let groups = vec![
        ("A".to_owned(), items(&["a1", "a2", "a3"])),
        ("B".to_owned(), items(&["b1"])),
    ];
    menu(at, groups).on_close(Message::Close)
}

fn moved(
    simulator: &mut Simulator<'_, Message>,
    position: Point,
) {
    simulator.point_at(position);
    simulator.simulate([Event::Mouse(
        mouse::Event::CursorMoved { position },
    )]);
}

/// The items pressed by a press every few pixels along a line, from `x` on.
fn pressed_along(
    simulator: &mut Simulator<'_, Message>,
    x: usize,
    y: f32,
) -> Vec<&'static str> {
    for x in (x..1024).step_by(4) {
        simulator.point_at(Point::new(x as f32, y));
        simulator.simulate(click());
    }
    let mut items: Vec<_> = simulator
        .drain()
        .filter_map(|message| match message {
            Message::Item(item) => Some(item),
            Message::Close => None,
        })
        .collect();
    items.dedup();
    items
}

// the list of the groups is at 50, 50: its lines are 26 high below an edge of 4
// and it ends before 110, whatever the font: a press from there on is on no group, which opens it
const BESIDE: usize = 110;
const A: Point = Point::new(60.0, 67.0);
const B: Point = Point::new(60.0, 93.0);

#[test]
fn a_group_under_the_cursor_opens_its_items_level_with_it() {
    let mut simulator = simulator(scene(Point::new(50.0, 50.0)));
    moved(&mut simulator, A);
    assert_eq!(pressed_along(&mut simulator, BESIDE, A.y), ["a1"]);
    moved(&mut simulator, B);
    assert_eq!(pressed_along(&mut simulator, BESIDE, B.y), ["b1"]);
}

#[test]
fn the_cursor_crosses_another_group_on_its_way_to_the_items() {
    let mut simulator = simulator(scene(Point::new(50.0, 50.0)));
    moved(&mut simulator, A);
    // down and to the right is towards the items of the first group, over the second
    moved(&mut simulator, Point::new(80.0, B.y));
    assert_eq!(pressed_along(&mut simulator, BESIDE, A.y), ["a1"]);

    // straight down is not
    moved(&mut simulator, A);
    moved(&mut simulator, B);
    assert_eq!(pressed_along(&mut simulator, BESIDE, B.y), ["b1"]);
}

#[test]
fn both_lists_lie_inside_the_room() {
    // asked for in the far corner, where neither list has room to the right or below
    let mut simulator = simulator(scene(Point::new(1000.0, 750.0)));
    // the two groups end at the bottom, 768, so the first is at 708 + 4
    moved(&mut simulator, Point::new(1014.0, 725.0));
    // and its three items end there too, so the first is at 768 - 86 + 4
    assert_eq!(pressed_along(&mut simulator, 0, 699.0), ["a1"]);
    assert_eq!(pressed_along(&mut simulator, 0, 751.0), ["a3"]);
}

#[test]
fn a_list_of_items_with_icons_is_wider_by_their_plates() {
    // the faces are loaded before a label is measured
    drop(simulator(scene(Point::ORIGIN)));
    // how far right a press still picks the item of the second group
    let end = |icons: bool| {
        let groups = vec![("B".to_owned(), vec![("b1".to_owned(), Message::Item("b1"))])];
        let scene = match icons {
            true => menu(Point::new(50.0, 50.0), groups).icons(|_| None),
            false => menu(Point::new(50.0, 50.0), groups),
        };
        let mut simulator = simulator(scene);
        moved(&mut simulator, A);
        (BESIDE..400)
            .rev()
            .find(|x| {
                simulator.point_at(Point::new(*x as f32, A.y));
                simulator.simulate(click());
                simulator
                    .drain()
                    .any(|message| message == Message::Item("b1"))
            })
            .expect("the item is beside its group")
    };
    // a plate of 20 and the 8 between it and the label
    assert_eq!(end(true), end(false) + 28);
}
