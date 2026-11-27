use serde_json::json;

use super::*;
use crate::gui::simulator;

/// A pipeline that names two expressions and a value.
fn pipeline() -> Pipeline {
    let mut pipeline = Pipeline::default();
    for name in ["net", "gross"] {
        pipeline
            .expressions
            .insert(name.to_owned(), Value::Null);
    }
    pipeline
        .values
        .insert("rate".to_owned(), 0.2.into());
    pipeline
}

/// Opens the picker of the tags of `name`, and types each of `tags` in it for a tag of its own.
fn tag(
    marks: &mut Marks,
    pipeline: &mut Pipeline,
    name: &str,
    tags: &[&str],
) {
    marks.update(Message::Pick(name.to_owned()), pipeline);
    for tag in tags {
        marks.update(Message::Type((*tag).to_owned()), pipeline);
        assert!(marks.update(Message::Add, pipeline));
    }
}

#[test]
fn a_tag_is_typed_for_a_name_and_ticked_for_another() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    // with no picker open, nothing is picked
    assert!(!marks.update(Message::Toggle("money".to_owned()), &mut pipeline));
    assert!(!marks.update(Message::Add, &mut pipeline));
    assert!(
        pipeline
            .ui
            .is_null()
    );

    // a new one takes the first hue of the palette, and the next the one after
    tag(&mut marks, &mut pipeline, "net", &[" money ", "tax"]);
    assert!(!marks.update(Message::Add, &mut pipeline));
    assert_eq!(
        pipeline.ui,
        json!({
            "colors": { "tags": { "money": "#3b82c4", "tax": "#cf6400" } },
            "tags": { "net": ["money", "tax"] },
        })
    );
    assert_eq!(color(&pipeline.ui, "tax"), hue::PALETTE[1]);
    assert_eq!(color(&pipeline.ui, "none"), hue::NEUTRAL);

    // one that is there is ticked, and typed again it is the same one
    marks.update(Message::Pick("rate".to_owned()), &mut pipeline);
    assert!(marks.update(Message::Toggle("money".to_owned()), &mut pipeline));
    tag(&mut marks, &mut pipeline, "rate", &["money"]);
    assert_eq!(pipeline.ui["tags"]["rate"], json!(["money"]));
    assert_eq!(pipeline.ui["colors"]["tags"]["money"], "#3b82c4");
    assert_eq!(of(&pipeline, "net"), ["money", "tax"].into());
    assert_eq!(all(&pipeline), ["money", "tax"].into());
}

#[test]
fn a_tag_that_nothing_carries_is_none_but_in_the_picker_it_was_unticked_in() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["money", "tax"]);
    assert!(marks.update(Message::Toggle("tax".to_owned()), &mut pipeline));
    assert_eq!(all(&pipeline), ["money"].into());
    assert_eq!(pipeline.ui["tags"]["net"], json!(["money"]));

    // its picker lists it still, to tick it again in the colour it had
    let lists = |marks: &Marks, pipeline: &Pipeline, name: &str| {
        let picker = marks
            .picker(pipeline, name)
            .unwrap();
        simulator(picker)
            .find("tax")
            .is_ok()
    };
    assert!(lists(&marks, &pipeline, "net"));
    assert!(
        marks
            .picker(&pipeline, "rate")
            .is_none()
    );
    assert_eq!(pipeline.ui["colors"]["tags"]["tax"], "#cf6400");

    // another picker does not, and a new tag takes the hue that the fewest have
    tag(&mut marks, &mut pipeline, "rate", &["raw"]);
    assert!(!lists(&marks, &pipeline, "rate"));
    assert_eq!(pipeline.ui["colors"]["tags"]["raw"], "#cf6400");

    // nor does the file hold its colour
    tidy(&mut pipeline);
    assert_eq!(
        pipeline.ui["colors"]["tags"],
        json!({ "money": "#3b82c4", "raw": "#cf6400" })
    );
}

#[test]
fn the_tags_of_a_name_go_along_as_it_is_renamed_and_go_with_it() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["money"]);
    renamed(&mut pipeline, "net", Some("nett"));
    assert_eq!(pipeline.ui["tags"], json!({ "nett": ["money"] }));
    // one that has none carries none under its new name
    renamed(&mut pipeline, "gross", Some("all"));
    assert_eq!(pipeline.ui["tags"], json!({ "nett": ["money"] }));

    renamed(&mut pipeline, "nett", None);
    assert_eq!(pipeline.ui["tags"], json!({}));
}

#[test]
fn a_tidy_leaves_in_the_ui_what_is_in_use_alone() {
    let mut pipeline = pipeline();
    pipeline.ui = json!({
        "tags": { "gone": ["old"], "net": ["money", 3] },
        "colors": { "tags": { "old": "#111111", "money": "#222222" }, "categories": {} },
        "sorted": [],
        "else": 1,
    });
    assert_eq!(all(&pipeline), ["money"].into());
    tidy(&mut pipeline);
    assert_eq!(
        pipeline.ui,
        json!({
            "tags": { "net": ["money", 3] },
            "colors": { "tags": { "money": "#222222" } },
            "else": 1,
        })
    );

    // and with nothing in use there is no `ui`, whatever it was
    for ui in [
        json!({ "tags": { "gone": ["old"] } }),
        json!(3),
        Value::Null,
    ] {
        pipeline.ui = ui;
        tidy(&mut pipeline);
        assert!(
            pipeline
                .ui
                .is_null()
        );
    }
}

#[test]
fn a_tag_is_renamed_wherever_it_is_carried_unless_another_has_the_name() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["money", "tax"]);
    tag(&mut marks, &mut pipeline, "rate", &["money"]);
    let mut rename = |pipeline: &mut Pipeline, tag: &str, new: &str| {
        marks.update(Message::Rename(tag.to_owned(), new.to_owned()), pipeline)
    };

    // the spaces around what is typed are no part of the name
    assert!(rename(&mut pipeline, "money", " cash "));
    assert_eq!(
        pipeline.ui,
        json!({
            "colors": { "tags": { "tax": "#cf6400", "cash": "#3b82c4" } },
            "tags": { "net": ["cash", "tax"], "rate": ["cash"] },
        })
    );
    // the name of another, with spaces or without, none, or its own is no other name for it
    for refused in [
        "tax", " tax", "", "  ", "cash",
    ] {
        assert!(!rename(&mut pipeline, "cash", refused), "{refused:?}");
    }
    assert_eq!(all(&pipeline), ["cash", "tax"].into());
}

#[test]
fn a_tag_is_taken_off_everything_from_the_list_of_those_in_use() {
    let catalog = catalog(&[]);
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["money", "tax"]);
    tag(&mut marks, &mut pipeline, "rate", &["money"]);
    let lists = |marks: &Marks, pipeline: &Pipeline, shown: &str, text: &str| {
        let shows = |text: &str| text.contains(shown);
        let mut ui = simulator(marks.list(pipeline, &catalog, shows, Help::default()));
        ui.find(text)
            .is_ok()
    };
    assert!(lists(&marks, &pipeline, "", "money") && lists(&marks, &pipeline, "", "tax"));
    // how many names carry each is said beside it
    assert!(lists(&marks, &pipeline, "", "2"));
    // and what is searched for narrows them
    assert!(lists(&marks, &pipeline, "o", "money") && !lists(&marks, &pipeline, "o", "tax"));

    assert!(marks.update(Message::Remove("money".to_owned()), &mut pipeline));
    assert_eq!(pipeline.ui["tags"], json!({ "net": ["tax"] }));
    assert!(!lists(&marks, &pipeline, "", "money") && lists(&marks, &pipeline, "", "tax"));
}

#[test]
fn enter_gives_a_tag_no_name_that_another_has_as_it_is_typed() {
    let catalog = catalog(&[]);
    use iced_test::selector::{Candidate, Target};

    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["ta", "tax"]);
    let field = |value: &'static str| {
        move |candidate: Candidate<'_>| {
            matches!(&candidate, Candidate::TextInput { state, .. } if state.text() == value)
                .then(|| Target::from(candidate))
        }
    };
    for (typed, is_given) in [("x", false), ("xes", true)] {
        let mut ui = simulator(marks.list(&pipeline, &catalog, |_| true, Help::default()));
        ui.click(field("ta"))
            .unwrap();
        ui.typewrite(typed);
        ui.tap_key(iced::keyboard::key::Named::Enter);
        assert_eq!(ui.drain().count(), usize::from(is_given), "{typed}");
    }
}

/// A catalog of these categories alone.
fn catalog(categories: &[&'static str]) -> Catalog {
    Catalog {
        categories: categories.to_vec(),
        ..crate::Registry::default().catalog()
    }
}

#[test]
fn the_categories_are_listed_under_the_tags_each_in_the_hue_at_its_place_from_the_last() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["money"]);
    let catalog = catalog(&["Filter", "Plot"]);
    let top = |shown: &str, text: &str| {
        let shows = |text: &str| text.contains(shown);
        let mut ui = simulator(marks.list(&pipeline, &catalog, shows, Help::default()));
        ui.find(text)
            .ok()
            .map(|found| found.bounds().y)
    };
    assert!(top("", "Filter") < top("", "Plot"));
    // and what is searched for narrows them as it does the tags
    assert!(top("o", "Plot").is_some() && top("o", "Filter").is_none());

    assert_eq!(category(&pipeline.ui, &catalog, "Filter"), hue::PALETTE[9]);
    assert_eq!(category(&pipeline.ui, &catalog, "Plot"), hue::PALETTE[8]);
    // one that has a colour of its own keeps it apart from that of a tag of its name
    pipeline.ui = json!({
        "colors": { "categories": { "Plot": "#000000" }, "tags": { "Plot": "#ffffff" } },
    });
    assert_eq!(category(&pipeline.ui, &catalog, "Plot"), Color::BLACK);
    assert_eq!(color(&pipeline.ui, "Plot"), Color::WHITE);
}
