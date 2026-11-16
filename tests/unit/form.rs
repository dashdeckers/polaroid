use iced_test::{Error, simulator};

use super::*;
use crate::Registry;
use crate::polars::prelude::*;

/// The form for the args of a `select`, with what it is laid out with.
struct Form {
    catalog: Value,
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
        message: Message,
    ) -> bool {
        self.state
            .update(message, &mut self.args)
    }

    /// The index of a method, or of a function if `namespace` is `pl`.
    fn method(
        &self,
        namespace: &str,
        name: &str,
    ) -> usize {
        methods::methods()
            .iter()
            .position(|method| match namespace {
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
        let defs = &self.catalog["$defs"];
        let schema = resolve(defs, &self.catalog["blocks"]["select"]["args"]);
        view(&self.state, defs, schema, &self.args, vec![])
    }
}

fn json_of(expr: Expr) -> Value {
    serde_json::to_value(&expr).expect("an expression serializes")
}

#[test]
fn a_chain_is_built_call_by_call() {
    let mut form = Form::of(Value::Null);
    let at = || "/exprs/0".to_owned();
    let put =
        |pointer: &str, value: &str| Message::Edit(Edit(pointer.to_owned(), Some(value.into())));

    assert!(form.send(Message::Replace(at(), form.method("pl", "col"))));
    assert!(form.send(put("/exprs/0/Column", "name")));
    assert!(form.send(Message::Call(at(), form.method("str", "to_lowercase"))));
    assert!(form.send(Message::Call(at(), form.method("", "alias"))));
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
    let replaced = Message::Replace(
        "/exprs/0/Agg/Sum".to_owned(),
        form.method("str", "to_uppercase"),
    );
    assert!(form.send(replaced));
    let upper = col("a")
        .str()
        .to_uppercase();
    assert_eq!(form.args["exprs"][0], json_of(upper.clone().sum()));

    assert!(form.send(Message::Remove("/exprs/0".to_owned())));
    assert_eq!(form.args["exprs"][0], json_of(upper));

    // a root leaves a hole
    assert!(form.send(Message::Remove("/exprs/0/Function/input/0".to_owned())));
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
            .expanded
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    };

    // showing more is no change
    assert!(!form.send(Message::Toggle(at())));
    assert_eq!(expanded(&form), ["/exprs/0"]);

    // `contains` is the receiver of `alias`, whose name is to be filled
    assert!(form.send(Message::Call(at(), form.method("", "alias"))));
    assert_eq!(expanded(&form), ["/exprs/0", "/exprs/0/Alias/0"]);

    assert!(form.send(Message::Remove(at())));
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

    // a double click lays them out
    let _ = ui.click(written)?;
    let _ = ui.click(written)?;
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
