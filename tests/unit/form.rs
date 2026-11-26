use iced_test::{Error, Simulator};

use super::*;
use crate::gui::{look, simulator};
use crate::polars::prelude::*;
use crate::{Catalog, Registry, marker};

// what a chain asks for is passed on by its form
impl From<chain::Message> for Message {
    fn from(message: chain::Message) -> Self {
        Self::Chain(message)
    }
}

/// A form as its owner has it: its value, the draft of that, and what says if an edit is taken.
struct Shown {
    catalog: &'static Catalog,
    pipeline: &'static Pipeline,
    schema: &'static Value,
    expressions: Expressions,
    check: Check<'static>,
    columns: Vec<String>,
    state: State,
    value: Value,
    draft: Option<Draft>,
}

/// What lives as long as a simulator, which keeps the state of the widgets that show it.
fn kept<T>(value: T) -> &'static T {
    Box::leak(Box::new(value))
}

impl Shown {
    /// The form of the `args` of a node of `block`, whose expressions are picked by name.
    fn of(
        block: &'static str,
        args: Value,
        pipeline: Pipeline,
    ) -> Self {
        let registry = kept(Registry::with_polars());
        let catalog = kept(registry.catalog());
        let pipeline = kept(pipeline);
        Self {
            catalog,
            pipeline,
            schema: resolve(&catalog.defs, &catalog.blocks[block].args),
            expressions: Expressions::Named,
            check: Rc::new(move |args| {
                registry
                    .resolved(block, args, pipeline)
                    .map(drop)
                    .map_err(|e| line(&e))
            }),
            columns: Vec::new(),
            state: State::default(),
            value: args,
            draft: None,
        }
    }

    /// The form of a `select` of the expression `expr`, which is edited as a chain.
    fn chained(expr: Value) -> Self {
        Self {
            expressions: Expressions::Chained,
            ..Self::of("select", json!({ "exprs": [expr] }), Pipeline::default())
        }
    }

    /// The form of any JSON, all of which is taken.
    fn any(
        value: Value,
        pipeline: Pipeline,
    ) -> Self {
        Self {
            schema: &ANY,
            check: Rc::new(|_| Ok(())),
            ..Self::of("limit", value, pipeline)
        }
    }

    /// Does what `message` asks, as the owner of the value does what the form leaves it to do.
    fn send(
        &mut self,
        message: impl Into<Message>,
    ) -> Change {
        let draft = self.draft.as_ref();
        let change = self
            .state
            .update(message.into(), &self.value, draft, &*self.check);
        match &change {
            Change::Value(value) => {
                self.value = value.clone();
                self.draft = None;
            }
            Change::Draft(draft) => self.draft = Some(draft.clone()),
            Change::Nothing | Change::Named(..) => {}
        }
        change
    }

    /// Does all that was asked of a simulator of its view.
    fn take(
        &mut self,
        ui: Simulator<'_, Message>,
    ) {
        let asked: Vec<Message> = ui
            .into_messages()
            .collect();
        for message in asked {
            self.send(message);
        }
    }

    /// The value as the form shows it: as it is edited, if that is not taken yet.
    fn shown(&self) -> &Value {
        Draft::shown(self.draft.as_ref(), &self.value).0
    }

    /// Why the value as it is edited is not taken, if it is not.
    fn why(&self) -> Option<&str> {
        Draft::shown(self.draft.as_ref(), &self.value).1
    }

    fn view(&self) -> impl Widget<Message> + use<> {
        self.form()
            .view(self.schema)
    }

    /// What it is laid out with.
    fn form(&self) -> Form<'static, '_> {
        let draft = self
            .draft
            .clone()
            .map(kept);
        let (value, held) = Draft::shown(draft, kept(self.value.clone()));
        Form {
            state: &self.state,
            defs: &self.catalog.defs,
            pipeline: self.pipeline,
            offers: kept(Offers::default()),
            columns: &self.columns,
            expressions: self.expressions,
            check: &self.check,
            value,
            held,
            label_width: 0.0,
            help: crate::gui::Help::default(),
        }
    }
}

/// A method, or a function if `namespace` is `pl`.
fn method(
    namespace: &str,
    name: &str,
) -> &'static methods::Method {
    methods::methods()
        .iter()
        .find(|method| match namespace {
            "pl" => {
                method.name == name
                    && method
                        .receiver
                        .is_none()
            }
            _ => {
                method.name == name
                    && method.namespace == namespace
                    && method
                        .receiver
                        .is_some()
            }
        })
        .expect("the method is in the table")
}

fn json_of(expr: Expr) -> Value {
    serde_json::to_value(&expr).expect("an expression serializes")
}

/// A pipeline that names numbers, a text and a table, and expressions with and without holes.
fn named() -> Pipeline {
    let rounded = json!({ "Function": {
        "input": [json_of(col("a"))],
        "function": { "Round": { "decimals": { "$arg": "digits" }, "mode": "HalfToEven" } },
    } });
    let expressions = json!({
        "two": json_of(lit(2)),
        "sum": json_of(lit(1) + lit(2)),
        "x": json_of(col("x")),
        "rounded": rounded,
    });
    let values = json!({
        "top": 2,
        "label": "cost",
        "bands": { "cheap": { "lo": 0, "hi": 10 } },
    });
    let named = |named: Value| {
        named
            .as_object()
            .cloned()
            .unwrap_or_default()
    };
    Pipeline {
        expressions: named(expressions),
        values: named(values),
        ..Pipeline::default()
    }
}

/// What a drag of sixty pixels up from the one handle of `ui` asks for.
fn dragged(mut ui: Simulator<'_, Message>) -> Result<Vec<Message>, Error> {
    use iced::{Event, mouse, window};

    let from = ui
        .find(polaroid_iced::HANDLE)?
        .bounds()
        .center();
    let to = iced::Point::new(from.x, from.y - 60.0);
    ui.point_at(from);
    ui.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: from }),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
    ]);
    ui.point_at(to);
    ui.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: to }),
        Event::Window(window::Event::RedrawRequested(iced::time::Instant::now())),
    ]);
    Ok(ui
        .into_messages()
        .collect())
}

#[test]
fn a_chain_is_built_call_by_call() {
    let mut form = Shown::chained(Value::Null);
    let at = || "/exprs/0".to_owned();
    let put = |pointer: &str, value: &str| Message::Put(pointer.to_owned(), value.into());

    form.send(chain::Message::Replace(at(), method("pl", "col")));
    form.send(put("/exprs/0/Column", "name"));
    form.send(chain::Message::Call(at(), method("str", "to_lowercase")));
    form.send(chain::Message::Call(at(), method("", "alias")));
    form.send(put("/exprs/0/Alias/1", "lower"));

    let built = col("name")
        .str()
        .to_lowercase()
        .alias("lower");
    assert_eq!(form.value["exprs"][0], json_of(built));
}

#[test]
fn a_call_is_replaced_and_removed() {
    let lower = col("a")
        .str()
        .to_lowercase()
        .sum();
    let mut form = Shown::chained(json_of(lower));

    // `to_lowercase` is the receiver of `sum`, and keeps its own
    let replaced =
        chain::Message::Replace("/exprs/0/Agg/Sum".to_owned(), method("str", "to_uppercase"));
    form.send(replaced);
    let upper = col("a")
        .str()
        .to_uppercase();
    assert_eq!(form.value["exprs"][0], json_of(upper.clone().sum()));

    form.send(chain::Message::Remove("/exprs/0".to_owned()));
    assert_eq!(form.value["exprs"][0], json_of(upper.clone()));

    // a root leaves a hole, which waits to be filled as its block takes none
    let hole = chain::Message::Remove("/exprs/0/Function/input/0".to_owned());
    assert!(matches!(form.send(hole), Change::Draft(_)));
    assert_eq!(form.value["exprs"][0], json_of(upper));
    assert_eq!(form.shown()["exprs"][0]["Function"]["input"], json!([null]));
}

#[test]
fn laid_out_arguments_follow_their_call() {
    let contains = col("a")
        .str()
        .contains(lit("x"), true);
    let mut form = Shown::chained(json_of(contains));
    let at = || "/exprs/0".to_owned();
    let expanded = |form: &Shown| {
        form.state
            .chain
            .expanded
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    };

    // showing more is no change
    assert_eq!(form.send(chain::Message::Toggle(at())), Change::Nothing);
    assert_eq!(expanded(&form), ["/exprs/0"]);

    // `contains` is the receiver of `alias`, whose name is to be filled
    form.send(chain::Message::Call(at(), method("", "alias")));
    assert_eq!(expanded(&form), ["/exprs/0", "/exprs/0/Alias/0"]);

    form.send(chain::Message::Remove(at()));
    assert_eq!(expanded(&form), ["/exprs/0"]);
}

#[test]
fn arguments_are_written_until_they_are_laid_out() -> Result<(), Error> {
    let contains = col("a")
        .str()
        .contains(lit("x"), false);
    let mut form = Shown::chained(json_of(contains));
    let written = r#"("x", …)"#;

    let mut ui = simulator(form.view());
    ui.find(".str.")?;
    ui.find(written)?;
    assert!(
        ui.find("pattern=")
            .is_err()
    );

    // a double click lays them out, on them or anywhere to their right
    let brackets = ui
        .find(written)?
        .bounds();
    ui.point_at(iced::Point::new(
        brackets.x + brackets.width + 80.0,
        brackets.center_y(),
    ));
    ui.simulate(iced_test::simulator::click());
    ui.simulate(iced_test::simulator::click());
    form.take(ui);

    let mut ui = simulator(form.view());
    ui.find("pattern=")?;
    ui.find("strict=")?;
    assert!(
        ui.find(written)
            .is_err()
    );
    Ok(())
}

#[test]
fn a_value_is_given_the_default_or_missing() {
    let five = json!(5);
    // the default is told from what was given, though it be the same again
    assert_eq!(tone_of(&five, Some(&five), false), Tone::Default);
    assert_eq!(tone_of(&json!(6), Some(&five), false), Tone::Given);
    assert_eq!(tone_of(&five, None, false), Tone::Given);

    // nothing is missing where something has to be, and a null may be what is meant
    assert_eq!(tone_of(&Value::Null, None, false), Tone::Refused);
    // and a text that was typed and cleared is a text
    assert_eq!(tone_of(&json!(""), None, false), Tone::Given);
    assert_eq!(tone_of(&Value::Null, None, true), Tone::Given);
    assert_eq!(
        tone_of(&Value::Null, Some(&Value::Null), true),
        Tone::Default
    );
}

#[test]
fn a_number_that_is_being_typed_is_shown_as_typed_and_put_as_far_as_it_goes() -> Result<(), Error> {
    let mut form = Shown::of("limit", json!({ "n": 1 }), Pipeline::default());
    let typed = |text: &str| Message::Type("/n".to_owned(), text.to_owned());

    // on its way to a number it is the number so far, and is shown as it was typed
    assert_eq!(form.send(typed("1.")), Change::Nothing);
    simulator(form.view()).find("1.")?;
    assert_eq!(form.send(typed("15")), Change::Value(json!({ "n": 15 })));

    // what else is done ends the typing, and the field shows the value again
    form.send(typed("15."));
    form.send(chain::Message::Toggle(String::new()));
    let mut ui = simulator(form.view());
    assert!(
        ui.find("15.")
            .is_err()
    );
    ui.find("15")?;
    Ok(())
}

#[test]
fn an_edit_that_is_not_taken_waits_as_a_draft() {
    let mut form = Shown::of("limit", json!({ "n": 1 }), Pipeline::default());
    let typed = |text: &str| Message::Type("/n".to_owned(), text.to_owned());

    // none where its block takes a number: the node keeps the one it has
    assert!(matches!(form.send(typed("")), Change::Draft(_)));
    assert_eq!(form.value, json!({ "n": 1 }));
    assert_eq!(*form.shown(), json!({ "n": null }));
    // and more of what writes none is no other draft
    assert_eq!(form.send(typed("x")), Change::Nothing);

    // what its block takes again is the value, and no draft is left
    assert_eq!(form.send(typed("7")), Change::Value(json!({ "n": 7 })));
    assert_eq!(form.draft, None);
    // as is what it had, typed again
    form.send(typed(""));
    assert_eq!(form.send(typed("7")), Change::Value(json!({ "n": 7 })));

    // and what took none before either is mended as it is edited
    let mut form = Shown::of("limit", json!({ "n": null }), Pipeline::default());
    let put = Message::Put("/n".to_owned(), json!("x"));
    assert_eq!(form.send(put), Change::Value(json!({ "n": "x" })));
}

#[test]
fn why_an_edit_is_not_taken_is_said_at_the_field_it_names_or_under_them_all() -> Result<(), Error> {
    let waits = |why: &str| look::unmarked(&format!("Not applied until it is whole: {why}"));

    // the core names the field of a number that an expression is none for
    let mut form = Shown::of("limit", json!({ "n": 1 }), named());
    form.send(Message::Put("/n".to_owned(), marker("x")));
    let why = form
        .why()
        .expect("it waits")
        .to_owned();
    assert!(why.contains("`n`"), "{why}");
    let mut ui = simulator(form.view());
    // shown without the backticks that mark the field
    ui.find(look::unmarked(&why).as_str())?;
    assert!(
        ui.find(waits(&why).as_str())
            .is_err()
    );

    // and what names no field is said under them all
    let mut form = Shown::chained(json_of(col("a").sum()));
    form.send(chain::Message::Remove("/exprs/0/Agg/Sum".to_owned()));
    let why = form
        .why()
        .expect("it waits")
        .to_owned();
    simulator(form.view()).find(waits(&why).as_str())?;
    Ok(())
}

#[test]
fn a_description_flows_within_its_paragraphs() {
    assert_eq!(
        look::flowed("one\ntwo\n\nthree\nfour"),
        "one two\n\nthree four"
    );
}

#[test]
fn the_name_of_a_column_is_picked_from_the_columns_that_are_read() -> Result<(), Error> {
    let args = json!({ "column": null, "name": "to" });
    let mut form = Shown::of("write_files", args, Pipeline::default());

    // until a Check finds the columns the list is there, with none to pick
    let mut ui = simulator(form.view());
    ui.find("column…")?;
    ui.find("to")?;

    // and one that is not among them is shown all the same
    form.columns = vec!["plot".to_owned()];
    let mut ui = simulator(form.view());
    ui.find("to")?;
    ui.click("column…")?;
    ui.click("plot")?;
    let asked: Vec<Message> = ui
        .into_messages()
        .collect();
    assert!(
        matches!(&asked[..], [Message::Put(at, name)] if at == "/column" && name == "plot"),
        "{asked:?}"
    );
    Ok(())
}

#[test]
fn a_drag_across_a_number_in_an_expression_moves_it() -> Result<(), Error> {
    let moved = |expr: Expr| -> Result<Value, Error> {
        let mut form = Shown::chained(json_of(expr));
        // the arguments of its last call are laid out, to be found
        form.send(chain::Message::Toggle("/exprs/0".to_owned()));
        for message in dragged(simulator(form.view()))? {
            form.send(message);
        }
        Ok(form.value["exprs"][0].take())
    };
    // ten steps: of one for a whole number, which stays whole
    assert_eq!(moved(col("a").gt(lit(5)))?, json_of(col("a").gt(lit(15))));
    // and of a tenth for one that is not
    assert_eq!(
        moved(col("a").gt(lit(0.5)))?,
        json_of(col("a").gt(lit(1.5)))
    );
    // what is no number has no handle
    assert!(moved(col("a").eq(lit("x"))).is_err());
    Ok(())
}

/// What typing `new` over `old` in a field of `expr` leaves of it, shown again after each key.
fn retyped(
    expr: Expr,
    old: &str,
    new: &str,
) -> Result<Value, Error> {
    use iced::keyboard::key::Named;

    let mut form = Shown::chained(json_of(expr));
    // the arguments of its last call are laid out, to be found
    form.send(chain::Message::Toggle("/exprs/0".to_owned()));

    let mut ui = simulator(form.view());
    let _ = ui.click(old)?;
    let _ = ui.tap_key(Named::End);
    let erased = old
        .chars()
        .map(|_| None);
    let written = new
        .chars()
        .map(Some);
    for key in erased.chain(written) {
        let _ = match key {
            None => ui.tap_key(Named::Backspace),
            Some(typed) => ui.typewrite(&typed.to_string()),
        };
        for message in ui.drain() {
            form.send(message);
        }
        ui = ui.rebuild(form.view());
    }
    Ok(form.shown()["exprs"][0].clone())
}

#[test]
fn a_field_that_is_cleared_keeps_its_focus_for_what_is_typed_next() -> Result<(), Error> {
    // a literal is no number while it is empty or on its way to one
    let more = retyped(col("a").gt(lit(5)), "5", "2.5")?;
    assert_eq!(more, json_of(col("a").gt(lit(2.5))));
    let more = retyped(col("a").eq(lit("x")), "x", "12")?;
    assert_eq!(more, json_of(col("a").eq(lit(12))));
    // a number, which is none while it is empty, and the name of a column
    let round = |name: &str, decimals| col(name).round(decimals, RoundMode::HalfToEven);
    assert_eq!(retyped(round("a", 2), "2", "10")?, json_of(round("a", 10)));
    assert_eq!(retyped(round("a", 2), "a", "bc")?, json_of(round("bc", 2)));
    // and a name, which is a string while it is empty
    let named = retyped(col("a").alias("x"), "x", "yz")?;
    assert_eq!(named, json_of(col("a").alias("yz")));
    Ok(())
}

#[test]
fn the_methods_of_no_namespace_are_offered_under_a_name_of_their_own() -> Result<(), Error> {
    let mut form = Shown::chained(json_of(col("a")));
    // nothing to pick from until a call is to be added
    assert!(
        simulator(form.view())
            .find("base")
            .is_err()
    );
    form.send(chain::Message::Narrow("/exprs/0".to_owned(), ""));
    simulator(form.view()).find("base")?;

    // and to come back to from another
    form.send(chain::Message::Narrow("/exprs/0".to_owned(), "str"));
    let mut ui = simulator(form.view());
    let _ = ui.click("str")?;
    let _ = ui.click("base")?;
    let asked: Vec<Message> = ui
        .into_messages()
        .collect();
    assert!(matches!(
        asked.as_slice(),
        [Message::Chain(
            chain::Message::Narrow(_, "")
        )]
    ));
    Ok(())
}

#[test]
fn a_variant_is_shown_in_lower_case_and_kept_as_it_is_written() -> Result<(), Error> {
    let mut form = Shown::any(json!("TopRight"), Pipeline::default());
    form.schema = kept(json!({ "enum": ["TopRight", "bottom"] }));
    let mut ui = simulator(form.view());
    ui.click("top right")?;
    ui.click("bottom")?;
    let asked: Vec<Message> = ui
        .into_messages()
        .collect();
    assert!(
        matches!(&asked[..], [Message::Put(at, picked)] if at.is_empty() && picked == "bottom"),
        "{asked:?}"
    );
    Ok(())
}

#[test]
fn a_field_offers_the_names_of_the_pipeline_and_greys_those_that_do_not_fit() -> Result<(), Error> {
    let form = Shown::of("limit", json!({ "n": 5 }), named());
    let mut ui = simulator(form.view());
    ui.click("@…")?;
    // an expression, a value, and each part of a table
    for offered in [
        "@two",
        "@x",
        "@top",
        "@bands.keys",
        "@bands.cheap.hi",
    ] {
        ui.find(offered)?;
    }
    // what the core does not take there is listed, and a press on it picks nothing
    ui.click("@label")?;
    ui.click("@x")?;
    ui.click("@bands.cheap.hi")?;
    let asked: Vec<Message> = ui
        .into_messages()
        .collect();
    let hi = json!({ "$value": "bands", "at": "/cheap/hi" });
    assert!(
        matches!(&asked[..], [Message::Put(at, named)] if at == "/n" && *named == hi),
        "{asked:?}"
    );

    // and a pipeline that names nothing offers nothing
    let form = Shown::of("limit", json!({ "n": 5 }), Pipeline::default());
    assert!(
        simulator(form.view())
            .find("@…")
            .is_err()
    );
    Ok(())
}

#[test]
fn a_field_that_holds_a_name_shows_it_and_what_it_stands_for() -> Result<(), Error> {
    let part = json!({ "$value": "bands", "at": "/cheap/hi" });
    let form = Shown::of("limit", json!({ "n": part }), named());
    let mut ui = simulator(form.view());
    ui.find("@bands.cheap.hi")?;
    ui.find("10")?;

    // an expression as it is written, and one without a name too, until a name takes its place
    let args = json!({ "predicates": [marker("x"), json_of(col("y"))] });
    let form = Shown::of("filter", args, named());
    let mut ui = simulator(form.view());
    ui.find("@x")?;
    ui.find(r#"pl.col("x")"#)?;
    ui.find(r#"pl.col("y")"#)?;
    ui.find("expression…")?;
    Ok(())
}

#[test]
fn a_named_number_is_moved_where_it_is_named() -> Result<(), Error> {
    let moved = |named: Value| {
        let form = Shown::of("limit", json!({ "n": named }), self::named());
        dragged(simulator(form.view()))
    };
    // ten steps up move the value, not the arg that names it
    let asked = moved(json!({ "$value": "top" }))?;
    assert!(
        matches!(&asked[..], [.., Message::Name(name, at, moved)]
            if name == "top" && at.is_empty() && *moved == json!(12)),
        "{asked:?}"
    );
    // and of an expression its literal
    let asked = moved(marker("two"))?;
    let twelve = json_of(lit(12));
    assert!(
        matches!(&asked[..], [.., Message::Name(name, at, moved)]
            if name == "two" && at == "/Literal" && *moved == twelve["Literal"]),
        "{asked:?}"
    );
    // what is worked out of others is moved where those are
    assert!(moved(marker("sum")).is_err());

    // and the owner of the form is left to put it there
    let mut form = Shown::of("limit", json!({ "n": 1 }), Pipeline::default());
    let name = Message::Name("top".to_owned(), String::new(), json!(3));
    assert_eq!(
        form.send(name),
        Change::Named("top".to_owned(), String::new(), json!(3))
    );
    Ok(())
}

#[test]
fn a_call_lays_out_a_line_for_each_hole_of_its_expression() -> Result<(), Error> {
    let args = json!({ "exprs": [marker("rounded")] });
    let mut form = Shown::of("select", args, named());
    let mut ui = simulator(form.view());
    ui.find("@rounded")?;
    ui.find("digits=")?;

    // a hole that stands where a number does is given one, typed in
    let typed = Message::Type("/exprs/0/with/digits".to_owned(), "2".to_owned());
    let given = json!({ "exprs": [{ "$expr": "rounded", "with": { "digits": 2 } }] });
    assert_eq!(form.send(typed), Change::Value(given));
    simulator(form.view()).find("2")?;
    Ok(())
}

#[test]
fn a_hole_is_named_as_a_reference_writes_it() {
    let hole = |typed: &str| named::hole_of(typed);
    assert_eq!(hole("x"), json!({ "$arg": "x" }));
    assert_eq!(hole("band.lo"), json!({ "$arg": "band", "at": "/lo" }));
    assert_eq!(hole("rows[0].lo"), json!({ "$arg": "rows", "at": "/0/lo" }));
    // and on the way to it
    assert_eq!(hole(""), json!({ "$arg": "" }));
    assert_eq!(hole("band."), json!({ "$arg": "band", "at": "/" }));
    for typed in ["x", "band.lo", "rows[0].lo"] {
        let written = reference(&hole(typed)).map(|hole| hole.to_string());
        assert_eq!(written, Some(format!("${typed}")));
    }
}

#[test]
fn a_hole_in_an_expression_is_typed_where_it_stands() -> Result<(), Error> {
    use iced::keyboard::key::Named;

    let column = json!({ "Column": { "$arg": "column" } });
    let mut form = Shown::chained(column);
    let mut ui = simulator(form.view());
    ui.find("$")?;
    let _ = ui.click("column")?;
    let _ = ui.tap_key(Named::End);
    let _ = ui.typewrite("s");
    form.take(ui);
    assert_eq!(
        form.shown()["exprs"][0],
        json!({ "Column": { "$arg": "columns" } })
    );
    Ok(())
}

#[test]
fn an_input_that_may_be_left_out_is_given_and_left_out_again() -> Result<(), Error> {
    let bare = col("a").replace_strict(lit(1), lit(2), None, None::<DataType>);
    let given = col("a").replace_strict(lit(1), lit(2), Some(lit(0)), None::<DataType>);
    let mut form = Shown::chained(json_of(bare.clone()));
    form.send(chain::Message::Toggle("/exprs/0".to_owned()));
    simulator(form.view()).find("default=")?;

    // from nothing, as the hole that it shows is filled
    let default = "/exprs/0/Function/input/3".to_owned();
    form.send(chain::Message::Replace(
        default.clone(),
        method("pl", "lit"),
    ));
    form.send(Message::Put(
        format!("{default}/Literal"),
        methods::literal_of("0"),
    ));
    assert_eq!(form.shown()["exprs"][0], json_of(given));

    form.send(Message::Drop(default));
    assert_eq!(form.value["exprs"][0], json_of(bare));
    Ok(())
}

#[test]
fn any_json_is_laid_out_as_what_it_is() -> Result<(), Error> {
    // none yet is a kind to pick, which starts empty
    let mut form = Shown::any(Value::Null, Pipeline::default());
    let mut ui = simulator(form.view());
    ui.click("value…")?;
    ui.click("table")?;
    form.take(ui);
    assert_eq!(form.value, json!({}));

    // a table is a line of each key and its value, which is added to be named
    let mut ui = simulator(form.view());
    ui.click("+")?;
    form.take(ui);
    assert_eq!(form.value, json!({ "": null }));
    assert!(
        simulator(form.view())
            .find("+")
            .is_err()
    );

    // a number has a handle, a flag a box, and a list its elements
    let table = json!({ "lo": 5, "on": true, "of": ["a"] });
    let mut form = Shown::any(table.clone(), Pipeline::default());
    let mut ui = simulator(form.view());
    for shown in ["lo", "5", "on", "of", "a"] {
        ui.find(shown)?;
    }
    for message in dragged(ui)? {
        form.send(message);
    }
    assert_eq!(form.value["lo"], 15);

    // a number that is typed stays one while it is none
    form.send(Message::Type("/lo".to_owned(), String::new()));
    simulator(form.view()).find(polaroid_iced::HANDLE)?;
    // and a key is another in its place
    let entries = table
        .as_object()
        .expect("it is a table");
    assert_eq!(
        table::renamed(entries, "on", "off"),
        json!({ "lo": 5, "off": true, "of": ["a"] })
    );
    assert_eq!(table::renamed(entries, "on", "lo"), table);

    // one is taken out, and a key that a pointer cannot hold as it is is put as it is
    form.send(Message::Drop("/on".to_owned()));
    form.send(Message::Put("/a~1b~0".to_owned(), json!(1)));
    assert_eq!(form.value, json!({ "lo": null, "of": ["a"], "a/b~": 1 }));
    Ok(())
}

#[test]
fn rows_of_one_shape_are_laid_out_as_a_grid() -> Result<(), Error> {
    let bands = json!({ "cheap": { "lo": 0, "hi": 10 }, "dear": { "lo": 10, "hi": 1000 } });
    let mut form = Shown::any(bands.clone(), Pipeline::default());
    // each key once, over its column, and each row on a line
    let mut ui = simulator(form.view());
    let mut at = |shown: &str| -> Result<iced::Rectangle, Error> {
        Ok(ui
            .find(shown)?
            .bounds())
    };
    let (lo, hi, dear, most) = (at("lo")?, at("hi")?, at("dear")?, at("1000")?);
    assert!((lo.y - hi.y).abs() < 1.0 && lo.x < hi.x, "{lo:?} {hi:?}");
    assert!((dear.center_y() - most.center_y()).abs() < 4.0);
    assert!((hi.x - most.x).abs() < 1.0, "{hi:?} {most:?}");

    // a cell is put where it is in its row, and a key is another in every row at once
    form.send(Message::Put("/cheap/lo".to_owned(), json!(1)));
    assert_eq!(form.value["cheap"], json!({ "lo": 1, "hi": 10 }));
    let renamed = table::each(&bands, |cells| table::renamed(cells, "lo", "from"));
    let from = json!({ "cheap": { "from": 0, "hi": 10 }, "dear": { "from": 10, "hi": 1000 } });
    assert_eq!(renamed, from);

    // a list of such rows is one too, and rows of other keys or of more than cells are none
    let is_grid = |rows: Value| {
        form.form()
            .grid(&ANY, kept(rows), "")
            .is_some()
    };
    assert!(is_grid(json!([{ "lo": 0 }, { "lo": 10 }])));
    assert!(!is_grid(json!([{ "lo": 0 }, { "hi": 10 }])));
    assert!(!is_grid(json!({ "a": { "lo": [0] } })));
    assert!(!is_grid(json!({ "a": 1 })));
    assert!(!is_grid(json!([])));
    // nor is a name, which JSON writes as a table
    assert!(!is_grid(json!([{ "$value": "top" }])));
    Ok(())
}

#[test]
fn what_is_laid_out_is_folded_away_with_the_value_that_is_replaced() {
    let contains = col("a")
        .str()
        .contains(lit("x"), true);
    let mut form = Shown::chained(json_of(contains));
    let at = || "/exprs/0".to_owned();
    form.send(chain::Message::Toggle(at()));
    form.send(chain::Message::Narrow(at(), "str"));

    // an edit below it leaves it laid out, and its namespace offered
    let column = "/exprs/0/Function/input/0/Column".to_owned();
    form.send(Message::Put(column, "b".into()));
    assert!(
        form.state
            .chain
            .expanded
            .contains("/exprs/0")
    );
    simulator(form.view())
        .find("method")
        .expect("the methods of the namespace are offered");

    // and another value in its place is not laid out as it was
    form.send(Message::Put(at(), Value::Null));
    assert!(
        form.state
            .chain
            .expanded
            .is_empty()
    );
}

#[test]
fn a_key_is_typed_on_through_one_that_another_entry_has() -> Result<(), Error> {
    use iced::keyboard::key::Named;

    let mut form = Shown::any(json!({ "a": 1, "x": 2 }), Pipeline::default());
    let mut ui = simulator(form.view());
    let _ = ui.click("x")?;
    let _ = ui.tap_key(Named::End);
    // erased it is free, as `a` it is another's and held as typed, and as `ab` it is free again
    for (key, held, keys) in [
        (None, None, ["a", ""]),
        (Some("a"), Some("a"), ["a", ""]),
        (Some("b"), None, ["a", "ab"]),
    ] {
        let _ = match key {
            None => ui.tap_key(Named::Backspace),
            Some(typed) => ui.typewrite(typed),
        };
        for message in ui.drain() {
            form.send(message);
        }
        ui = ui.rebuild(form.view());
        let entries = form
            .value
            .as_object()
            .expect("it is a table");
        assert!(
            entries
                .keys()
                .eq(keys)
        );
        let typed = form
            .state
            .keying
            .as_ref()
            .map(|(_, typed)| typed.as_str());
        assert_eq!(typed, held);
    }
    Ok(())
}
