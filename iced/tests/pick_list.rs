//! A pick list under a simulator: closed as wide as what it shows, open as wide as its options.

use iced::widget::container;
use iced::{Fill, Point, Rectangle, Widget};
use iced_test::simulator::click;
use iced_test::{Error, Simulator, simulator};
use polaroid_iced::{PickList, pick_list};

const SHORT: &str = "a";
const LONG: &str = "one that is a good deal longer than that";

/// A list of a short and a long option that shows `selected`, and asks for what is picked.
fn list(selected: &'static str) -> impl Widget<&'static str> {
    pick_list(Some(selected), vec![SHORT, LONG], ToString::to_string).on_select(|picked| picked)
}

/// Where the list that shows `selected` is.
fn bounds(
    simulator: &mut Simulator<&'static str>,
    selected: &str,
) -> Result<Rectangle, Error> {
    Ok(simulator
        .find(selected)?
        .bounds())
}

/// Opens the list that is at `bounds` and presses its second option, this far from its left.
fn pick_second(
    simulator: &mut Simulator<&'static str>,
    bounds: Rectangle,
    x: f32,
) {
    simulator.point_at(bounds.center());
    simulator.simulate(click());
    // the options are as tall as the list, and under it
    let second = bounds.y + bounds.height * 2.5;
    simulator.point_at(Point::new(bounds.x + x, second));
    simulator.simulate(click());
}

#[test]
fn closed_it_is_as_wide_as_what_it_shows() -> Result<(), Error> {
    let short = bounds(&mut simulator(list(SHORT)), SHORT)?;
    let long = bounds(&mut simulator(list(LONG)), LONG)?;
    assert!(short.width < 60.0, "{short:?}");
    assert!(long.width > 250.0, "{long:?}");
    Ok(())
}

#[test]
fn open_it_is_as_wide_as_its_widest_option() -> Result<(), Error> {
    let mut simulator = simulator(list(SHORT));
    let bounds = bounds(&mut simulator, SHORT)?;
    // far to the right of the closed list, where the long option reaches
    pick_second(&mut simulator, bounds, 250.0);
    let picked: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(picked, [LONG]);
    Ok(())
}

#[test]
fn a_press_beside_the_open_list_closes_it() -> Result<(), Error> {
    let mut simulator = simulator(list(SHORT));
    let bounds = bounds(&mut simulator, SHORT)?;
    pick_second(&mut simulator, bounds, 600.0);
    // and what was under it is not picked by a press there now
    simulator.point_at(Point::new(bounds.x + 10.0, bounds.y + bounds.height * 2.5));
    simulator.simulate(click());
    assert_eq!(
        simulator
            .into_messages()
            .count(),
        0
    );
    Ok(())
}

#[test]
fn at_the_edge_of_the_window_it_opens_to_the_left() -> Result<(), Error> {
    let at_right = container(list(SHORT))
        .width(Fill)
        .align_right(Fill);
    let mut simulator = simulator(at_right);
    let bounds = bounds(&mut simulator, SHORT)?;
    // far to the left of the closed list
    pick_second(&mut simulator, bounds, -200.0);
    let picked: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(picked, [LONG]);
    Ok(())
}

#[test]
fn a_drag_down_from_its_handle_steps_through_the_options() -> Result<(), Error> {
    use iced::{Event, mouse, window};
    use polaroid_iced::HANDLE;

    let options = vec!["a", "b", "c", "d"];
    let stepped = pick_list(Some("b"), options, ToString::to_string)
        .on_select(|picked| picked)
        .stepped();
    let mut simulator = simulator(stepped);
    let at = simulator
        .find(HANDLE)?
        .bounds()
        .center();
    simulator.point_at(at);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: at }),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
    ]);
    // from `b`, which it shows until it is built anew: one on, one back, and off the end twice
    for (i, x) in [6.0, 0.0, 60.0, 90.0]
        .into_iter()
        .enumerate()
    {
        let position = Point::new(at.x, at.y + x);
        simulator.point_at(position);
        simulator.simulate([
            Event::Mouse(mouse::Event::CursorMoved { position }),
            Event::Window(window::Event::RedrawRequested(
                iced::time::Instant::now() + std::time::Duration::from_secs(i as u64),
            )),
        ]);
    }
    let picked: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(picked, ["c", "a", "d", "d"]);
    Ok(())
}

/// A list of four that shows `banana`, of which `cherry` cannot be picked.
fn fruits() -> PickList<'static, &'static str, &'static str> {
    let options = vec![
        "apple",
        "banana",
        "cherry",
        "blueberry",
    ];
    pick_list(Some("banana"), options, ToString::to_string)
        .on_select(|picked| picked)
        .disabled(|option| (*option == "cherry").then(|| "eaten".to_owned()))
}

#[test]
fn an_option_that_cannot_be_picked_is_listed_and_a_press_on_it_does_nothing() -> Result<(), Error> {
    let mut simulator = simulator(fruits());
    simulator.click("banana")?;
    // listed in its place, and pressed it keeps the list open, where another is picked
    simulator.click("cherry")?;
    simulator.click("blueberry")?;
    let picked: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(picked, ["blueberry"]);
    Ok(())
}

#[test]
fn a_step_goes_past_an_option_that_cannot_be_picked() -> Result<(), Error> {
    use iced::{Event, mouse, window};
    use polaroid_iced::HANDLE;

    let mut simulator = simulator(fruits().stepped());
    let at = simulator
        .find(HANDLE)?
        .bounds()
        .center();
    let down = Point::new(at.x, at.y + 6.0);
    simulator.point_at(at);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: at }),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
    ]);
    simulator.point_at(down);
    simulator.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: down }),
        Event::Window(window::Event::RedrawRequested(iced::time::Instant::now())),
    ]);
    let picked: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(picked, ["blueberry"]);
    Ok(())
}

#[test]
fn a_long_list_opens_scrolled_to_what_is_selected() -> Result<(), Error> {
    let options: Vec<String> = (0..80)
        .map(|n| format!("option {n}"))
        .collect();
    let list =
        pick_list(Some("option 70".to_owned()), options, String::clone).on_select(|picked| picked);
    let mut simulator = simulator(list);
    simulator.click("option 70")?;
    simulator.click("option 71")?;
    let picked: Vec<_> = simulator
        .into_messages()
        .collect();
    assert_eq!(picked, ["option 71"]);
    Ok(())
}
