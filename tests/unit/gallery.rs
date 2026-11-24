use iced_test::simulator;

use super::*;
use crate::polars::prelude::*;

/// An image as wide as this.
fn image(width: u32) -> Option<Vec<u8>> {
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="3"/>"#);
    Some(svg.into_bytes())
}

/// Four rows of images, of which one is none, beside a text, a number, a list and a constant.
fn gallery() -> Gallery {
    let lists = Series::new("drawn".into(), [1, 2, 3, 4])
        .implode()
        .unwrap()
        .into_series()
        .new_from_index(0, 4);
    let frame = df!(
        "name" => ["b", "a", "b", "a"],
        "size" => [10, 10, 2, 10],
        "all" => ["same"; 4],
        "plot" => [image(4), image(5), None, image(6)],
    )
    .unwrap()
    .hstack(&[lists.into()])
    .unwrap();
    Gallery::of(&frame)
}

#[test]
fn a_frame_without_images_is_no_gallery() {
    let frame = df!("name" => ["a"], "bytes" => [Some(b"abc".to_vec())]).unwrap();
    let gallery = Gallery::of(&frame);
    assert!(
        gallery
            .view(&State::default(), 300.0, true, Help::default())
            .is_none()
    );
}

#[test]
fn the_columns_of_more_than_one_plain_value_are_its_attributes_in_order() {
    let gallery = gallery();
    let attributes: Vec<_> = gallery
        .attributes
        .iter()
        .map(|attribute| {
            (
                attribute
                    .name
                    .as_str(),
                attribute.is_numeric,
                &attribute.values,
            )
        })
        .collect();
    let (names, sizes) = (
        vec!["a".to_owned(), "b".to_owned()],
        vec![
            "2".to_owned(),
            "10".to_owned(),
        ],
    );
    assert_eq!(
        attributes,
        [
            ("name", false, &names),
            ("size", true, &sizes)
        ]
    );
}

#[test]
fn the_rows_left_have_the_value_picked_of_every_attribute() {
    let gallery = gallery();
    let mut state = State::default();
    assert_eq!(gallery.rows(&state), [0, 1, 2, 3]);

    // the second of the names, which is b
    state.update(Message::Pick("name".to_owned(), Some(1)), &gallery);
    assert_eq!(gallery.rows(&state), [0, 2]);
    // and the first of the sizes, which is 2
    state.row = 1;
    state.update(Message::Pick("size".to_owned(), Some(0)), &gallery);
    assert_eq!(gallery.rows(&state), [2]);
    assert_eq!(state.row, 0);

    state.update(Message::Pick("name".to_owned(), None), &gallery);
    assert_eq!(gallery.rows(&state), [2]);
    // a value that a Run since took away picks nothing
    state.update(Message::Pick("size".to_owned(), Some(7)), &gallery);
    assert_eq!(gallery.rows(&state), [0, 1, 2, 3]);
}

#[test]
fn it_says_which_row_of_those_left_it_shows_when_it_is_browsed() {
    let gallery = gallery();
    let mut state = State::default();
    let says = |state: &State, is_browsed: bool, text: &str| {
        let view = gallery
            .view(state, 300.0, is_browsed, Help::default())
            .expect("it has images");
        simulator(view)
            .find(text)
            .is_ok()
    };
    assert!(says(&state, true, "1 of 4"));
    assert!(says(&state, true, "name"));
    // small, it is an image and no more
    assert!(!says(&state, false, "1 of 4"));

    state.update(Message::Row(9), &gallery);
    assert!(says(&state, true, "4 of 4"));
    state.update(Message::Pick("size".to_owned(), Some(1)), &gallery);
    assert!(says(&state, true, "1 of 3"));
    assert!(says(&state, true, "10"));
    // one row left is not browsed, and none is said to be none
    state.update(Message::Pick("size".to_owned(), Some(0)), &gallery);
    assert!(!says(&state, true, "1 of 1"));
    assert!(!says(&state, true, "no row has these values"));
    state.update(Message::Pick("name".to_owned(), Some(0)), &gallery);
    assert!(says(&state, true, "no row has these values"));
}

#[test]
fn a_row_is_shown_whatever_was_picked_and_the_rows_left_are_stepped_through_in_a_round() {
    let gallery = gallery();
    let mut state = State::default();
    state.update(Message::Pick("name".to_owned(), Some(1)), &gallery);
    // the third row is the second of those of its name
    state.update(Message::Show(2), &gallery);
    assert_eq!(gallery.shown(&state), Some(2));
    assert_eq!(gallery.rows(&state), [0, 2]);
    state.update(Message::Step, &gallery);
    assert_eq!(gallery.shown(&state), Some(0));

    // one of another name is shown among all
    state.update(Message::Show(3), &gallery);
    assert_eq!(gallery.shown(&state), Some(3));
    assert_eq!(
        gallery
            .rows(&state)
            .len(),
        4
    );
    state.update(Message::Step, &gallery);
    assert_eq!(gallery.shown(&state), Some(0));
}

#[test]
fn the_image_shown_is_exported_as_it_lies_in_the_frame() {
    let frame = df!("name" => ["a", "b"], "plot" => [image(4), image(5)]).unwrap();
    let gallery = Gallery::of(&frame);
    let mut state = State::default();
    state.update(Message::Row(1), &gallery);
    let (bytes, kind) = gallery
        .export(&state, &frame)
        .expect("an image is shown");
    assert_eq!((Some(bytes.to_vec()), kind), (image(5), "svg"));
}

#[test]
fn an_image_says_how_wide_and_tall_it_is() {
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x05\0\0\0\x03\xc0";
    assert_eq!(size(png), Some((1280.0, 960.0)));
    assert_eq!(size(&image(4).unwrap()), Some((4.0, 3.0)));
    let quoted = b"<svg xmlns='w' stroke-width='9' width='640' height='480' viewBox='0 0 1 1'>";
    assert_eq!(size(quoted), Some((640.0, 480.0)));
    assert_eq!(size(b"<svg>"), None);
    assert_eq!(size(b"%PDF"), None);
}
