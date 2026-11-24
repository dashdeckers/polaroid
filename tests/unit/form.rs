use iced_test::{Error, simulator};

use super::*;
use crate::Registry;
use crate::polars::prelude::*;

// what a chain asks for is passed on by its form
impl From<chain::Message> for Message {
    fn from(message: chain::Message) -> Self {
        Self::Chain(message)
    }
}

/// The form for the args of a `select`, with what it is laid out with.
struct Form {
    catalog: crate::Catalog,
    state: State,
    args: Value,
}

impl Form {
    fn of(expr: Value) -> Self {
        Form {
            catalog: Registry::with_polars().catalog(),
            state: State::default(),
            args: json!({ "exprs": [expr] }),
        }
    }

    fn send(
        &mut self,
        message: impl Into<Message>,
    ) -> bool {
        self.state
            .update(message.into(), &mut self.args)
    }

    /// A method, or a function if `namespace` is `pl`.
    fn method(
        &self,
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

    fn view(&self) -> impl Widget<Message> {
        let defs = &self.catalog.defs;
        let schema = resolve(defs, &self.catalog.blocks["select"].args);
        view(
            &self.state,
            defs,
            schema,
            &self.args,
            Expressions::Chained(&[]),
            0.0,
        )
    }
}

fn json_of(expr: Expr) -> Value {
    serde_json::to_value(&expr).expect("an expression serializes")
}

#[test]
fn a_chain_is_built_call_by_call() {
    let mut form = Form::of(Value::Null);
    let at = || "/exprs/0".to_owned();
    let put = |pointer: &str, value: &str| Message::Put(pointer.to_owned(), value.into());

    assert!(form.send(chain::Message::Replace(at(), form.method("pl", "col"))));
    assert!(form.send(put("/exprs/0/Column", "name")));
    assert!(form.send(chain::Message::Call(
        at(),
        form.method("str", "to_lowercase")
    )));
    assert!(form.send(chain::Message::Call(at(), form.method("", "alias"))));
    assert!(form.send(put("/exprs/0/Alias/1", "lower")));

    let built = col("name")
        .str()
        .to_lowercase()
        .alias("lower");
    assert_eq!(form.args["exprs"][0], json_of(built));
}

#[test]
fn a_call_is_replaced_and_removed() {
    let lower = col("a")
        .str()
        .to_lowercase()
        .sum();
    let mut form = Form::of(json_of(lower));

    // `to_lowercase` is the receiver of `sum`, and keeps its own
    let replaced = chain::Message::Replace(
        "/exprs/0/Agg/Sum".to_owned(),
        form.method("str", "to_uppercase"),
    );
    assert!(form.send(replaced));
    let upper = col("a")
        .str()
        .to_uppercase();
    assert_eq!(form.args["exprs"][0], json_of(upper.clone().sum()));

    assert!(form.send(chain::Message::Remove("/exprs/0".to_owned())));
    assert_eq!(form.args["exprs"][0], json_of(upper));

    // a root leaves a hole
    assert!(form.send(chain::Message::Remove(
        "/exprs/0/Function/input/0".to_owned()
    )));
    assert_eq!(form.args["exprs"][0]["Function"]["input"], json!([null]));
}

#[test]
fn laid_out_arguments_follow_their_call() {
    let contains = col("a")
        .str()
        .contains(lit("x"), true);
    let mut form = Form::of(json_of(contains));
    let at = || "/exprs/0".to_owned();
    let expanded = |form: &Form| {
        form.state
            .chain
            .expanded
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    };

    // showing more is no change
    assert!(!form.send(chain::Message::Toggle(at())));
    assert_eq!(expanded(&form), ["/exprs/0"]);

    // `contains` is the receiver of `alias`, whose name is to be filled
    assert!(form.send(chain::Message::Call(at(), form.method("", "alias"))));
    assert_eq!(expanded(&form), ["/exprs/0", "/exprs/0/Alias/0"]);

    assert!(form.send(chain::Message::Remove(at())));
    assert_eq!(expanded(&form), ["/exprs/0"]);
}

#[test]
fn arguments_are_written_until_they_are_laid_out() -> Result<(), Error> {
    let contains = col("a")
        .str()
        .contains(lit("x"), false);
    let mut form = Form::of(json_of(contains));
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
    let messages: Vec<Message> = ui
        .into_messages()
        .collect();
    for message in messages {
        form.send(message);
    }

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
    assert_eq!(Tone::of(&five, Some(&five), false), Tone::Default);
    assert_eq!(Tone::of(&json!(6), Some(&five), false), Tone::Given);
    assert_eq!(Tone::of(&five, None, false), Tone::Given);

    // nothing is missing where something has to be, and a null may be what is meant
    assert_eq!(Tone::of(&Value::Null, None, false), Tone::Missing);
    // and a text that was typed and cleared is a text
    assert_eq!(Tone::of(&json!(""), None, false), Tone::Given);
    assert_eq!(Tone::of(&Value::Null, None, true), Tone::Given);
    assert_eq!(
        Tone::of(&Value::Null, Some(&Value::Null), true),
        Tone::Default
    );
}

#[test]
fn a_number_that_is_being_typed_is_shown_but_not_put_in_the_value() -> Result<(), Error> {
    let catalog = Registry::with_polars().catalog();
    let defs = &catalog.defs;
    let schema = resolve(defs, &catalog.blocks["limit"].args);
    let mut state = State::default();
    let mut args = json!({ "n": 1 });
    let typed = |text: &str| Message::Type("/n".to_owned(), text.to_owned());

    // on its way to a number it is none, and is shown as it was typed
    assert!(state.update(typed("1."), &mut args));
    assert_eq!(args, json!({ "n": null }));
    simulator(view(
        &state,
        defs,
        schema,
        &args,
        Expressions::Chained(&[]),
        0.0,
    ))
    .find("1.")?;
    // more of what is no number changes nothing
    assert!(!state.update(typed("1.e"), &mut args));

    assert!(state.update(typed("15"), &mut args));
    assert_eq!(args, json!({ "n": 15 }));

    // what else is done ends the typing, and the field shows the value again
    assert!(state.update(typed("-"), &mut args));
    let toggle = Message::Chain(chain::Message::Toggle(String::new()));
    let _ = state.update(toggle, &mut args);
    let mut ui = simulator(view(
        &state,
        defs,
        schema,
        &args,
        Expressions::Chained(&[]),
        0.0,
    ));
    assert!(
        ui.find("-")
            .is_err()
    );
    Ok(())
}

#[test]
fn a_description_flows_within_its_paragraphs() {
    assert_eq!(flowed("one\ntwo\n\nthree\nfour"), "one two\n\nthree four");
}

#[test]
fn the_name_of_a_column_is_picked_from_the_columns_that_are_read() {
    let catalog = Registry::with_polars().catalog();
    let defs = &catalog.defs;
    let schema = resolve(defs, &catalog.blocks["write_files"].args);
    let (state, named) = (State::default(), Map::new());
    let args = json!({ "column": null, "name": "to" });
    let pick = |columns: &[String]| {
        let named = Expressions::Named(&named, columns);
        let mut ui = simulator(view(&state, defs, schema, &args, named, 0.0));
        (
            ui.find("column…")
                .is_ok(),
            ui.find("to")
                .is_ok(),
        )
    };

    // one that is not among them is shown all the same
    assert_eq!(pick(&["plot".to_owned()]), (true, true));
    // and until a Check finds the columns, a name is typed
    assert_eq!(pick(&[]), (false, true));
}

/// What a drag of sixty pixels up from the handle of its number leaves of `expr`.
fn dragged(expr: Expr) -> Result<Value, Error> {
    use iced::{Event, mouse, window};

    let mut form = Form::of(json_of(expr));
    // the arguments of its last call are laid out, to be found
    form.send(chain::Message::Toggle("/exprs/0".to_owned()));
    let mut ui = simulator(form.view());
    let from = ui
        .find(polaroid_iced::HANDLE)?
        .bounds()
        .center();
    let to = iced::Point::new(from.x, from.y - 60.0);
    ui.point_at(from);
    ui.simulate([Event::Mouse(
        mouse::Event::ButtonPressed(mouse::Button::Left),
    )]);
    ui.point_at(to);
    ui.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: to }),
        Event::Window(window::Event::RedrawRequested(iced::time::Instant::now())),
    ]);
    let asked: Vec<Message> = ui
        .into_messages()
        .collect();
    for message in asked {
        form.send(message);
    }
    Ok(form.args.take())
}

#[test]
fn a_drag_across_a_number_in_an_expression_moves_it() -> Result<(), Error> {
    // ten steps: of one for a whole number, which stays whole
    let more = dragged(col("a").gt(lit(5)))?;
    assert_eq!(more["exprs"][0], json_of(col("a").gt(lit(15))));
    // and of a tenth for one that is not
    let more = dragged(col("a").gt(lit(0.5)))?;
    assert_eq!(more["exprs"][0], json_of(col("a").gt(lit(1.5))));
    // what is no number has no handle
    assert!(dragged(col("a").eq(lit("x"))).is_err());
    Ok(())
}

#[test]
fn the_methods_of_no_namespace_are_offered_under_a_name_of_their_own() -> Result<(), Error> {
    let mut form = Form::of(json_of(col("a")));
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
    let base = ui
        .find("str")?
        .bounds();
    // the first of the list that opened under it
    ui.point_at(iced::Point::new(
        base.center_x(),
        base.y + base.height * 1.5,
    ));
    ui.simulate(iced_test::simulator::click());
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
