use iced_test::simulator;

use super::known::Status;
use super::*;

/// A chain of `limit`s below a scan, each reading the one before.
fn chain(len: usize) -> Editor {
    let registry = Arc::new(Registry::with_polars());
    let nodes = (0..len)
        .map(|n| Node {
            id: format!("n{n}"),
            block: match n {
                0 => "scan_parquet",
                _ => "limit",
            }
            .to_owned(),
            inputs: match n {
                0 => vec![],
                _ => vec![format!("n{}", n - 1)],
            },
            args: match n {
                0 => serde_json::json!({ "path": "a.parquet" }),
                _ => serde_json::json!({ "n": 5 }),
            },
            ui: Value::Null,
        })
        .collect();
    let mut pipeline = Pipeline {
        nodes,
        ..Pipeline::default()
    };
    layout::layout(&mut pipeline);
    Editor::new(registry, "p.json".to_owned(), pipeline)
}

fn status(editor: &Editor) -> Vec<Status> {
    editor
        .pipeline
        .nodes
        .iter()
        .map(|node| {
            editor
                .known
                .status(&node.id)
        })
        .collect()
}

#[test]
fn an_edit_forgets_the_node_and_its_readers() {
    use Status::*;

    let mut editor = chain(3);
    assert_eq!(status(&editor), [Unknown, Unknown, Unknown]);

    let checks = ["n0", "n1", "n2"]
        .map(|id| (id.to_owned(), Ok(vec![])))
        .into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    assert_eq!(status(&editor), [Checked, Checked, Checked]);

    let _ = editor.update(Message::Ran(vec!["n1".to_owned()], Ok(vec![])));
    assert_eq!(status(&editor), [Ran, Ran, Checked]);

    let wire = Edit::Wire(0, "n0".to_owned());
    let _ = editor.update(Message::Edit("n1".to_owned(), wire));
    assert_eq!(status(&editor), [Ran, Unknown, Unknown]);

    let _ = editor.update(Message::Ran(vec!["n2".to_owned()], Err("no".to_owned())));
    assert_eq!(status(&editor), [Ran, Unknown, Failed]);
}

#[test]
fn a_node_wired_badly_is_rejected() {
    use Status::*;

    let mut editor = chain(2);
    let wire = Edit::Wire(0, "gone".to_owned());
    let _ = editor.update(Message::Edit("n1".to_owned(), wire));
    assert_eq!(status(&editor), [Unknown, Rejected]);
}

#[test]
fn a_press_on_its_plus_lays_a_card_open() {
    let mut editor = chain(1);
    let node = |editor: &Editor| {
        editor
            .pipeline
            .nodes[0]
            .clone()
    };
    assert!(!layout::is_open(&node(&editor)));

    let mut ui = simulator(editor.view());
    ui.click("+")
        .unwrap();
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert!(layout::is_open(&node(&editor)));
    assert!(editor.is_dirty);

    let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Toggle));
    assert!(!layout::is_open(&node(&editor)));
}

#[test]
fn an_id_is_renamed_on_enter_and_its_readers_follow() {
    let mut editor = chain(2);
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Output(true)));

    // an id that is taken is not given
    let _ = editor.update(Message::Edit(
        "n0".to_owned(),
        Edit::Rename("n1".to_owned()),
    ));
    assert_eq!(
        editor
            .pipeline
            .nodes[0]
            .id,
        "n0"
    );

    let _ = editor.update(Message::Edit(
        "n1".to_owned(),
        Edit::Rename("last".to_owned()),
    ));
    let _ = editor.update(Message::Edit(
        "n0".to_owned(),
        Edit::Rename("first".to_owned()),
    ));
    assert_eq!(
        editor
            .pipeline
            .nodes[1]
            .inputs,
        ["first"]
    );
    assert_eq!(
        editor
            .pipeline
            .outputs,
        ["last"]
    );
}

#[test]
fn a_deleted_node_leaves_its_readers_unwired() {
    let mut editor = chain(2);
    let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Delete));
    assert_eq!(
        editor
            .pipeline
            .nodes[0]
            .inputs,
        [""]
    );
}

#[test]
fn a_run_keeps_the_saved_frames_and_a_rerun_does_not() {
    let mut editor = chain(2);
    editor
        .pipeline
        .outputs = vec!["n1".to_owned()];
    let frame = ("n1".to_owned(), DataFrame::empty());
    let _ = editor.update(Message::Run);
    let _ = editor.update(Message::Ran(vec!["n1".to_owned()], Ok(vec![frame])));
    assert_eq!(status(&editor), [Status::Ran, Status::Ran]);

    let _ = editor.update(Message::Run);
    assert!(
        editor
            .known
            .frame("n1")
            .is_some()
    );
    assert_eq!(status(&editor), [Status::Ran, Status::Ran]);

    let _ = editor.update(Message::Rerun);
    assert!(
        editor
            .known
            .frame("n1")
            .is_none()
    );
    assert_eq!(
        status(&editor),
        [
            Status::Unknown,
            Status::Unknown
        ]
    );
}

#[test]
fn a_run_has_the_writes_done() {
    use Status::*;

    let mut editor = chain(2);
    editor
        .pipeline
        .nodes
        .push(Node {
            id: "written".to_owned(),
            block: "write_parquet".to_owned(),
            inputs: vec!["n0".to_owned()],
            args: serde_json::json!({ "path": "out.parquet" }),
            ui: Value::Null,
        });
    editor.check_wiring();
    // a Run is to have it write, though it is no output
    let (_, targets, _) = editor
        .known
        .plan(&editor.pipeline, false);
    assert_eq!(targets, ["written"]);

    let _ = editor.update(Message::Run);
    let _ = editor.update(Message::Ran(vec!["written".to_owned()], Ok(vec![])));
    assert_eq!(status(&editor), [Ran, Unknown, Ran]);
}

#[test]
fn a_right_click_on_the_background_adds_a_node_there() {
    use iced::mouse;

    let mut editor = chain(1);
    let right = [
        mouse::Event::ButtonPressed(mouse::Button::Right),
        mouse::Event::ButtonReleased(mouse::Button::Right),
    ]
    .map(iced::Event::Mouse);

    // on a card, there is no menu
    let mut ui = simulator(editor.view());
    ui.point_at(Point::new(60.0, 100.0));
    ui.simulate(right.clone());
    assert_eq!(
        ui.into_messages()
            .count(),
        0
    );

    let mut ui = simulator(editor.view());
    ui.point_at(Point::new(400.0, 300.0));
    ui.simulate(right);
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    let (_, point) = editor.menu.unwrap();

    // the menu is where was pressed: its third group is Reshape, whose seventh block is limit
    let mut ui = simulator(editor.view());
    let position = Point::new(410.0, 300.0 + 4.0 + 2.5 * 26.0);
    ui.point_at(position);
    ui.simulate([iced::Event::Mouse(
        mouse::Event::CursorMoved { position },
    )]);
    ui.point_at(Point::new(580.0, 352.0 + 4.0 + 6.5 * 26.0));
    ui.simulate(iced_test::simulator::click());
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert!(
        editor
            .menu
            .is_none()
    );
    let added = &editor
        .pipeline
        .nodes[1];
    assert_eq!(added.block, "limit");
    assert_eq!(added.ui["x"], point.x.round());
    assert_eq!(added.ui["y"], point.y.round());
}

#[test]
fn a_field_keeps_the_typing_when_an_error_comes() {
    // the views outlive the editors they are of, as those of the window do
    let open = || {
        let mut editor = chain(1);
        let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Toggle));
        Box::leak(Box::new(editor))
    };
    let before = open();
    let after = open();
    let checks = [("n0".to_owned(), Err("no such file".to_owned()))].into();
    let _ = after.update(Message::Checked(Ok(checks)));

    let mut ui = simulator(before.view());
    ui.click("a.parquet")
        .unwrap();
    let mut ui = ui.rebuild(after.view());
    ui.typewrite("x");

    let edits = ui
        .into_messages()
        .filter(|message| matches!(message, Message::Edit(_, Edit::Args(_))))
        .count();
    assert_eq!(edits, 1);
}

#[test]
fn a_scan_is_not_looked_up_on_disk_until_a_check() {
    // a scan of a directory without Parquet files, which building its plan would reject
    let directory = std::env::temp_dir().join(format!("polaroid-empty-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut editor = chain(2);
    let args = serde_json::json!({ "path": directory });
    let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Args(args)));
    std::fs::remove_dir_all(directory).unwrap();

    assert_eq!(
        status(&editor),
        [
            Status::Unknown,
            Status::Unknown
        ]
    );
}

/// What a press on a label of the window asks for.
fn press(
    editor: &Editor,
    label: &str,
) -> Vec<Message> {
    let mut ui = simulator(editor.view());
    ui.click(label)
        .unwrap();
    ui.into_messages()
        .collect()
}

#[test]
fn a_check_covers_the_runs_until_every_node_is_sound() {
    let mut editor = chain(2);
    let checked = |editor: &mut Editor, first: Columns| {
        let checks = [
            ("n0".to_owned(), first),
            ("n1".to_owned(), Ok(vec![])),
        ]
        .into();
        let _ = editor.update(Message::Checked(Ok(checks)));
    };
    let is_covered = |editor: &Editor| matches!(press(editor, "Run")[..], [Message::Check]);

    // whether or not a Run would compute anything
    assert!(is_covered(&editor));
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Output(true)));
    assert!(is_covered(&editor));

    // a Check that found an error is to be done again, once that is seen to
    checked(&mut editor, Err("no such file".to_owned()));
    assert!(is_covered(&editor));
    checked(&mut editor, Ok(vec![]));
    assert!(matches!(press(&editor, "Run")[..], [Message::Run]));
    assert!(matches!(press(&editor, "Rerun")[..], [Message::Rerun]));

    // a node that no Run computes may be wired badly, which no Check would mend
    let _ = editor.update(Message::Add("limit".to_owned(), Point::ORIGIN));
    assert!(!is_covered(&editor));

    let args = Edit::Args(serde_json::json!({ "n": 6 }));
    let _ = editor.update(Message::Edit("n1".to_owned(), args));
    assert!(is_covered(&editor));

    // and so is a Check after a Run that failed
    checked(&mut editor, Ok(vec![]));
    let _ = editor.update(Message::Ran(vec!["n1".to_owned()], Err("no".to_owned())));
    assert!(is_covered(&editor));
    checked(&mut editor, Ok(vec![]));
    assert!(!is_covered(&editor));
}

#[test]
fn a_load_and_a_new_take_the_place_of_the_pipeline() {
    let path = std::env::temp_dir().join(format!("polaroid-load-{}.json", std::process::id()));
    let mut editor = chain(2);
    editor.path = path
        .to_str()
        .unwrap()
        .to_owned();
    let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Toggle));
    let _ = editor.update(Message::Save);
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Delete));
    assert!(editor.is_dirty);

    // no file picked, nothing happens
    let _ = editor.update(Message::Loaded(None));
    assert!(editor.is_dirty);
    let saved = editor.path.clone();
    let _ = editor.update(Message::Loaded(Some(saved)));
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        editor
            .pipeline
            .nodes
            .len(),
        2
    );
    assert!(!editor.is_dirty);

    // a new pipeline is empty under its name, and yet to be saved
    let _ = editor.update(Message::Named(Some("new.json".to_owned())));
    assert!(
        editor
            .pipeline
            .nodes
            .is_empty()
    );
    assert_eq!(editor.path, "new.json");
    assert!(editor.is_dirty);
}

#[test]
fn a_closed_card_is_as_wide_as_its_id_and_wires_by_its_ports_alone() {
    let mut editor = chain(2);
    let long = "a_rather_long_name_for_a_scan_of_orders";
    let _ = editor.update(Message::Edit(
        "n0".to_owned(),
        Edit::Rename(long.to_owned()),
    ));
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Wire(0, String::new())));

    let mut ui = simulator(editor.view());
    let id = ui
        .find(long)
        .unwrap();
    assert!(id.bounds().width > 250.0);
    // which node it reads is picked on the open card, with its args
    assert!(
        ui.find("unwired")
            .is_err()
    );
    drop(ui);

    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    let mut ui = simulator(editor.view());
    assert!(
        ui.find("unwired")
            .is_ok()
    );
}

#[test]
fn an_open_card_is_resized_to_whole_pixels() {
    let mut editor = chain(1);
    let _ = editor.update(Message::Resize("n0".to_owned(), Size::new(300.4, 200.6)));
    assert_eq!(
        layout::size(
            &editor
                .pipeline
                .nodes[0]
        ),
        Size::new(300.0, 201.0)
    );
    assert!(editor.is_dirty);
}

#[test]
fn an_expression_is_renamed_edited_and_deleted_for_the_nodes_that_use_it() {
    let mut editor = chain(2);
    let drawer = |editor: &mut Editor, message: drawer::Message| {
        let _ = editor.update(Message::Drawer(message));
    };
    let checked = |editor: &mut Editor| {
        let checks = ["n0", "n1"]
            .map(|id| (id.to_owned(), Ok(vec![])))
            .into();
        let _ = editor.update(Message::Checked(Ok(checks)));
    };

    drawer(&mut editor, drawer::Message::New);
    drawer(
        &mut editor,
        drawer::Message::Name("expression_1".to_owned(), "few".to_owned()),
    );
    drawer(
        &mut editor,
        drawer::Message::Rename("expression_1".to_owned()),
    );
    assert!(editor.is_dirty);
    let args = Edit::Args(serde_json::json!({ "n": crate::marker("few") }));
    let _ = editor.update(Message::Edit("n1".to_owned(), args));

    // an edit to it forgets the nodes that use it, and no others
    checked(&mut editor);
    let five = form::Message::Put(String::new(), serde_json::json!(5));
    drawer(&mut editor, drawer::Message::Form("few".to_owned(), five));
    assert_eq!(
        editor
            .pipeline
            .expressions["few"],
        5
    );
    assert_eq!(
        status(&editor),
        [
            Status::Checked,
            Status::Unknown
        ]
    );

    // under another name it is the same to them
    checked(&mut editor);
    drawer(
        &mut editor,
        drawer::Message::Name("few".to_owned(), "some".to_owned()),
    );
    drawer(&mut editor, drawer::Message::Rename("few".to_owned()));
    let args = &editor
        .pipeline
        .nodes[1]
        .args;
    assert_eq!(args["n"], crate::marker("some"));
    assert_eq!(
        status(&editor),
        [
            Status::Checked,
            Status::Checked
        ]
    );

    // and gone, they are wired badly
    drawer(&mut editor, drawer::Message::Delete("some".to_owned()));
    assert!(
        editor
            .pipeline
            .expressions
            .is_empty()
    );
    assert_eq!(
        status(&editor),
        [
            Status::Checked,
            Status::Rejected
        ]
    );
}

#[test]
fn a_card_picks_an_expression_by_its_name() {
    use crate::polars::prelude::{col, lit};

    let mut editor = chain(1);
    let large = serde_json::to_value(col("x").gt(lit(1))).unwrap();
    editor
        .pipeline
        .expressions
        .insert("large".to_owned(), large);
    let _ = editor.update(Message::Add("filter".to_owned(), Point::new(300.0, 20.0)));
    let args = Edit::Args(serde_json::json!({ "predicates": [crate::marker("large")] }));
    let _ = editor.update(Message::Edit("filter_1".to_owned(), args));
    let _ = editor.update(Message::Edit("filter_1".to_owned(), Edit::Toggle));

    // with the drawer hidden, what names and writes the expression is the card
    let _ = editor.update(Message::ToggleDrawer);
    let mut ui = simulator(editor.view());
    assert!(
        ui.find("large")
            .is_ok()
    );
    assert!(
        ui.find(r#"pl.col("x").gt(1)"#)
            .is_ok()
    );
    assert!(
        ui.find("Expressions")
            .is_ok()
    );
}

/// What dragging across the label of an open or closed card of a scan selects.
fn selected(is_open: bool) -> String {
    use iced::advanced::widget::operation::{self, Operation, Outcome};
    use iced::mouse;

    let mut editor = chain(1);
    let checks = [("n0".to_owned(), Err("no such file".to_owned()))].into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    if is_open {
        let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Toggle));
    }

    let mut ui = simulator(editor.view());
    let bounds = ui
        .find("no such file")
        .unwrap()
        .bounds();
    let y = bounds.center_y();
    let position = Point::new(bounds.x + 1.0, y);
    ui.point_at(position);
    ui.simulate([
        iced::Event::Mouse(mouse::Event::CursorMoved { position }),
        iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
    ]);
    let position = Point::new(bounds.x + bounds.width - 1.0, y);
    ui.point_at(position);
    ui.simulate([iced::Event::Mouse(
        mouse::Event::CursorMoved { position },
    )]);

    let mut copy = operation::text::copy();
    ui.operate(&mut operation::black_box(&mut copy));
    match copy.finish() {
        Outcome::Some(text) => text,
        _ => String::new(),
    }
}

#[test]
fn the_text_of_a_card_is_selected_by_a_drag_across_it() {
    assert_eq!(selected(true), "no such file");
    assert_eq!(selected(false), "no such file");
}
