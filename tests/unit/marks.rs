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
