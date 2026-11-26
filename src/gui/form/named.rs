//! The names in a value: an expression, a value or a part of one, a call, and a hole.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use iced::widget::{column, container, row};
use iced::{Center, Element, Font, Widget, padding};
use polaroid_iced::theme::{Tone, font, spacing};
use serde_json::{Number, Value, json};

use super::{
    ANY,
    Expressions,
    FLAG,
    Form,
    INDENT,
    Message,
    NUMBER,
    Offer,
    TEXT,
    escaped,
    put,
    stepped,
};
use crate::gui::look::{aside, code, fitted, pick, scrubbed};
use crate::schema::{self, nudged};
use crate::{Kind, Pipeline, Reference, Taken, methods, reference};

impl<'a> Form<'a, '_> {
    /// The face that a name in it is written in: an expression edited here is code.
    fn face(self) -> Font {
        match self.expressions {
            Expressions::Named => font::SANS,
            Expressions::Chained => font::MONO,
        }
    }

    /// Every name that a field may hold: listed when a field first offers them.
    fn offers(self) -> Vec<Offer> {
        let listed = || {
            let offer =
                |reference: Reference| Rc::new((reference.to_string(), Value::from(&reference)));
            let mut offers: Vec<Offer> = self
                .pipeline
                .expressions
                .keys()
                .map(|name| offer(Reference::to(Kind::Expression, name)))
                .collect();
            for (name, value) in &self.pipeline.values {
                parts(name, value, String::new(), &mut |part| {
                    offers.push(offer(part))
                });
            }
            // an expression edited here may have holes: those it has, and one more
            if self.expressions == Expressions::Chained {
                let mut holes = Vec::new();
                holes_in(self.value, &mut holes);
                let new = (1..)
                    .map(|n| format!("hole_{n}"))
                    .find(|new| !holes.contains(&new.as_str()))
                    .expect("some number is free");
                let holes = holes
                    .into_iter()
                    .chain([new.as_str()]);
                offers.extend(holes.map(|hole| offer(Reference::to(Kind::Argument, hole))));
            }
            offers
        };
        self.offers
            .get_or_init(listed)
            .clone()
    }

    /// A list of the names that the field at `at` may hold: one the core refuses there is greyed.
    pub(super) fn names(
        self,
        schema: &Value,
        value: &Value,
        at: &str,
        placeholder: &str,
        tone: Tone,
    ) -> Option<Element<'a, Message>> {
        let offers = self.offers();
        let selected =
            reference(value).map(|reference| Rc::new((reference.to_string(), value.clone())));
        if offers.is_empty() && selected.is_none() {
            return None;
        }
        // where an expression stands, a value stands as the literal that it is
        let is_expression = std::ptr::eq(schema, &self.defs["Expr"]);
        let placed = move |named: &Value| {
            let is_value = reference(named).is_some_and(|named| named.kind == Kind::Value);
            match is_expression && is_value {
                true => json!({ "Literal": named }),
                false => named.clone(),
            }
        };
        let (pipeline, root, check) = (self.pipeline, self.value, Rc::clone(self.check));
        let (put_at, tried_at) = (at.to_owned(), at.to_owned());
        // each is asked of the core once, when the list opens
        let asked = RefCell::new(BTreeMap::new());
        let why = move |offer: &Offer| {
            let (label, named) = &**offer;
            asked
                .borrow_mut()
                .entry(label.clone())
                .or_insert_with(|| {
                    // a call is judged once its arguments are given
                    let is_call = crate::marked(named)
                        .and_then(|name| {
                            pipeline
                                .holes(name)
                                .ok()
                        })
                        .is_some_and(|holes| !holes.is_empty());
                    if is_call {
                        return None;
                    }
                    let mut tried = root.clone();
                    put(&mut tried, &tried_at, Some(placed(named)));
                    check(&tried).err()
                })
                .clone()
        };
        let label = |offer: &Offer| offer.0.clone();
        let pick = pick(selected, offers, label, placeholder, self.face(), tone)
            .on_select(move |offer: Offer| Message::Put(put_at.clone(), placed(&offer.1)))
            .disabled(why);
        Some(self.help.of(
            pick,
            "Names an expression, a value or a part of a value of the pipeline, or a hole of the \
             expression, to stand here. One that is not taken here is greyed, with why.",
        ))
    }

    /// A field that holds a name: the name, what it stands for, and its arguments if it has holes.
    pub(super) fn named(
        self,
        schema: &'a Value,
        value: &'a Value,
        at: String,
        tone: Tone,
    ) -> Element<'a, Message> {
        let Some(reference) = reference(value) else {
            return self.plain(schema, value, at, tone);
        };
        // a hole is named here, as a part of what fills it may be: `band.lo`, `rows[0]`
        if reference.kind == Kind::Argument {
            let typed = reference.to_string()[1..].to_owned();
            let hole = fitted("hole", typed, font::MONO, tone)
                .on_input(move |typed| Message::Put(at.clone(), hole_of(&typed)));
            return row![code("$"), hole]
                .align_y(Center)
                .boxed();
        }
        let (written, number) = stands(self.pipeline, &reference).unwrap_or_default();
        // a plain number is moved where it is named, for all that names it
        let moved = number.map(|(inside, number)| {
            let is_literal = reference.kind == Kind::Expression;
            let name = reference
                .name
                .to_owned();
            let number = Value::Number(number);
            let schema = stepped(schema, &number);
            move |steps| {
                let moved = nudged(schema, &number, steps);
                let moved = match is_literal {
                    true => methods::literal_of(&moved.to_string()),
                    false => moved,
                };
                Message::Name(name.clone(), inside.clone(), moved)
            }
        });
        let line = row![
            self.names(schema, value, &at, "", tone),
            scrubbed(aside(written).font(font::MONO), moved, self.help)
        ]
        .spacing(spacing::XS)
        .align_y(Center);

        // what a call gives each hole of its expression, on a line of its own
        let holes = match reference.kind {
            Kind::Expression => self
                .pipeline
                .holes(reference.name)
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let given = holes
            .into_iter()
            .map(|(hole, taken)| {
                let given = reference
                    .with
                    .and_then(|with| with.get(hole))
                    .unwrap_or(&Value::Null);
                let taken = match taken {
                    Taken::Expression => &self.defs["Expr"],
                    Taken::Text => &*TEXT,
                    Taken::Number => &*NUMBER,
                    Taken::Flag => &*FLAG,
                    Taken::Any => &ANY,
                };
                row![
                    code(format!("{hole}=")),
                    self.value(taken, given, format!("{at}/with/{}", escaped(hole)))
                ]
                .spacing(spacing::XS)
                .align_y(Center)
                .boxed()
            });
        let given: Vec<_> = given.collect();
        let given = (!given.is_empty())
            .then(|| container(column(given).spacing(spacing::XS)).padding(padding::left(INDENT)));
        column![line, given]
            .spacing(spacing::XS)
            .boxed()
    }
}

/// What `reference` stands for as it is written, and the plain number it is, by its pointer there.
fn stands(
    pipeline: &Pipeline,
    reference: &Reference,
) -> Option<(String, Option<(String, Number)>)> {
    if reference.kind == Kind::Expression {
        let expression = pipeline
            .expressions
            .get(reference.name)?;
        let number = methods::typed(expression)
            .and_then(|typed| schema::number(&typed))
            .map(|number| ("/Literal".to_owned(), number));
        return Some((methods::text(expression), number));
    }
    let part = pipeline
        .values
        .get(reference.name)?
        .pointer(reference.at)?;
    Some(match (reference.part, part) {
        (Some("keys"), Value::Object(fields)) => (
            json!(
                fields
                    .keys()
                    .collect::<Vec<_>>()
            )
            .to_string(),
            None,
        ),
        (Some("values"), Value::Object(fields)) => (
            json!(
                fields
                    .values()
                    .collect::<Vec<_>>()
            )
            .to_string(),
            None,
        ),
        (_, Value::Number(number)) => {
            let at = reference
                .at
                .to_owned();
            (number.to_string(), Some((at, number.clone())))
        }
        _ => (part.to_string(), None),
    })
}

/// Gives `found` the value `name` and each part of it: keys, values and entries, or rows.
fn parts<'a>(
    name: &'a str,
    value: &Value,
    at: String,
    found: &mut impl FnMut(Reference),
) {
    let part = |part| Reference {
        at: &at,
        part,
        ..Reference::to(Kind::Value, name)
    };
    found(part(None));
    match value {
        // a name in it is no table
        _ if reference(value).is_some() => {}
        Value::Object(fields) => {
            found(part(Some("keys")));
            found(part(Some("values")));
            for (key, field) in fields {
                parts(name, field, format!("{at}/{}", escaped(key)), found);
            }
        }
        Value::Array(rows) => {
            for (i, row) in rows
                .iter()
                .enumerate()
            {
                parts(name, row, format!("{at}/{i}"), found);
            }
        }
        _ => {}
    }
}

/// Notes in `holes` the name of each hole in `value`, once.
fn holes_in<'a>(
    value: &'a Value,
    holes: &mut Vec<&'a str>,
) {
    match (reference(value), value) {
        (Some(hole), _) if hole.kind == Kind::Argument => {
            if !holes.contains(&hole.name) {
                holes.push(hole.name);
            }
        }
        (_, Value::Array(items)) => items
            .iter()
            .for_each(|item| holes_in(item, holes)),
        (_, Value::Object(fields)) => fields
            .values()
            .for_each(|field| holes_in(field, holes)),
        _ => {}
    }
}

/// The hole that `typed` names, or the part of one: `band.lo` and `rows[0]` as a reference writes.
pub(super) fn hole_of(typed: &str) -> Value {
    let mut steps = typed
        .split(['.', '['])
        .map(|step| step.trim_end_matches(']'));
    let name = steps
        .next()
        .unwrap_or_default();
    let at: String = steps
        .map(|step| format!("/{}", escaped(step)))
        .collect();
    Value::from(&Reference {
        at: &at,
        ..Reference::to(Kind::Argument, name)
    })
}
