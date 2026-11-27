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
    marks.update(Message::Pick(named(name)), pipeline);
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
    marks.update(Message::Pick(named("rate")), &mut pipeline);
    assert!(marks.update(Message::Toggle("money".to_owned()), &mut pipeline));
    tag(&mut marks, &mut pipeline, "rate", &["money"]);
    assert_eq!(pipeline.ui["tags"]["rate"], json!(["money"]));
    assert_eq!(pipeline.ui["colors"]["tags"]["money"], "#3b82c4");
    assert_eq!(of(&pipeline, &named("net")), ["money", "tax"].into());
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
            .picker(pipeline, &named(name))
            .unwrap();
        simulator(picker)
            .find("tax")
            .is_ok()
    };
    assert!(lists(&marks, &pipeline, "net"));
    assert!(
        marks
            .picker(&pipeline, &named("rate"))
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

/// How far down a view what says `said` is shown.
fn top(
    view: Element<'_, Message>,
    said: &str,
) -> f32 {
    simulator(view)
        .find(said)
        .unwrap()
        .bounds()
        .y
}

/// What a press and release of `button` at `at` asks of `view`.
fn pressed(
    view: Element<'_, Message>,
    at: iced::Point,
    button: iced::mouse::Button,
) -> Vec<Message> {
    use iced::mouse::Event::{ButtonPressed, ButtonReleased};

    let mut ui = simulator(view);
    ui.point_at(at);
    ui.simulate(
        [
            ButtonPressed(button),
            ButtonReleased(button),
        ]
        .map(iced::Event::Mouse),
    );
    ui.into_messages()
        .collect()
}

#[test]
fn a_right_press_on_a_tag_or_a_category_lays_the_palette_open_under_it_to_pick_its_colour() {
    use iced::mouse::Button;

    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["money"]);
    marks.update(Message::Close, &mut pipeline);
    let catalog = catalog(&["Plot"]);
    let name = simulator(list(&marks, &pipeline, &catalog))
        .find("money")
        .unwrap()
        .bounds();
    let plot = top(list(&marks, &pipeline, &catalog), "Plot");
    let right = |marks: &mut Marks, pipeline: &mut Pipeline| {
        let asked = pressed(
            list(marks, pipeline, &catalog),
            name.center(),
            Button::Right,
        );
        assert_eq!(asked.len(), 1, "{asked:?}");
        for message in asked {
            assert!(!marks.update(message, pipeline));
        }
    };
    right(&mut marks, &mut pipeline);
    assert_eq!(marks.painting, Some(Mark::Tag("money".to_owned())));

    // the palette covers nothing: what is under its line makes room for it
    let room = top(list(&marks, &pipeline, &catalog), "Plot") - plot;
    assert!(room >= dot::L, "{room}");
    // and a second right press closes it again
    right(&mut marks, &mut pipeline);
    assert_eq!(marks.painting, None);
    assert_eq!(top(list(&marks, &pipeline, &catalog), "Plot"), plot);

    // the second of its hues is pressed, under the line from where that starts
    right(&mut marks, &mut pipeline);
    let second = iced::Point::new(
        dot::L * 1.5 + spacing::XS,
        name.y + name.height + spacing::XS + dot::L / 2.0,
    );
    let asked = pressed(list(&marks, &pipeline, &catalog), second, Button::Left);
    assert!(
        matches!(
            &asked[..],
            [Message::Painted(
                Mark::Tag(_),
                _
            )]
        ),
        "{asked:?}"
    );
    for message in asked {
        assert!(marks.update(message, &mut pipeline));
    }
    assert_eq!(color(&pipeline.ui, "money"), hue::PALETTE[1]);
    assert_eq!(marks.painting, None);

    // a category keeps the colour that is picked for it apart from those of the tags
    let plot = || Mark::Category("Plot".to_owned());
    marks.update(Message::Paint(Some(plot())), &mut pipeline);
    assert!(marks.update(Message::Painted(plot(), hue::PALETTE[4]), &mut pipeline));
    assert_eq!(category(&pipeline.ui, &catalog, "Plot"), hue::PALETTE[4]);
    assert_eq!(
        pipeline.ui["colors"],
        json!({ "tags": { "money": "#cf6400" }, "categories": { "Plot": "#b750cf" } })
    );
}

#[test]
fn a_picker_of_tags_opens_with_no_palette_that_was_open_in_the_list() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    tag(&mut marks, &mut pipeline, "net", &["money"]);
    marks.update(Message::Close, &mut pipeline);
    let money = || Message::Paint(Some(Mark::Tag("money".to_owned())));
    let catalog = catalog(&["Plot"]);
    // how far down the first category is listed, and the field of a new tag in the picker
    let tops = |marks: &Marks, pipeline: &Pipeline| {
        let picker = marks
            .picker(pipeline, &named("net"))
            .map(|picker| top(picker, "new tag"));
        (top(list(marks, pipeline, &catalog), "Plot"), picker)
    };
    let (closed, _) = tops(&marks, &pipeline);
    marks.update(money(), &mut pipeline);
    let (open, _) = tops(&marks, &pipeline);
    assert!(open > closed);

    marks.update(Message::Pick(named("net")), &mut pipeline);
    let (listed, picker) = tops(&marks, &pipeline);
    assert_eq!(listed, closed);
    // one that is opened in the picker is open there alone
    marks.update(money(), &mut pipeline);
    let (listed, painted) = tops(&marks, &pipeline);
    assert_eq!(listed, closed);
    assert!(painted > picker);
}

/// The tags in use and the categories as they are listed, all of them.
fn list<'a>(
    marks: &'a Marks,
    pipeline: &'a Pipeline,
    catalog: &'a Catalog,
) -> Element<'a, Message> {
    marks.list(pipeline, catalog, |_| true, Help::default())
}

/// What a pipeline names `name`, as what carries tags.
fn named(name: &str) -> Carrier {
    Carrier::Name(name.to_owned())
}

#[test]
fn a_node_carries_its_tags_in_its_own_ui_and_those_of_the_names_it_uses_through_them() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    pipeline
        .nodes
        .push(crate::Node {
            id: "kept".to_owned(),
            block: "filter".to_owned(),
            inputs: vec![],
            args: json!({ "predicates": [crate::marker("net")] }),
            ui: Value::Null,
        });
    let node = || Carrier::Node("kept".to_owned());
    tag(&mut marks, &mut pipeline, "net", &["money"]);
    marks.update(Message::Pick(node()), &mut pipeline);
    marks.update(Message::Type("mine".to_owned()), &mut pipeline);
    assert!(marks.update(Message::Add, &mut pipeline));
    assert_eq!(pipeline.nodes[0].ui, json!({ "tags": ["mine"] }));
    assert_eq!(pipeline.ui["tags"], json!({ "net": ["money"] }));
    assert_eq!(of(&pipeline, &node()), ["mine"].into());
    assert_eq!(through(&pipeline, "kept"), ["money"].into());
    assert_eq!(all(&pipeline), ["mine", "money"].into());
    // a name of its id carries none of them
    assert!(of(&pipeline, &named("kept")).is_empty());

    // one that it carries itself too is its own, and a Save keeps the colour of each
    assert!(marks.update(Message::Toggle("money".to_owned()), &mut pipeline));
    assert!(through(&pipeline, "kept").is_empty());
    assert!(marks.update(Message::Toggle("money".to_owned()), &mut pipeline));
    tidy(&mut pipeline);
    assert_eq!(
        pipeline.ui["colors"]["tags"],
        json!({ "money": "#3b82c4", "mine": "#cf6400" })
    );

    // a tag is renamed and taken off on the nodes as on the names, which leaves no empty list
    let rename = Message::Rename("mine".to_owned(), "ours".to_owned());
    assert!(marks.update(rename, &mut pipeline));
    assert_eq!(pipeline.nodes[0].ui, json!({ "tags": ["ours"] }));
    assert!(marks.update(Message::Remove("ours".to_owned()), &mut pipeline));
    assert_eq!(pipeline.nodes[0].ui, json!({}));
}

#[test]
fn the_glass_rings_a_node_for_each_mark_picked_that_it_bears() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    let catalog = crate::Registry::with_polars().catalog();
    let node = |id: &str, block: &str, args: Value| crate::Node {
        id: id.to_owned(),
        block: block.to_owned(),
        inputs: vec![],
        args,
        ui: Value::Null,
    };
    let uses = json!({ "predicates": [crate::marker("net")] });
    pipeline.nodes = vec![
        node("kept", "filter", uses),
        node("cut", "limit", json!({})),
    ];
    tag(&mut marks, &mut pipeline, "net", &["money"]);
    marks.update(
        Message::Pick(Carrier::Node("kept".to_owned())),
        &mut pipeline,
    );
    marks.update(Message::Type("mine".to_owned()), &mut pipeline);
    marks.update(Message::Add, &mut pipeline);
    let rings = |marks: &Marks, pipeline: &Pipeline, node: usize| {
        marks.rings(pipeline, &catalog, &pipeline.nodes[node])
    };
    assert!(rings(&marks, &pipeline, 0).is_empty());

    // its menu opens over it, and lists the tags in use over the categories
    let pressed = |marks: &Marks, pipeline: &Pipeline, said: &str| {
        let mut ui = simulator(marks.glass(pipeline, &catalog, Help::default()));
        ui.click(said)
            .unwrap();
        ui.into_messages()
            .collect::<Vec<_>>()
    };
    let filters = catalog.blocks["filter"].category;
    assert!(!marks.update(Message::Glass, &mut pipeline));
    for said in ["money", "mine", filters] {
        for message in pressed(&marks, &pipeline, said) {
            // what it rings is no part of the pipeline
            assert!(!marks.update(message, &mut pipeline));
        }
    }
    // the tags by name, then the category: its own, and the one of a name that it uses
    let (mine, money) = (hue::PALETTE[1], hue::PALETTE[0]);
    let category = category(&pipeline.ui, &catalog, filters);
    assert_eq!(rings(&marks, &pipeline, 0), [mine, money, category]);
    let others = usize::from(catalog.blocks["limit"].category == filters);
    assert_eq!(rings(&marks, &pipeline, 1).len(), others);

    // it rings for a tag under its new name, and no longer once that is pressed again
    let rename = Message::Rename("money".to_owned(), "cash".to_owned());
    assert!(marks.update(rename, &mut pipeline));
    assert_eq!(rings(&marks, &pipeline, 0), [money, mine, category]);
    for message in pressed(&marks, &pipeline, "cash") {
        marks.update(message, &mut pipeline);
    }
    assert_eq!(rings(&marks, &pipeline, 0), [mine, category]);
    // nor for one that is taken off everything
    marks.update(Message::Remove("mine".to_owned()), &mut pipeline);
    assert_eq!(rings(&marks, &pipeline, 0), [category]);
}

#[test]
fn the_picker_that_is_open_is_that_of_what_it_was_opened_for_alone() {
    let (mut pipeline, mut marks) = (pipeline(), Marks::default());
    let node = Carrier::Node("net".to_owned());
    marks.update(Message::Pick(node.clone()), &mut pipeline);
    // a name is not the node of its id
    assert!(marks.is_picking(&node) && !marks.is_picking(&named("net")));
    marks.update(Message::Close, &mut pipeline);
    assert!(!marks.is_picking(&node));
}
