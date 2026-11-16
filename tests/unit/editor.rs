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
    graph::layout(&mut pipeline);
    Editor::new(registry, "p.json".to_owned(), pipeline)
}

fn status(editor: &Editor) -> Vec<graph::Status> {
    editor
        .pipeline
        .nodes
        .iter()
        .map(|node| editor.status(&node.id))
        .collect()
}

#[test]
fn an_edit_forgets_the_node_and_its_readers() {
    use graph::Status::*;

    let mut editor = chain(3);
    assert_eq!(status(&editor), [Unknown, Unknown, Unknown]);

    let checks = ["n0", "n1", "n2"]
        .map(|id| (id.to_owned(), Ok(vec![])))
        .into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    assert_eq!(status(&editor), [Checked, Checked, Checked]);

    let _ = editor.update(Message::Ran(vec!["n1".to_owned()], Ok(vec![])));
    assert_eq!(status(&editor), [Ran, Ran, Checked]);

    editor.select(Some(1));
    let _ = editor.update(Message::Edit(Edit::Wire(0, "n0".to_owned())));
    assert_eq!(status(&editor), [Ran, Unknown, Unknown]);

    let _ = editor.update(Message::Ran(vec!["n2".to_owned()], Err("no".to_owned())));
    assert_eq!(status(&editor), [Ran, Unknown, Failed]);
}

#[test]
fn a_node_wired_badly_is_rejected() {
    use graph::Status::*;

    let mut editor = chain(2);
    editor.select(Some(1));
    let _ = editor.update(Message::Edit(Edit::Wire(0, "gone".to_owned())));
    assert_eq!(status(&editor), [Unknown, Rejected]);
}

#[test]
fn a_double_click_on_its_id_lays_a_card_open() {
    let mut editor = chain(2);
    // the first card is at 20, 20 and its "save output" box below its id
    let id = Point::new(60.0, 35.0);
    let save = Point::new(40.0, 77.0);

    let presses = |editor: &Editor, at: Point, times: usize| {
        let mut ui = simulator(
            canvas(editor.graph())
                .width(Fill)
                .height(Fill),
        );
        ui.point_at(at);
        for _ in 0..times {
            ui.simulate(iced_test::simulator::click());
        }
        ui.into_messages()
            .collect::<Vec<_>>()
    };
    let toggles = |events: &[graph::Event]| {
        events
            .iter()
            .filter(|event| matches!(event, graph::Event::Toggle(0)))
            .count()
    };

    assert_eq!(toggles(&presses(&editor, id, 1)), 0);
    assert_eq!(toggles(&presses(&editor, id, 2)), 1);
    assert_eq!(toggles(&presses(&editor, id, 3)), 1);
    assert_eq!(toggles(&presses(&editor, save, 2)), 0);

    assert!(!graph::is_open(
        &editor
            .pipeline
            .nodes[0]
    ));
    let _ = editor.update(Message::Graph(graph::Event::Toggle(0)));
    assert!(graph::is_open(
        &editor
            .pipeline
            .nodes[0]
    ));
    assert!(editor.is_dirty);
    let _ = editor.update(Message::Graph(graph::Event::Toggle(0)));
    assert!(!graph::is_open(
        &editor
            .pipeline
            .nodes[0]
    ));

    let saves = presses(&editor, save, 1);
    assert!(matches!(saves.as_slice(), [graph::Event::Save(0)]));
    let _ = editor.update(Message::Graph(graph::Event::Save(0)));
    assert_eq!(
        editor
            .pipeline
            .outputs,
        ["n0"]
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
    assert_eq!(
        status(&editor),
        [
            graph::Status::Ran,
            graph::Status::Ran
        ]
    );

    let _ = editor.update(Message::Run);
    assert!(
        editor
            .frame("n1")
            .is_some()
    );
    assert_eq!(
        status(&editor),
        [
            graph::Status::Ran,
            graph::Status::Ran
        ]
    );

    let _ = editor.update(Message::Rerun);
    assert!(
        editor
            .frame("n1")
            .is_none()
    );
    assert_eq!(
        status(&editor),
        [
            graph::Status::Unknown,
            graph::Status::Unknown
        ]
    );
}

#[test]
fn a_run_has_the_writes_done() {
    use graph::Status::*;

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
