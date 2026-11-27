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

    let toggle = toggle_of(&editor, "n0");
    click_at(&mut editor, toggle);
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
        ui.find("Names")
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
    let toggle = toggle_of(&editor, "n1");
    click_at(&mut editor, toggle);
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

/// How far down the editor each of `texts` is shown.
fn tops<const N: usize>(
    editor: &Editor,
    texts: [&str; N],
) -> [f32; N] {
    let mut ui = simulator(editor.view());
    texts.map(|text| {
        ui.find(text)
            .unwrap()
            .bounds()
            .y
    })
}

#[test]
fn an_expression_is_listed_with_those_that_have_a_hole_once_it_has_one() {
    let mut editor = chain(1);
    let named = |editor: &mut Editor, name: &str, expression: Value| {
        editor
            .pipeline
            .expressions
            .insert(name.to_owned(), expression);
    };
    named(&mut editor, "plain", crate::marker("gone"));
    named(&mut editor, "holed", serde_json::json!({ "$arg": "x" }));
    editor
        .pipeline
        .values
        .insert("value".to_owned(), 1.into());
    let texts = [
        "plain",
        "Holed expressions",
        "holed",
        "Values",
        "value",
    ];
    assert!(tops(&editor, texts).is_sorted());

    // with a hole written in it, be it sound or not, it is listed with the others that have one
    named(&mut editor, "plain", serde_json::json!([{ "$arg": "x" }]));
    assert!(tops(&editor, ["Holed expressions", "plain"]).is_sorted());
}

#[test]
fn what_is_typed_for_a_name_stays_in_its_field_when_the_entry_before_it_goes() {
    let mut editor = chain(1);
    for name in ["a", "b"] {
        editor
            .pipeline
            .values
            .insert(name.to_owned(), 1.into());
    }
    let mut without = chain(1);
    without
        .pipeline
        .values
        .insert("b".to_owned(), 1.into());

    let mut ui = simulator(editor.view());
    ui.click(field("b"))
        .unwrap();
    ui.typewrite("c");
    let mut ui = ui.rebuild(without.view());
    assert!(
        ui.find(field("bc"))
            .is_ok()
    );
}

/// Has the editor do all that was asked of a simulator of it.
fn apply(
    editor: &mut Editor,
    asked: Vec<Message>,
) {
    for message in asked {
        let _ = editor.update(message);
    }
}

/// Presses what says `said` in the editor, which then does what that asks.
fn press_on(
    editor: &mut Editor,
    said: &str,
) {
    let mut ui = simulator(editor.view());
    ui.click(said)
        .unwrap();
    let asked = asked(ui);
    apply(editor, asked);
}

#[test]
fn a_list_is_shown_by_name_once_asked_which_the_pipeline_remembers() {
    let mut editor = chain(1);
    for name in ["b", "A"] {
        editor
            .pipeline
            .expressions
            .insert(name.to_owned(), Value::Null);
    }
    assert!(tops(&editor, ["b", "A"]).is_sorted());

    // its toggle says the order it is in, and a press on it asks for the other
    press_on(&mut editor, "file");
    assert!(tops(&editor, ["A", "b"]).is_sorted());
    assert_eq!(
        editor.pipeline.ui["sorted"],
        serde_json::json!(["expressions"])
    );
    assert!(editor.is_dirty);
    // in the file they are in the order they were
    let names: Vec<&String> = editor
        .pipeline
        .expressions
        .keys()
        .collect();
    assert_eq!(names, ["b", "A"]);

    press_on(&mut editor, "a–z");
    assert!(tops(&editor, ["b", "A"]).is_sorted());
    assert_eq!(editor.pipeline.ui["sorted"], serde_json::json!([]));
}

#[test]
fn a_name_given_on_enter_is_no_longer_typed_in_the_field_that_is_then_another_s() {
    // by name, `a` is listed before `ab`, and renamed `ac` after it
    let sorted = |names: [&str; 2]| {
        let mut editor = chain(1);
        for name in names {
            editor
                .pipeline
                .values
                .insert(name.to_owned(), 1.into());
        }
        editor.pipeline.ui = serde_json::json!({ "sorted": ["values"] });
        editor
    };
    let (before, after) = (sorted(["a", "ab"]), sorted(["ac", "ab"]));
    let mut ui = simulator(before.view());
    ui.click(field("a"))
        .unwrap();
    ui.typewrite("c");
    ui.tap_key(iced::keyboard::key::Named::Enter);
    assert_eq!(ui.drain().count(), 1);
    // the field that was typed in is that of `ab` now, which shows its own name
    let mut ui = ui.rebuild(after.view());
    for name in ["ab", "ac"] {
        assert!(
            ui.find(field(name))
                .is_ok(),
            "{name}"
        );
    }
}

/// Where each text that says `said` is shown, from the first down.
fn every(
    editor: &Editor,
    said: &str,
) -> Vec<iced::Rectangle> {
    use iced_test::selector::Candidate;

    let mut found = Vec::new();
    let _ = simulator(editor.view()).find(|candidate: Candidate<'_>| {
        if let Candidate::Text {
            content, bounds, ..
        } = candidate
            && content == said
        {
            found.push(bounds);
        }
        None::<()>
    });
    found
}

#[test]
fn the_toggle_of_a_list_without_an_add_is_where_those_of_the_others_are() {
    let editor = chain(1);
    let toggles = every(&editor, "file");
    assert_eq!(toggles.len(), 3);
    assert!(
        toggles
            .iter()
            .all(|toggle| toggle.x == toggles[0].x)
    );
}

/// Has the editor do what the mouse asks of it with the cursor at `at`, in a view of its own.
fn mouse(
    editor: &mut Editor,
    at: Point,
    events: impl IntoIterator<Item = iced::mouse::Event>,
) {
    let mut ui = simulator(editor.view());
    ui.point_at(at);
    ui.simulate(
        events
            .into_iter()
            .map(iced::Event::Mouse),
    );
    let asked = asked(ui);
    apply(editor, asked);
}

/// The expressions of the editor in the order of the file, their names run together.
fn expressions(editor: &Editor) -> String {
    editor
        .pipeline
        .expressions
        .keys()
        .map(String::as_str)
        .collect()
}

/// An editor of three expressions `a`, `b` and `c`, and of `h` after `a`, which has a hole.
fn named() -> Editor {
    let mut editor = chain(1);
    let expressions = [
        ("a", crate::marker("gone")),
        ("h", serde_json::json!({ "$arg": "x" })),
        ("b", crate::marker("gone")),
        ("c", crate::marker("gone")),
    ];
    for (name, expression) in expressions {
        editor
            .pipeline
            .expressions
            .insert(name.to_owned(), expression);
    }
    editor
}

#[test]
fn an_entry_is_let_go_before_or_after_another_of_its_list_which_moves_it_in_the_file() {
    use drawer::Message::{Cancel, Drop, Grab, Over};
    use drawer::Place::{After, Before};

    let mut editor = named();
    let drag = |editor: &mut Editor, name: &str, place: Option<drawer::Place>| {
        let over = place.map(Over);
        for message in [Grab(name.to_owned())]
            .into_iter()
            .chain(over)
            .chain([Drop])
        {
            let _ = editor.update(Message::Drawer(message));
        }
    };

    // those of the other lists stay where they are in the file
    drag(&mut editor, "c", Some(Before("a".to_owned())));
    assert_eq!(expressions(&editor), "cahb");
    assert!(editor.is_dirty);
    drag(&mut editor, "c", Some(After("b".to_owned())));
    assert_eq!(expressions(&editor), "ahbc");

    // where it is, it stays, and so it does let go at no place or outside the drawer
    editor.is_dirty = false;
    drag(&mut editor, "c", Some(After("b".to_owned())));
    drag(&mut editor, "c", Some(Before("c".to_owned())));
    drag(&mut editor, "c", Some(After("gone".to_owned())));
    drag(&mut editor, "a", None);
    for message in [
        Grab("a".to_owned()),
        Over(After("c".to_owned())),
        Cancel,
        Drop,
    ] {
        let _ = editor.update(Message::Drawer(message));
    }
    assert_eq!(expressions(&editor), "ahbc");
    assert!(!editor.is_dirty);
}

#[test]
fn an_entry_is_dragged_by_its_grip_in_a_list_that_is_in_the_order_of_the_file() {
    use iced::mouse::Button::Left;
    use iced::mouse::Event::{ButtonPressed, ButtonReleased, CursorMoved};

    let mut editor = named();
    // the grips of `a`, `b` and `c`, then that of `h` in the list under theirs
    let grips: Vec<Point> = every(&editor, theme::sign::GRIP)
        .iter()
        .map(|grip| grip.center())
        .collect();
    assert_eq!(grips.len(), 4);
    let moved = |position| CursorMoved { position };

    // to where the button is let go
    mouse(&mut editor, grips[0], [ButtonPressed(Left)]);
    mouse(&mut editor, grips[2], [moved(grips[2])]);
    mouse(&mut editor, grips[2], [ButtonReleased(Left)]);
    assert_eq!(expressions(&editor), "hbac");

    // and nowhere once the cursor leaves the drawer
    let outside = Point::new(900.0, grips[0].y);
    mouse(&mut editor, grips[0], [ButtonPressed(Left)]);
    let mut ui = simulator(editor.view());
    ui.point_at(grips[2]);
    ui.simulate([iced::Event::Mouse(moved(
        grips[2],
    ))]);
    ui.point_at(outside);
    ui.simulate([iced::Event::Mouse(moved(
        outside,
    ))]);
    let asked = asked(ui);
    apply(&mut editor, asked);
    mouse(&mut editor, grips[2], [moved(grips[2])]);
    mouse(&mut editor, grips[2], [ButtonReleased(Left)]);
    assert_eq!(expressions(&editor), "hbac");

    // by name there is nothing to drag by, and the names are where they were
    let left = |editor: &Editor| {
        simulator(editor.view())
            .find(field("a"))
            .unwrap()
            .bounds()
            .x
    };
    let was = left(&editor);
    press_on(&mut editor, "file");
    assert_eq!(every(&editor, theme::sign::GRIP).len(), 1);
    assert_eq!(left(&editor), was);
}

#[test]
fn a_right_click_on_a_name_picks_its_tags_which_follow_it() {
    use iced::mouse::Button::Right;
    use iced::mouse::Event::ButtonPressed;

    let mut editor = chain(1);
    for name in ["rate", "base"] {
        editor
            .pipeline
            .values
            .insert(name.to_owned(), 1.into());
    }
    let name = |editor: &Editor, name: &str| {
        simulator(editor.view())
            .find(field(name))
            .unwrap()
            .bounds()
    };
    let at = name(&editor, "rate").center();
    mouse(&mut editor, at, [ButtonPressed(Right)]);
    let marks = |editor: &mut Editor, message| {
        let _ = editor.update(Message::Marks(message));
    };
    for tag in ["tax", "vat"] {
        marks(&mut editor, marks::Message::Type(tag.to_owned()));
        marks(&mut editor, marks::Message::Add);
    }
    assert_eq!(
        editor.pipeline.ui["tags"],
        serde_json::json!({ "rate": ["tax", "vat"] })
    );
    assert!(editor.is_dirty);
    // its field is as wide with the dots of its tags in it as that of a name without any
    assert_eq!(name(&editor, "rate").width, name(&editor, "base").width);

    // its picker lists each, to take it off again by its tick, until Escape closes it
    press_on(&mut editor, "vat");
    assert_eq!(
        editor.pipeline.ui["tags"],
        serde_json::json!({ "rate": ["tax"] })
    );
    let lists = |editor: &Editor| {
        simulator(editor.view())
            .find("tax")
            .is_ok()
    };
    assert!(lists(&editor));
    let _ = editor.update(Message::Dismiss);
    assert!(!lists(&editor));
    // and its field is typed in under the dots as any other
    let mut ui = simulator(editor.view());
    ui.click(field("rate"))
        .unwrap();
    ui.typewrite("s");
    assert!(
        ui.find(field("rates"))
            .is_ok()
    );
    drop(ui);

    // under another name it carries the same, and deleted it carries none
    let drawer = |editor: &mut Editor, message| {
        let _ = editor.update(Message::Drawer(message));
    };
    drawer(
        &mut editor,
        drawer::Message::Rename("rate".to_owned(), "due".to_owned()),
    );
    assert_eq!(
        editor.pipeline.ui["tags"],
        serde_json::json!({ "due": ["tax"] })
    );
    drawer(&mut editor, drawer::Message::Delete("due".to_owned()));
    assert_eq!(editor.pipeline.ui["tags"], serde_json::json!({}));
}

#[test]
fn a_save_writes_no_tag_of_what_is_no_longer_named() {
    let mut editor = chain(1);
    let path = std::env::temp_dir().join(format!("polaroid-tags-{}.json", std::process::id()));
    editor.path = path
        .to_string_lossy()
        .into_owned();
    editor.pipeline.ui = serde_json::json!({
        "tags": { "gone": ["old"] },
        "colors": { "tags": { "old": "#111111" } },
    });
    let _ = editor.update(Message::Save);
    assert!(
        editor
            .pipeline
            .ui
            .is_null()
    );
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(!written.contains("old"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_search_narrows_the_drawer_to_what_has_every_word_typed() {
    use crate::polars::prelude::col;

    let mut editor = chain(1);
    let price = serde_json::to_value(col("price")).unwrap();
    editor
        .pipeline
        .expressions
        .insert("net".to_owned(), price);
    for name in ["rate", "Net_rate"] {
        editor
            .pipeline
            .values
            .insert(name.to_owned(), 1.into());
    }
    editor.pipeline.ui = serde_json::json!({ "tags": { "rate": ["Tax"] } });
    let listed = |editor: &mut Editor, query: &str| {
        let search = drawer::Message::Search(query.to_owned());
        let _ = editor.update(Message::Drawer(search));
        let mut ui = simulator(editor.view());
        ["net", "rate", "Net_rate"]
            .into_iter()
            .filter(|name| {
                ui.find(field(name))
                    .is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(listed(&mut editor, " "), ["net", "rate", "Net_rate"]);
    // in a name, whatever the case, in a tag, and in what it is written as
    assert_eq!(listed(&mut editor, "NET"), ["net", "Net_rate"]);
    assert_eq!(listed(&mut editor, "tax"), ["rate"]);
    assert_eq!(listed(&mut editor, "price"), ["net"]);
    assert_eq!(listed(&mut editor, "rate net"), ["Net_rate"]);
    assert!(listed(&mut editor, "rate price").is_empty());
    assert!(!editor.is_dirty);

    // what is narrowed has nothing to drag by, until the search is cleared by its button
    assert_eq!(listed(&mut editor, "e"), ["net", "rate", "Net_rate"]);
    assert!(every(&editor, theme::sign::GRIP).is_empty());
    press_on(&mut editor, theme::sign::DELETE);
    assert_eq!(every(&editor, theme::sign::GRIP).len(), 3);
}

#[test]
fn what_is_typed_in_the_search_narrows_as_it_is_typed() {
    let mut editor = chain(1);
    for name in ["rate", "base"] {
        editor
            .pipeline
            .values
            .insert(name.to_owned(), 1.into());
    }
    let mut ui = simulator(editor.view());
    ui.click(field("search"))
        .unwrap();
    ui.typewrite("b");
    let asked = asked(ui);
    apply(&mut editor, asked);
    let mut ui = simulator(editor.view());
    assert!(
        ui.find(field("base"))
            .is_ok()
    );
    assert!(
        ui.find(field("rate"))
            .is_err()
    );
}

/// Where the toggle of the open card of node `id` is: the sign before its id, on the line of it.
fn toggle_of(
    editor: &Editor,
    id: &str,
) -> Point {
    let id = simulator(editor.view())
        .find(field(id))
        .unwrap()
        .bounds();
    [
        theme::sign::OPEN,
        theme::sign::CLOSE,
    ]
    .into_iter()
    .flat_map(|sign| every(editor, sign))
    .map(|sign| sign.center())
    .filter(|sign| sign.x < id.x && (id.y..id.y + id.height).contains(&sign.y))
    .max_by(|one, other| {
        one.x
            .total_cmp(&other.x)
    })
    .unwrap()
}

#[test]
fn the_search_laid_open_lists_the_tags_in_use_over_the_categories() {
    let mut editor = chain(1);
    editor
        .pipeline
        .values
        .insert("rate".to_owned(), 1.into());
    editor.pipeline.ui = serde_json::json!({ "tags": { "rate": ["tax"], "gone": ["old"] } });
    let lists = |editor: &Editor, text: &str| {
        simulator(editor.view())
            .find(field(text))
            .is_ok()
    };
    assert!(!lists(&editor, "tax"));
    // its toggle is the first there is
    press_on(&mut editor, theme::sign::OPEN);
    assert!(lists(&editor, "tax"));
    let category = editor
        .catalog
        .categories[0];
    assert!(tops(&editor, ["tax", category]).is_sorted());
    // a tag of what the pipeline does not name is none in use
    assert!(!lists(&editor, "old"));
    assert!(!editor.is_dirty);
    // what is searched for narrows the tags as it does the names
    let _ = editor.update(Message::Drawer(drawer::Message::Search("ra".to_owned())));
    assert!(!lists(&editor, "tax") && lists(&editor, "rate"));
    let _ = editor.update(Message::Drawer(drawer::Message::Search(String::new())));

    // renamed there it is renamed where it is carried, and taken off everything it is gone
    let _ = editor.update(Message::Marks(marks::Message::Rename(
        "tax".to_owned(),
        "vat".to_owned(),
    )));
    assert_eq!(
        editor.pipeline.ui["tags"]["rate"],
        serde_json::json!(["vat"])
    );
    assert!(editor.is_dirty);
    let _ = editor.update(Message::Marks(marks::Message::Remove("vat".to_owned())));
    assert!(!lists(&editor, "vat"));
}

/// Has the editor do what a click at `at` asks of it.
fn click_at(
    editor: &mut Editor,
    at: Point,
) {
    use iced::mouse::Button::Left;
    use iced::mouse::Event::{ButtonPressed, ButtonReleased};

    mouse(
        editor,
        at,
        [
            ButtonPressed(Left),
            ButtonReleased(Left),
        ],
    );
}

#[test]
fn a_right_click_on_a_tile_or_on_the_head_of_an_open_card_picks_the_tags_of_its_node() {
    use iced::mouse::Button::Right;
    use iced::mouse::Event::ButtonPressed;

    let mut editor = chain(2);
    let center = |editor: &Editor, text: &str| {
        simulator(editor.view())
            .find(text)
            .unwrap()
            .bounds()
            .center()
    };
    let tag = |editor: &mut Editor, at: Point, tag: &str| {
        mouse(editor, at, [ButtonPressed(Right)]);
        for message in [
            marks::Message::Type(tag.to_owned()),
            marks::Message::Add,
        ] {
            let _ = editor.update(Message::Marks(message));
        }
    };
    // on the id of its tile, and once the card is open on its toggle
    let at = center(&editor, "n1");
    tag(&mut editor, at, "slow");
    assert_eq!(
        editor
            .pipeline
            .nodes[1]
            .ui["tags"],
        serde_json::json!(["slow"])
    );
    assert!(editor.is_dirty);
    // its picker lists the tag under the tile, until Escape closes it
    assert!(center(&editor, "slow").y > at.y);
    let _ = editor.update(Message::Dismiss);
    assert!(
        simulator(editor.view())
            .find("slow")
            .is_err()
    );

    let _ = editor.update(Message::Edit("n1".to_owned(), Edit::Toggle));
    let at = toggle_of(&editor, "n1");
    tag(&mut editor, at, "big");
    assert_eq!(
        editor
            .pipeline
            .nodes[1]
            .ui["tags"],
        serde_json::json!(["big", "slow"])
    );
    // renamed, the node has its tags still, and the other node has none
    let _ = editor.update(Message::Edit(
        "n1".to_owned(),
        Edit::Rename("last".to_owned()),
    ));
    assert_eq!(
        editor
            .pipeline
            .nodes[1]
            .ui["tags"],
        serde_json::json!(["big", "slow"])
    );
    assert!(
        editor
            .pipeline
            .nodes[0]
            .ui["tags"]
            .is_null()
    );
}

#[test]
fn the_glass_floats_in_the_corner_of_the_graph_and_what_it_rings_is_no_edit() {
    let mut editor = chain(1);
    let glass = simulator(editor.view())
        .find(theme::sign::GLASS)
        .unwrap()
        .bounds();
    let id = simulator(editor.view())
        .find("n0")
        .unwrap()
        .bounds();
    assert!(glass.x > id.x && glass.y > id.y);
    press_on(&mut editor, theme::sign::GLASS);
    let category = editor
        .catalog
        .categories[0];
    press_on(&mut editor, category);
    assert!(!editor.is_dirty);
    // Escape closes its menu
    let _ = editor.update(Message::Dismiss);
    assert!(
        simulator(editor.view())
            .find(category)
            .is_err()
    );
}
