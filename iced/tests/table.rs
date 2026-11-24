//! The table under a simulator: its least height, and cells selected, scrolled to and copied.

use iced::event::Status;
use iced::keyboard::{self, Key, Modifiers};
use iced::mouse::{self, ScrollDelta};
use iced::widget::canvas;
use iced::{Event, Point, Widget};
use iced_test::simulator::{Simulator, press_key, simulator};
use polaroid_iced::{Asked, Heading, Table, table};

/// A table of this many columns and rows, in which a cell says its row and its column.
fn cells<'a>(
    columns: usize,
    rows: usize,
) -> Table<'a> {
    let names = ["a", "b", "c", "d", "e"];
    let headings = names[..columns]
        .iter()
        .map(|name| Heading {
            name,
            kind: "text".to_owned(),
            is_numeric: false,
        })
        .collect();
    table(headings).rows(rows, |row, column| format!("{row}:{column}"))
}

/// The table at the corner of the window, 400 wide: three of its columns start in view.
fn scene<'a>(table: Table<'a>) -> impl Widget<Asked> + use<'a> {
    let height = table.least();
    canvas(table)
        .width(400)
        .height(height)
}

/// The middle of a cell in view: a column is 140 wide, a row 20 high below a header of 38.
fn cell(
    row: usize,
    column: usize,
) -> Point {
    // right of the numbers of the rows, which are less than 60 wide
    Point::new(100.0 + 140.0 * column as f32, 48.0 + 20.0 * row as f32)
}

/// Presses at a point, moves to another and lets go there.
fn drag(
    simulator: &mut Simulator<'_, Asked>,
    from: Point,
    to: Point,
) {
    let button = mouse::Button::Left;
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

/// What Ctrl+C copies.
fn copied(simulator: &mut Simulator<'_, Asked>) -> Vec<String> {
    let mut press = press_key(Key::Character("c".into()), None);
    if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = &mut press {
        *modifiers = Modifiers::COMMAND;
    }
    simulator.simulate([press]);
    simulator
        .drain()
        .filter_map(|asked| match asked {
            Asked::Copy(text) => Some(text),
            Asked::Row(_) => None,
        })
        .collect()
}

/// Turns the wheel this many notches over the first cell, and tells whether the table took it.
fn wheel(
    simulator: &mut Simulator<'_, Asked>,
    notches: f32,
) -> Status {
    simulator.point_at(cell(0, 0));
    let delta = ScrollDelta::Lines { x: 0.0, y: notches };
    simulator.simulate([Event::Mouse(
        mouse::Event::WheelScrolled { delta },
    )])[0]
}

#[test]
fn it_is_as_tall_as_its_header_and_up_to_five_rows_at_the_least() {
    assert_eq!(cells(1, 0).least(), 38.0 + 20.0);
    assert_eq!(cells(1, 3).least(), 38.0 + 3.0 * 20.0);
    assert_eq!(cells(1, 40).least(), 38.0 + 5.0 * 20.0);
    // without rows it is its header alone
    assert_eq!(table(Vec::new()).least(), 38.0);
}

#[test]
fn selected_cells_are_copied_under_their_names_and_one_cell_alone() {
    let mut simulator = simulator(scene(cells(3, 40)));
    drag(&mut simulator, cell(0, 0), cell(1, 1));
    assert_eq!(copied(&mut simulator), ["a\tb\n0:0\t0:1\n1:0\t1:1"]);

    // dragged back and up it is the same cells
    drag(&mut simulator, cell(1, 1), cell(0, 0));
    assert_eq!(copied(&mut simulator), ["a\tb\n0:0\t0:1\n1:0\t1:1"]);

    drag(&mut simulator, cell(2, 1), cell(2, 1));
    assert_eq!(copied(&mut simulator), ["2:1"]);

    // a press beside the table takes Ctrl+C from it
    let beside = Point::new(700.0, 400.0);
    drag(&mut simulator, beside, beside);
    assert!(copied(&mut simulator).is_empty());
}

#[test]
fn the_wheel_scrolls_three_rows_a_notch_and_sideways_with_shift() {
    let mut simulator = simulator(scene(cells(5, 40)));
    assert_eq!(wheel(&mut simulator, -1.0), Status::Captured);
    drag(&mut simulator, cell(0, 0), cell(0, 0));
    assert_eq!(copied(&mut simulator), ["3:0"]);

    // not past the first row
    wheel(&mut simulator, 5.0);
    drag(&mut simulator, cell(0, 0), cell(0, 0));
    assert_eq!(copied(&mut simulator), ["0:0"]);

    // two notches with shift are 120 to the right, which puts the second column under the first
    let shift = keyboard::Event::ModifiersChanged(Modifiers::SHIFT);
    simulator.simulate([Event::Keyboard(shift)]);
    wheel(&mut simulator, -2.0);
    drag(&mut simulator, cell(0, 0), cell(0, 0));
    assert_eq!(copied(&mut simulator), ["0:1"]);
}

#[test]
fn a_table_that_is_all_in_view_leaves_the_wheel_to_what_it_lies_on() {
    let mut simulator = simulator(scene(cells(2, 3)));
    assert_eq!(wheel(&mut simulator, -1.0), Status::Ignored);
}

#[test]
fn a_press_on_a_cell_asks_for_its_row() {
    let mut simulator = simulator(scene(cells(3, 40)));
    wheel(&mut simulator, -1.0);
    drag(&mut simulator, cell(1, 2), cell(2, 2));
    // the header is no row
    let header = Point::new(100.0, 10.0);
    drag(&mut simulator, header, header);
    let asked: Vec<Asked> = simulator
        .drain()
        .collect();
    assert_eq!(asked, [Asked::Row(4)]);
}
