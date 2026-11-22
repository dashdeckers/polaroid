use iced_test::simulator;

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
        polars: polars::VERSION.to_owned(),
        nodes,
        outputs: vec![],
    };
    layout::layout(&mut pipeline);
    Editor::new(registry, "p.json".to_owned(), pipeline)
}

fn status(editor: &Editor) -> Vec<Status> {
    editor
        .pipeline
        .nodes
        .iter()
        .map(|node| editor.status(&node.id))
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
            .frame("n1")
            .is_some()
    );
    assert_eq!(status(&editor), [Status::Ran, Status::Ran]);

    let _ = editor.update(Message::Rerun);
    assert!(
        editor
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
    assert_eq!(editor.writes, ["written"]);

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

    let mut ui = simulator(editor.view());
    ui.click("limit")
        .unwrap();
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
