use super::known::Status;
use super::look::{is_refused, is_unsaved};
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

/// The field that holds `value`, which the text of a tile that says the same is not.
fn field(value: &str) -> impl iced_test::Selector<Output = iced_test::selector::Target> + '_ {
    use iced_test::selector::{Candidate, Target};

    move |candidate: Candidate<'_>| {
        let is_it =
            matches!(&candidate, Candidate::TextInput { state, .. } if state.text() == value);
        is_it.then(|| Target::from(candidate))
    }
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
fn a_double_click_on_its_tile_lays_a_card_open_and_its_toggle_closes_it() {
    let mut editor = chain(1);
    let node = |editor: &Editor| {
        editor
            .pipeline
            .nodes[0]
            .clone()
    };
    assert!(!layout::is_open(&node(&editor)));

    // a closed card has no toggle, and one press on its tile does not open it
    let mut ui = simulator(editor.view());
    assert!(
        ui.find(theme::sign::CLOSE)
            .is_err()
    );
    ui.click("n0")
        .unwrap();
    assert_eq!(ui.drain().count(), 0);
    ui.simulate(iced_test::simulator::click());
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert!(layout::is_open(&node(&editor)));
    assert!(editor.is_dirty);

    let mut ui = simulator(editor.view());
    ui.click(theme::sign::CLOSE)
        .unwrap();
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert!(!layout::is_open(&node(&editor)));
}

#[test]
fn the_small_table_saves_its_node_from_its_tile_and_from_its_open_card() {
    let mut editor = chain(1);
    for is_open in [false, true] {
        let mut ui = simulator(editor.view());
        // it ends the line of the block on a tile, and stands before the id on an open card
        let middle = theme::control::S / 2.0;
        let table = match is_open {
            false => {
                let block = ui
                    .find("scan_parquet")
                    .unwrap()
                    .bounds();
                Point::new(block.x + block.width + middle, block.center_y())
            }
            true => {
                let id = ui
                    .find(field("n0"))
                    .unwrap()
                    .bounds();
                Point::new(id.x - theme::spacing::S - middle, id.center_y())
            }
        };
        ui.point_at(table);
        ui.simulate(iced_test::simulator::click());
        for message in ui
            .into_messages()
            .collect::<Vec<_>>()
        {
            let _ = editor.update(message);
        }
        // saved from the closed card, and no longer from the open one
        assert_eq!(
            editor
                .pipeline
                .outputs
                .len(),
            usize::from(!is_open)
        );
        let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Toggle));
    }
}

#[test]
fn a_tile_says_what_is_wrong_with_its_node_when_hovered() {
    let mut editor = chain(1);
    let checks = [("n0".to_owned(), Err("no such file".to_owned()))].into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    let mut ui = simulator(editor.view());
    assert!(
        ui.find("no such file")
            .is_err()
    );
    let position = ui
        .find("n0")
        .unwrap()
        .bounds()
        .center();
    ui.point_at(position);
    ui.simulate([iced::Event::Mouse(
        iced::mouse::Event::CursorMoved { position },
    )]);
    assert!(
        ui.find("no such file")
            .is_ok()
    );
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
fn a_run_keeps_the_saved_frames_and_a_reload_does_not() {
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

    let _ = editor.update(Message::Reload);
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
fn clear_says_how_much_the_saved_frames_take_and_lets_them_go() {
    use crate::polars::prelude::df;

    let mut editor = chain(2);
    editor
        .pipeline
        .outputs = vec!["n1".to_owned()];
    // two thousand numbers of eight bytes each
    let frame = df!("x" => (0..2000i64).collect::<Vec<_>>()).unwrap();
    let ran = vec![("n1".to_owned(), frame)];
    let _ = editor.update(Message::Ran(vec!["n1".to_owned()], Ok(ran)));
    assert!(
        simulator(editor.view())
            .find("16 KB")
            .is_ok()
    );

    for message in press(&editor, "Clear") {
        let _ = editor.update(message);
    }
    assert!(
        editor
            .known
            .frame("n1")
            .is_none()
    );
    assert!(
        simulator(editor.view())
            .find("0 B")
            .is_ok()
    );
    // with nothing left to let go of, it does nothing
    assert!(press(&editor, "Clear").is_empty());
}

#[test]
fn a_size_is_to_a_tenth_below_ten_of_its_unit_and_else_to_a_whole_one() {
    let sizes = [
        0,
        999,
        1_000,
        9_949,
        9_960,
        340_400,
        999_499,
        999_500,
        12_345_678,
        1_234_567_890,
    ];
    assert_eq!(
        sizes.map(bytes),
        [
            "0 B", "999 B", "1.0 KB", "9.9 KB", "10 KB", "340 KB", "999 KB", "1.0 MB", "12 MB",
            "1.2 GB",
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
        .plan(&editor.pipeline);
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

    // the menu is where was pressed: its third group is Filter, whose fifth block is limit
    let mut ui = simulator(editor.view());
    let position = Point::new(410.0, 300.0 + 4.0 + 2.5 * 26.0);
    ui.point_at(position);
    ui.simulate([iced::Event::Mouse(
        mouse::Event::CursorMoved { position },
    )]);
    ui.point_at(Point::new(580.0, 352.0 + 4.0 + 4.5 * 26.0));
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
fn the_files_are_pressed_as_buttons_and_save_once_there_is_a_change() {
    let mut editor = chain(1);
    assert!(matches!(press(&editor, "New")[..], [Message::New]));
    assert!(matches!(press(&editor, "Load")[..], [Message::Load]));
    assert!(press(&editor, "Save").is_empty());
    editor.is_dirty = true;
    assert!(matches!(press(&editor, "Save")[..], [Message::Save]));
}

#[test]
fn the_end_of_the_switch_is_a_button_once_manual_is_chosen() {
    let mut editor = chain(1);
    assert!(matches!(
        press(&editor, "Live")[..],
        [Message::Mode(Mode::Live)]
    ));
    assert!(matches!(press(&editor, "Check")[..], [Message::Check]));

    // elsewhere, it is the stop of Manual, which a press chooses with a Check at once
    editor.mode = Mode::Delay;
    assert!(
        simulator(editor.view())
            .find("Check")
            .is_err()
    );
    assert!(press(&editor, "Delay 0.5 s").is_empty());
    let pressed = press(&editor, "Manual");
    assert!(matches!(pressed[..], [Message::Check]));
    for message in pressed {
        let _ = editor.update(message);
    }
    assert_eq!(editor.mode, Mode::Manual);
    assert!(editor.job.is_some());
}

#[test]
fn the_button_is_a_check_until_every_node_is_sound_and_a_run_from_then_on() {
    let mut editor = chain(2);
    let checked = |editor: &mut Editor, first: Columns| {
        let checks = [
            ("n0".to_owned(), first),
            ("n1".to_owned(), Ok(vec![])),
        ]
        .into();
        let _ = editor.update(Message::Checked(Ok(checks)));
    };
    let is_run = |editor: &Editor| matches!(press(editor, "Run")[..], [Message::Run]);
    let is_covered = |editor: &Editor| matches!(press(editor, "Check")[..], [Message::Check]);

    // whether or not a Run would compute anything
    assert!(is_covered(&editor));
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Output(true)));
    assert!(is_covered(&editor));

    // a Check that found an error is to be done again, once that is seen to
    checked(&mut editor, Err("no such file".to_owned()));
    assert!(is_covered(&editor));
    checked(&mut editor, Ok(vec![]));
    assert!(is_run(&editor));
    assert!(matches!(press(&editor, "Reload")[..], [Message::Reload]));

    // a node that no Run computes may be wired badly, which no Check would mend
    let _ = editor.update(Message::Add("limit".to_owned(), Point::ORIGIN));
    assert!(is_run(&editor));

    let args = Edit::Args(serde_json::json!({ "n": 6 }));
    let _ = editor.update(Message::Edit("n1".to_owned(), args));
    assert!(is_covered(&editor));

    // and so is a Check after a Run that failed
    checked(&mut editor, Ok(vec![]));
    let _ = editor.update(Message::Ran(vec!["n1".to_owned()], Err("no".to_owned())));
    assert!(is_covered(&editor));
    checked(&mut editor, Ok(vec![]));
    assert!(is_run(&editor));
}

#[test]
fn a_change_is_followed_by_a_check_and_a_run_unless_their_button_is_pressed() {
    let mut editor = chain(2);
    let edit = |editor: &mut Editor, n: u32| {
        let args = Edit::Args(serde_json::json!({ "n": n }));
        let _ = editor.update(Message::Edit("n1".to_owned(), args));
    };
    let checks = || {
        [
            ("n0".to_owned(), Ok(vec![])),
            ("n1".to_owned(), Ok(vec![])),
        ]
        .into()
    };

    // nothing is under way after a change, until a mode has one follow it
    edit(&mut editor, 6);
    assert!(editor.job.is_none());
    for mode in [Mode::Live, Mode::Delay] {
        let _ = editor.update(Message::Mode(mode));
        assert!(editor.job.is_some());
        edit(&mut editor, 7);
        assert!(editor.job.is_some());
        // a Check that finds all sound is followed by a Run
        let _ = editor.update(Message::Checked(Ok(checks())));
        assert!(editor.job.is_some());
        // and a card laid open is no change
        let _ = editor.update(Message::Ran(vec![], Ok(vec![])));
        let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
        assert!(editor.job.is_none());
    }

    // nor does any follow a change that leaves a node unsound, as a number half typed does
    let _ = editor.update(Message::Mode(Mode::Live));
    let _ = editor.update(Message::Ran(vec![], Ok(vec![])));
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Output(true)));
    let _ = editor.update(Message::Ran(vec![], Ok(vec![])));
    let args = Edit::Args(serde_json::json!({ "n": null }));
    let _ = editor.update(Message::Edit("n1".to_owned(), args));
    assert!(editor.job.is_none());

    // their button is pressed for a Check alone
    let _ = editor.update(Message::Check);
    assert_eq!(editor.mode, Mode::Manual);
    let _ = editor.update(Message::Checked(Ok(checks())));
    assert!(editor.job.is_none());
    edit(&mut editor, 8);
    assert!(editor.job.is_none());
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
fn a_tile_is_a_square_and_an_open_card_is_as_wide_as_its_id() {
    let mut editor = chain(2);
    let long = "a_rather_long_name_for_a_scan_of_orders";
    let _ = editor.update(Message::Edit(
        "n0".to_owned(),
        Edit::Rename(long.to_owned()),
    ));
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Wire(0, String::new())));

    // a tile cuts a long id short, and says nothing of what its node reads
    let mut ui = simulator(editor.view());
    let id = ui
        .find(long)
        .unwrap();
    assert!(id.bounds().width < theme::surface::TILE);
    assert!(
        ui.find("unwired")
            .is_err()
    );
    drop(ui);

    // which node it reads is picked on the open card, with its args
    for id in [long, "n1"] {
        let _ = editor.update(Message::Edit(id.to_owned(), Edit::Toggle));
    }
    let mut ui = simulator(editor.view());
    assert!(
        ui.find("unwired")
            .is_ok()
    );
    drop(ui);
    // the id is whole in its field, though the card was made no wider than its tile
    let made = Size::new(theme::surface::TILE, 0.0);
    let _ = editor.update(Message::Resize(long.to_owned(), made));
    let mut ui = simulator(editor.view());
    let id = ui
        .find(field(long))
        .unwrap();
    assert!(
        id.bounds().width > polaroid_iced::width_of(long, theme::size::TITLE, theme::font::SANS)
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

    drawer(&mut editor, drawer::Message::New(crate::Kind::Expression));
    let rename = |old: &str, new: &str| drawer::Message::Rename(old.to_owned(), new.to_owned());
    drawer(&mut editor, rename("expression_1", "few"));
    assert!(editor.is_dirty);
    let args = Edit::Args(serde_json::json!({ "n": crate::marker("few") }));
    let _ = editor.update(Message::Edit("n1".to_owned(), args));

    // an edit to it forgets the nodes that use it, and no others
    checked(&mut editor);
    let five = serde_json::json!({ "Literal": { "Dyn": { "Int": 5 } } });
    let put = form::Message::Put(String::new(), five.clone());
    drawer(&mut editor, drawer::Message::Form("few".to_owned(), put));
    assert_eq!(
        editor
            .pipeline
            .expressions["few"],
        five
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
    drawer(&mut editor, rename("few", "some"));
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
        ui.find("@large")
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

/// What dragging across the error on the open card of a scan selects.
fn selected() -> String {
    use iced::advanced::widget::operation::{self, Operation, Outcome};
    use iced::mouse;

    let mut editor = chain(1);
    let checks = [("n0".to_owned(), Err("no such file".to_owned()))].into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    let _ = editor.update(Message::Edit("n0".to_owned(), Edit::Toggle));

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
    assert_eq!(selected(), "no such file");
}

#[test]
fn a_cell_of_bytes_shows_their_number() {
    use crate::polars::prelude::AnyValue;

    assert_eq!(card::shown(AnyValue::Binary(b"abcd")), "4 bytes");
    assert_eq!(card::shown(AnyValue::Int32(4)), "4");
    assert_eq!(card::shown(AnyValue::String("4 bytes")), "4 bytes");
}

#[test]
fn a_card_shows_the_images_that_its_block_has_shown_and_browses_them_when_open() {
    use crate::polars::prelude::df;

    let image = |width: u32| {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="3"/>"#)
    };
    let images = [
        Some(image(4).into_bytes()),
        None,
        Some(image(5).into_bytes()),
    ];
    let frame = df!("name" => ["a", "b", "c"], "plot" => images).unwrap();
    let mut editor = chain(2);
    editor
        .pipeline
        .outputs = vec!["n1".to_owned()];
    let ran = |editor: &mut Editor| {
        let ran = vec![("n1".to_owned(), frame.clone())];
        let _ = editor.update(Message::Ran(vec!["n1".to_owned()], Ok(ran)));
    };
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));

    // a limit passes the images on, and shows none
    ran(&mut editor);
    assert!(
        simulator(editor.view())
            .find("1 of 3")
            .is_err()
    );
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    let node = &mut editor
        .pipeline
        .nodes[1];
    node.block = "view_images".to_owned();
    node.args = serde_json::json!({ "column": "plot" });
    ran(&mut editor);

    // a closed card shows one, and an open one says which of how many
    assert!(
        simulator(editor.view())
            .find("1 of 3")
            .is_err()
    );
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    assert!(
        simulator(editor.view())
            .find("1 of 3")
            .is_ok()
    );

    // a column that a value names is that column
    editor
        .pipeline
        .values
        .insert("shown".to_owned(), "plot".into());
    editor
        .pipeline
        .nodes[1]
        .args = serde_json::json!({ "column": { "$value": "shown" } });
    ran(&mut editor);
    assert!(
        simulator(editor.view())
            .find("1 of 3")
            .is_ok()
    );
}

#[test]
fn delay_waits_longer_as_it_is_dragged_up_and_half_a_second_at_the_least() {
    let mut editor = chain(1);
    assert_eq!(editor.still, Duration::from_millis(500));
    let _ = editor.update(Message::Wait(3));
    assert_eq!(editor.still, Duration::from_millis(800));
    let _ = editor.update(Message::Wait(-1));
    assert_eq!(editor.still, Duration::from_millis(700));
    let _ = editor.update(Message::Wait(-9));
    assert_eq!(editor.still, Duration::from_millis(500));
    assert!(
        simulator(editor.view())
            .find("Delay 0.5 s")
            .is_ok()
    );
}

#[test]
fn a_name_is_marked_while_what_is_typed_for_it_is_another() {
    use iced::keyboard::key::Named;

    // nothing typed, or the name again
    assert!(!is_unsaved(None, "n1"));
    assert!(!is_unsaved(Some("n1"), "n1"));
    assert!(is_unsaved(Some("n"), "n1"));
    assert!(is_unsaved(Some(""), "n1"));
    // and red while it is empty or taken
    let is_taken = |name: &str| name == "n0";
    assert!(!is_refused(Some("n1"), "n1", is_taken));
    assert!(!is_refused(Some("n2"), "n1", is_taken));
    assert!(is_refused(Some("n0"), "n1", is_taken));
    assert!(is_refused(Some(""), "n1", is_taken));

    // typed in the drawer, it is marked until Enter gives it
    let mut editor = chain(1);
    let add = |editor: &mut Editor| {
        let new = drawer::Message::New(crate::Kind::Expression);
        let _ = editor.update(Message::Drawer(new));
    };
    let typed = |ui: &mut iced_test::Simulator<'_, Message>| {
        ui.click("expression_1")
            .unwrap();
        ui.tap_key(Named::End);
        ui.typewrite("_few");
        ui.find("expression_1_few")
            .expect("what is typed is shown");
        ui.tap_key(Named::Enter);
    };
    add(&mut editor);
    let mut ui = simulator(editor.view());
    typed(&mut ui);
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert!(
        editor
            .pipeline
            .expressions
            .contains_key("expression_1_few")
    );

    // one that is taken stays typed on Enter, until its field is left
    add(&mut editor);
    let mut ui = simulator(editor.view());
    typed(&mut ui);
    assert!(
        ui.find("expression_1")
            .is_err()
    );
    ui.point_at(Point::new(700.0, 600.0));
    ui.simulate(iced_test::simulator::click());
    assert!(
        ui.find("expression_1")
            .is_ok()
    );
    assert_eq!(
        ui.into_messages()
            .count(),
        0
    );
    assert_eq!(
        editor
            .pipeline
            .expressions
            .len(),
        2
    );
}

#[test]
fn an_id_is_given_on_enter_and_shown_again_when_its_field_is_left() {
    use iced::keyboard::key::Named;

    let mut editor = chain(2);
    let id = |editor: &Editor| {
        editor
            .pipeline
            .nodes[1]
            .id
            .clone()
    };
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    let mut ui = simulator(editor.view());
    ui.click(field("n1"))
        .unwrap();
    ui.tap_key(Named::End);
    ui.typewrite("x");
    ui.tap_key(Named::Enter);
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert_eq!(id(&editor), "n1x");

    // another node's id stays typed, and Enter does not give it
    let mut ui = simulator(editor.view());
    ui.click(field("n1x"))
        .unwrap();
    ui.tap_key(Named::End);
    for _ in 0..2 {
        ui.tap_key(Named::Backspace);
    }
    ui.typewrite("0");
    ui.tap_key(Named::Enter);
    assert!(
        ui.find(field("n1x"))
            .is_err()
    );
    // until the field is left, which shows the id again
    ui.point_at(Point::new(700.0, 600.0));
    ui.simulate(iced_test::simulator::click());
    assert!(
        ui.find(field("n1x"))
            .is_ok()
    );
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert_eq!(id(&editor), "n1x");

    // and so is what was typed and never given
    let mut ui = simulator(editor.view());
    ui.click(field("n1x"))
        .unwrap();
    ui.typewrite("yz");
    assert!(
        ui.find(field("n1x"))
            .is_err()
    );
    ui.point_at(Point::new(700.0, 600.0));
    ui.simulate(iced_test::simulator::click());
    assert!(
        ui.find(field("n1x"))
            .is_ok()
    );
}

#[test]
fn a_node_dropped_on_another_swaps_wiring_and_place_with_it() {
    // a scan, then two limits
    let mut editor = chain(3);
    let place = |editor: &Editor, id: &str| {
        let node = editor
            .pipeline
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap();
        (layout::position(node), node.inputs.clone())
    };
    let (first, _) = place(&editor, "n1");
    let (second, _) = place(&editor, "n2");

    // the card was dragged about before it was dropped
    let _ = editor.update(Message::Move("n1".to_owned(), Point::new(5.0, 5.0)));
    let swap = Message::Swap("n1".to_owned(), "n2".to_owned(), first);
    let _ = editor.update(swap);
    assert_eq!(place(&editor, "n1"), (second, vec!["n2".to_owned()]));
    assert_eq!(place(&editor, "n2"), (first, vec!["n0".to_owned()]));
    assert!(editor.is_dirty);

    // a node that reads a frame does not swap with one that reads none
    assert!(editor.swaps("n1", "n2"));
    assert!(!editor.swaps("n0", "n1"));
    assert!(!editor.swaps("n1", "n1"));
    let _ = editor.update(Message::Swap("n0".to_owned(), "n1".to_owned(), first));
    assert_eq!(place(&editor, "n2"), (first, vec!["n0".to_owned()]));
}

#[test]
fn help_has_everything_say_what_it_is_for_until_it_is_pressed_again() {
    let mut editor = chain(2);
    let hint = "Runs everything again from the files on disk, whatever was kept before.";
    // whether the editor says this with the cursor over the button of a label
    let says = |editor: &Editor, label: &str, said: &str| {
        let mut ui = simulator(editor.view());
        let position = ui
            .find(label)
            .expect("the button is there")
            .bounds()
            .center();
        ui.point_at(position);
        ui.simulate([iced::Event::Mouse(
            iced::mouse::Event::CursorMoved { position },
        )]);
        ui.find(said)
            .is_ok()
    };
    // the introduction is what the Help button says, and nowhere until it is hovered
    let said = |editor: &Editor| {
        [
            says(editor, "Reload", hint),
            says(editor, "Help", INTRO),
            says(editor, "Reload", INTRO),
        ]
    };

    assert_eq!(said(&editor), [false, false, false]);
    let _ = editor.update(Message::Help);
    assert_eq!(said(&editor), [true, true, false]);
    let _ = editor.update(Message::Help);
    assert_eq!(said(&editor), [false, false, false]);
}

#[test]
fn a_check_that_follows_a_change_looks_at_the_nodes_alone_whose_columns_are_not_found() {
    let mut editor = chain(3);
    let columns = vec![("x".to_owned(), DataType::Int64)];
    let checks = [
        ("n0".to_owned(), Ok(columns.clone())),
        ("n1".to_owned(), Ok(columns)),
        ("n2".to_owned(), Err("no such column".to_owned())),
    ]
    .into();
    let _ = editor.update(Message::Checked(Ok(checks)));

    // an empty frame of its columns stands in for each node that has some
    let found = editor.known.found();
    let mut ids: Vec<&String> = found
        .keys()
        .collect();
    ids.sort();
    assert_eq!(ids, ["n0", "n1"]);
    assert_eq!(found["n1"].get_column_names(), ["x"]);
    assert_eq!(found["n1"].height(), 0);

    // an edit lets go of what was found of the node and of those that read it
    let args = serde_json::json!({ "n": 6 });
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Args(args)));
    let found = editor.known.found();
    assert_eq!(
        found
            .keys()
            .collect::<Vec<_>>(),
        ["n0"]
    );
}

/// All that was asked of a simulator.
fn asked(ui: iced_test::Simulator<'_, Message>) -> Vec<Message> {
    ui.into_messages()
        .collect()
}

#[test]
fn args_that_their_block_does_not_take_wait_as_a_draft_on_the_card() {
    use iced::keyboard::key::Named;

    let mut editor = chain(2);
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    let checks = ["n0", "n1"]
        .map(|id| (id.to_owned(), Ok(vec![])))
        .into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    let n = |editor: &Editor| {
        editor
            .pipeline
            .nodes[1]
            .args["n"]
            .clone()
    };

    // its number erased is none, which a limit does not take: the node keeps what it has
    let mut ui = simulator(editor.view());
    ui.click("5")
        .unwrap();
    ui.tap_key(Named::End);
    ui.tap_key(Named::Backspace);
    for message in asked(ui) {
        let _ = editor.update(message);
    }
    assert_eq!(n(&editor), 5);
    assert!(
        editor
            .drafts
            .contains_key("n1")
    );
    // and nothing is forgotten of what was found
    assert_eq!(
        status(&editor),
        [
            Status::Checked,
            Status::Checked
        ]
    );
    assert!(
        simulator(editor.view())
            .find("5")
            .is_err()
    );

    // another typed in its place is taken, and no draft is left
    let _ = editor.update(Message::Edit(
        "n1".to_owned(),
        Edit::Args(serde_json::json!({ "n": 7 })),
    ));
    assert_eq!(n(&editor), 7);
    assert!(
        editor
            .drafts
            .is_empty()
    );
    assert_eq!(
        status(&editor),
        [
            Status::Checked,
            Status::Unknown
        ]
    );
}

#[test]
fn a_card_offers_no_name_that_a_check_of_its_node_would_refuse() {
    use crate::polars::prelude::{col, lit};

    let mut editor = chain(1);
    let named = |name: &str| serde_json::to_value(col(name).gt(lit(1))).unwrap();
    editor
        .pipeline
        .expressions
        .extend([
            ("good".to_owned(), named("a")),
            ("bad".to_owned(), named("nope")),
        ]);
    let _ = editor.update(Message::Add("filter".to_owned(), Point::new(300.0, 20.0)));
    let filter = || "filter_1".to_owned();
    let _ = editor.update(Message::Edit(filter(), Edit::Wire(0, "n0".to_owned())));
    let good = serde_json::json!({ "predicates": [crate::marker("good")] });
    let bad = serde_json::json!({ "predicates": [crate::marker("bad")] });
    let _ = editor.update(Message::Edit(filter(), Edit::Args(good.clone())));
    let _ = editor.update(Message::Edit(filter(), Edit::Toggle));
    let _ = editor.update(Message::ToggleDrawer);

    // until the columns it reads are found, its block alone says what it takes
    let check = |editor: &Editor, args: &Value| {
        let card = editor.card(
            &editor
                .pipeline
                .nodes[1],
        );
        (card.check())(args)
    };
    assert_eq!(check(&editor, &bad), Ok(()));

    let columns = vec![("a".to_owned(), DataType::Int64)];
    let checks = [
        ("n0".to_owned(), Ok(columns.clone())),
        (filter(), Ok(columns)),
    ]
    .into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    assert_eq!(check(&editor, &good), Ok(()));
    let why = check(&editor, &bad)
        .expect_err("a column that is not there is refused")
        .why;
    assert!(why.contains("nope"), "{why}");

    // so listed it is greyed, and a press on it picks nothing
    let mut ui = simulator(editor.view());
    ui.click("@good")
        .unwrap();
    ui.click("@bad")
        .unwrap();
    assert_eq!(
        ui.into_messages()
            .count(),
        0
    );
}

#[test]
fn a_value_is_named_in_the_drawer_and_moved_from_the_card_that_names_it() {
    let mut editor = chain(2);
    let drawer = |editor: &mut Editor, message: drawer::Message| {
        let _ = editor.update(Message::Drawer(message));
    };
    let value = |editor: &Editor| {
        editor
            .pipeline
            .values["value_1"]
            .clone()
    };
    drawer(&mut editor, drawer::Message::New(crate::Kind::Value));
    assert_eq!(value(&editor), Value::Null);

    // none yet, its kind is picked in the drawer, where it is laid open
    let mut ui = simulator(editor.view());
    ui.click("value…")
        .unwrap();
    ui.click("number")
        .unwrap();
    for message in asked(ui) {
        let _ = editor.update(message);
    }
    assert_eq!(value(&editor), 0);

    // a card that names it moves it where it is named, and what was found of the node is let go
    let named = serde_json::json!({ "n": { "$value": "value_1" } });
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Args(named)));
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    let checks = ["n0", "n1"]
        .map(|id| (id.to_owned(), Ok(vec![])))
        .into();
    let _ = editor.update(Message::Checked(Ok(checks)));
    let put = drawer::Message::Put("value_1".to_owned(), String::new(), 3.into());
    drawer(&mut editor, put);
    assert_eq!(value(&editor), 3);
    assert_eq!(
        status(&editor),
        [
            Status::Checked,
            Status::Unknown
        ]
    );
    let _ = editor.update(Message::ToggleDrawer);
    assert!(
        simulator(editor.view())
            .find("@value_1")
            .is_ok()
    );

    // edited to what the node that uses it does not take, it waits and the node has it as it was
    let text = form::Message::Put(String::new(), "three".into());
    drawer(
        &mut editor,
        drawer::Message::Form("value_1".to_owned(), text),
    );
    assert_eq!(value(&editor), 3);
    let whole = form::Message::Put(String::new(), 4.into());
    drawer(
        &mut editor,
        drawer::Message::Form("value_1".to_owned(), whole),
    );
    assert_eq!(value(&editor), 4);

    // and it shares its names with the expressions
    drawer(&mut editor, drawer::Message::New(crate::Kind::Expression));
    let rename = drawer::Message::Rename("expression_1".to_owned(), "value_1".to_owned());
    drawer(&mut editor, rename);
    assert!(
        editor
            .pipeline
            .expressions
            .contains_key("expression_1")
    );
}

#[test]
fn an_expression_is_given_a_hole_in_the_drawer_which_says_what_it_takes() {
    let mut editor = chain(1);
    let _ = editor.update(Message::Drawer(drawer::Message::New(
        crate::Kind::Expression,
    )));
    let mut ui = simulator(editor.view());
    ui.click("@…")
        .unwrap();
    ui.click("$hole_1")
        .unwrap();
    for message in asked(ui) {
        let _ = editor.update(message);
    }
    assert_eq!(
        editor
            .pipeline
            .expressions["expression_1"],
        serde_json::json!({ "$arg": "hole_1" })
    );
    assert!(
        simulator(editor.view())
            .find("takes hole_1: an expression")
            .is_ok()
    );
}

#[test]
fn a_draft_is_taken_once_a_check_finds_what_it_waited_for() {
    use crate::polars::prelude::{col, lit};

    let mut editor = chain(1);
    let named = |name: &str| serde_json::to_value(col(name).gt(lit(1))).unwrap();
    editor
        .pipeline
        .expressions
        .extend([
            ("good".to_owned(), named("a")),
            ("late".to_owned(), named("b")),
        ]);
    let _ = editor.update(Message::Add("filter".to_owned(), Point::new(300.0, 20.0)));
    let filter = || "filter_1".to_owned();
    let _ = editor.update(Message::Edit(filter(), Edit::Wire(0, "n0".to_owned())));
    let good = serde_json::json!({ "predicates": [crate::marker("good")] });
    let late = serde_json::json!({ "predicates": [crate::marker("late")] });
    let _ = editor.update(Message::Edit(filter(), Edit::Args(good.clone())));
    let found = |columns: &[&str]| {
        let columns: Vec<_> = columns
            .iter()
            .map(|column| ((*column).to_owned(), DataType::Int64))
            .collect();
        let checks = [
            ("n0".to_owned(), Ok(columns.clone())),
            (filter(), Ok(columns)),
        ];
        Message::Checked(Ok(checks.into()))
    };
    let args = |editor: &Editor| {
        editor
            .pipeline
            .nodes[1]
            .args
            .clone()
    };
    let _ = editor.update(found(&["a"]));

    // a column that is not there is refused, and the edit waits
    let check = editor
        .card(
            &editor
                .pipeline
                .nodes[1],
        )
        .check();
    let put = form::Message::Put(String::new(), late.clone());
    let change = form::State::default().update(put, &good, None, &*check);
    drop(check);
    let form::Change::Draft(draft) = change else {
        panic!("it waits: {change:?}");
    };
    let _ = editor.update(Message::Draft(filter(), draft));
    assert_eq!(args(&editor), good);

    // a Check that does not find it leaves it to wait, and one that does has it taken
    let _ = editor.update(found(&["a"]));
    assert_eq!(args(&editor), good);
    let _ = editor.update(found(&["a", "b"]));
    assert_eq!(args(&editor), late);
    assert!(
        editor
            .drafts
            .is_empty()
    );
}

#[test]
fn a_value_that_a_node_using_it_would_refuse_is_not_taken() {
    let mut editor = chain(2);
    editor
        .pipeline
        .values
        .insert("top".to_owned(), serde_json::json!(2));
    let named = serde_json::json!({ "n": { "$value": "top" } });
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Args(named)));
    let put = |editor: &mut Editor, value: Value| {
        let put = form::Message::Put(String::new(), value);
        let _ = editor.update(Message::Drawer(drawer::Message::Form(
            "top".to_owned(),
            put,
        )));
    };
    let top = |editor: &Editor| {
        editor
            .pipeline
            .values["top"]
            .clone()
    };

    // a text is no number of rows: the limit that names it keeps the number
    put(&mut editor, serde_json::json!("x"));
    assert_eq!(top(&editor), 2);
    // and a number that it takes is the value
    put(&mut editor, serde_json::json!(7));
    assert_eq!(top(&editor), 7);
}

#[test]
fn a_tile_is_shown_open_while_the_cursor_rests_on_it_which_is_no_edit() {
    let mut editor = chain(2);
    let is_shown_open = |editor: &Editor| {
        simulator(editor.view())
            .find(field("n1"))
            .is_ok()
    };
    let is_open = |editor: &Editor| {
        layout::is_open(
            &editor
                .pipeline
                .nodes[1],
        )
    };
    assert!(!is_shown_open(&editor));

    let _ = editor.update(Message::Peek(Some("n1".to_owned())));
    assert!(is_shown_open(&editor));
    assert!(!is_open(&editor) && !editor.is_dirty);
    let _ = editor.update(Message::Peek(None));
    assert!(!is_shown_open(&editor));

    // its toggle keeps it open, and the next closes it though the cursor never left it
    let _ = editor.update(Message::Peek(Some("n1".to_owned())));
    let mut ui = simulator(editor.view());
    ui.click(theme::sign::OPEN)
        .unwrap();
    for message in ui
        .into_messages()
        .collect::<Vec<_>>()
    {
        let _ = editor.update(message);
    }
    assert!(
        is_open(&editor)
            && editor
                .peek
                .is_none()
    );
    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    assert!(!is_shown_open(&editor));

    // one renamed while the cursor rests on it stays open under its new id
    let _ = editor.update(Message::Peek(Some("n1".to_owned())));
    let rename = Edit::Rename("last".to_owned());
    let _ = editor.update(Message::Edit("n1".to_owned(), rename));
    assert_eq!(
        editor
            .peek
            .as_deref(),
        Some("last")
    );
}
