//! An expression laid out as its chain of calls: the root, then one call to a line.

use iced::widget::{column, container, mouse_area, pick_list, row, text_input};
use iced::{Center, Element, Fill, Font, Widget, padding};
use serde_json::{Value, json};

use super::{Edit, Form, Message, small};
use crate::methods::{self, Method};
use crate::schema;

/// How far a call is set in from its root, and arguments from their call.
const INDENT: f32 = 20.0;

/// Arguments longer than this are not written while folded away.
const WRITTEN: usize = 32;

/// About how wide a letter of code is.
const LETTER: f32 = 9.0;

/// A Method to pick, by its index among the Methods.
#[derive(Debug, Clone, PartialEq)]
struct Choice {
    index: usize,
    label: String,
}

impl<'a> Form<'a, '_> {
    /// The expression `value` at the pointer `at` as its chain; a hole offers the functions.
    pub(super) fn chain(
        self,
        value: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        // the calls from the last one back to the root, and what is left
        let mut calls = Vec::new();
        let mut node = value;
        let mut pointer = at.clone();
        let rest = loop {
            let Some(method) = methods::read(methods::methods(), node) else {
                break Some((node, pointer));
            };
            calls.push((method, node, pointer.clone()));
            let Some(receiver) = &method.receiver else {
                break None;
            };
            pointer.push_str(receiver);
            node = node
                .pointer(receiver)
                .unwrap_or(&Value::Null);
        };

        let root = rest.map(|(node, pointer)| match node {
            Value::Null => {
                let functions = self.choices(|method| {
                    method
                        .receiver
                        .is_none()
                });
                pick_list(None::<Choice>, functions, |choice| choice.label.clone())
                    .on_select(move |choice| Message::Replace(pointer.clone(), choice.index))
                    .placeholder("pl.…")
                    .padding([2, 6])
                    .width(fitted("pl.…"))
                    .font(Font::MONOSPACE)
                    .boxed()
            }
            _ => self.plain(&self.defs["Expr"], node, pointer),
        });
        let calls = calls
            .into_iter()
            .rev()
            .map(|(method, node, pointer)| self.call(method, node, pointer));
        let add = (!value.is_null()).then(|| self.add(at));

        let lines = root
            .into_iter()
            .chain(calls)
            .chain(add)
            .enumerate()
            .map(|(i, line)| match i {
                0 => line,
                _ => container(line)
                    .padding(padding::left(INDENT))
                    .boxed(),
            });
        column(lines)
            .spacing(5)
            .boxed()
    }

    /// One call of a chain: its name, which picks another in its place, and its arguments.
    fn call(
        self,
        method: &'a Method,
        node: &'a Value,
        at: String,
    ) -> Element<'a, Message> {
        let is_function = method
            .receiver
            .is_none();
        let before = match (is_function, method.namespace) {
            (true, _) => String::new(),
            (false, "") => ".".to_owned(),
            (false, namespace) => format!(".{namespace}."),
        };
        // it may be replaced by those that are called the same way
        let others = self.choices(|other| match is_function {
            true => other
                .receiver
                .is_none(),
            false => {
                other
                    .receiver
                    .is_some()
                    && other.namespace == method.namespace
            }
        });
        let selected = others
            .iter()
            .find(|choice| std::ptr::eq(&methods::methods()[choice.index], method))
            .cloned();
        let width = fitted(
            selected
                .as_ref()
                .map_or("", |choice| &choice.label),
        );
        let name = pick_list(selected, others, |choice| choice.label.clone())
            .on_select({
                let at = at.clone();
                move |choice| Message::Replace(at.clone(), choice.index)
            })
            .padding([2, 6])
            .width(width)
            .font(Font::MONOSPACE);
        let remove = small("×", Message::Remove(at.clone()));
        let toggle =
            |label: String| mouse_area(code(label)).on_double_click(Message::Toggle(at.clone()));

        let has_arguments = !(method
            .inputs
            .is_empty()
            && method
                .options
                .is_empty());
        let inline = self.inline(method, node, &at);
        let is_laid_out = inline.is_none()
            && has_arguments
            && self
                .state
                .expanded
                .contains(&at);
        let after: Element<'a, Message> = match inline {
            Some(inline) => row![code("("), inline, code(")")]
                .align_y(Center)
                .boxed(),
            None if !has_arguments => code("()").boxed(),
            None if is_laid_out => toggle("(".to_owned()).boxed(),
            None => {
                let written = methods::arguments(methods::methods(), method, node);
                let is_short = written
                    .chars()
                    .count()
                    <= WRITTEN;
                match is_short {
                    true => toggle(format!("({written})")).boxed(),
                    false => toggle("(…)".to_owned()).boxed(),
                }
            }
        };
        let line = row![
            code(before),
            name,
            after,
            remove
        ]
        .spacing(2)
        .align_y(Center);
        if !is_laid_out {
            return line.boxed();
        }

        let expr = &self.defs["Expr"];
        let arguments = method
            .inputs
            .iter()
            .chain(&method.options)
            .map(|arg| {
                let value = node
                    .pointer(&arg.at)
                    .unwrap_or(&Value::Null);
                row![
                    code(format!("{}=", arg.name)),
                    self.value(
                        schema::pointer(self.defs, expr, &arg.at),
                        value,
                        format!("{at}{}", arg.at)
                    )
                ]
                .spacing(5)
                .boxed()
            });
        column![
            line,
            container(column(arguments).spacing(5)).padding(padding::left(INDENT)),
            toggle(")".to_owned()),
        ]
        .spacing(5)
        .boxed()
    }

    /// The argument of a `col` or a plain `lit`, to edit in place: a column or a value.
    fn inline(
        self,
        method: &Method,
        node: &'a Value,
        at: &str,
    ) -> Option<Element<'a, Message>> {
        let option = method
            .options
            .first()?;
        let value = node.pointer(&option.at)?;
        let pointer = format!("{at}{}", option.at);

        match method.kind.as_str() {
            "/Column" => {
                let name = value
                    .as_str()
                    .unwrap_or_default();
                let put =
                    move |name: String| Message::Edit(Edit(pointer.clone(), Some(name.into())));
                if self
                    .columns
                    .is_empty()
                {
                    return Some(
                        text_input("column", name)
                            .on_input(put)
                            .width(Fill.max(150))
                            .boxed(),
                    );
                }
                // it shows a name that is none of the columns, as one named before they were known
                let selected = self
                    .columns
                    .iter()
                    .find(|column| *column == name)
                    .cloned();
                let placeholder = match name {
                    "" => "column…",
                    name => name,
                };
                Some(
                    pick_list(
                        selected,
                        self.columns
                            .to_vec(),
                        String::clone,
                    )
                    .on_select(put)
                    .placeholder(placeholder.to_owned())
                    .boxed(),
                )
            }
            "/Literal" => {
                let typed = typed(value)?;
                let put = move |typed: String| {
                    Message::Edit(Edit(pointer.clone(), Some(literal(&typed))))
                };
                Some(
                    text_input("value", typed)
                        .on_input(put)
                        .width(Fill.max(150))
                        .boxed(),
                )
            }
            _ => None,
        }
    }

    /// The line that adds a call to the chain at `at`: its namespaces, then their methods.
    fn add(
        self,
        at: String,
    ) -> Element<'a, Message> {
        let narrowed = match &self.state.namespace {
            Some((pointer, namespace)) if *pointer == at => *namespace,
            _ => {
                return row![
                    code("."),
                    small("+", Message::Narrow(at, ""))
                ]
                .align_y(Center)
                .boxed();
            }
        };

        let mut namespaces: Vec<&'static str> = Vec::new();
        for method in methods::methods() {
            let is_new = method
                .receiver
                .is_some()
                && !method
                    .namespace
                    .is_empty()
                && !namespaces.contains(&method.namespace);
            if is_new {
                namespaces.push(method.namespace);
            }
        }
        // back to the methods of the expression itself
        if !narrowed.is_empty() {
            namespaces.insert(0, "");
        }
        let selected = (!narrowed.is_empty()).then_some(narrowed);
        let namespace = pick_list(selected, namespaces, |namespace| match *namespace {
            "" => "(none)".to_owned(),
            namespace => namespace.to_owned(),
        })
        .on_select({
            let at = at.clone();
            move |namespace| Message::Narrow(at.clone(), namespace)
        })
        .placeholder("namespace")
        .padding([2, 6])
        .width(fitted(selected.unwrap_or("namespace")))
        .font(Font::MONOSPACE);

        let methods = self.choices(|method| {
            method
                .receiver
                .is_some()
                && method.namespace == narrowed
        });
        let method = pick_list(None::<Choice>, methods, |choice| choice.label.clone())
            .on_select(move |choice| Message::Call(at.clone(), choice.index))
            .placeholder("method")
            .padding([2, 6])
            .width(fitted("method"))
            .font(Font::MONOSPACE);

        row![
            code("."),
            namespace,
            code("."),
            method
        ]
        .spacing(2)
        .align_y(Center)
        .boxed()
    }

    /// The Methods that `is_offered` holds for, the most picked first; a function by its full name.
    fn choices(
        self,
        is_offered: impl Fn(&Method) -> bool,
    ) -> Vec<Choice> {
        let mut choices: Vec<Choice> = methods::methods()
            .iter()
            .enumerate()
            .filter(|(_, method)| is_offered(method))
            .map(|(index, method)| {
                let label = match (&method.receiver, method.namespace) {
                    (Some(_), _) => method
                        .name
                        .to_owned(),
                    (None, "") => format!("pl.{}", method.name),
                    (None, namespace) => format!("{namespace}.{}", method.name),
                };
                Choice { index, label }
            })
            .collect();
        choices.sort_by_key(|choice| {
            let is_first = ["pl.col", "pl.lit", "alias"].contains(
                &choice
                    .label
                    .as_str(),
            );
            (!is_first, choice.label.clone())
        });
        choices
    }
}

/// The width of a pick list showing `label`, which would else be as wide as its widest choice.
fn fitted(label: &str) -> f32 {
    // the letters, the padding and the arrow
    label
        .chars()
        .count() as f32
        * LETTER
        + 36.0
}

/// Text that reads as code.
fn code<'a>(text: impl Into<String>) -> iced::widget::Text<'a> {
    iced::widget::text(text.into()).font(Font::MONOSPACE)
}

/// A plain literal as it is typed; a string in quotes only if it would read as something else.
fn typed(value: &Value) -> Option<String> {
    let number = [
        &value["Dyn"]["Int"],
        &value["Dyn"]["Float"],
    ]
    .into_iter()
    .find(|number| number.is_number());
    let string = [
        &value["Dyn"]["Str"],
        &value["Scalar"]["String"],
    ]
    .into_iter()
    .find_map(Value::as_str);

    match (number, string, &value["Scalar"]) {
        (Some(number), _, _) => Some(number.to_string()),
        (_, Some(string), _) => match literal(string)["Scalar"]["String"] == *string {
            true => Some(string.to_owned()),
            false => Some(format!("\"{string}\"")),
        },
        (_, _, scalar) if scalar["Boolean"] == true => Some("True".to_owned()),
        (_, _, scalar) if scalar["Boolean"] == false => Some("False".to_owned()),
        (_, _, scalar) if scalar["Null"].is_string() => Some("None".to_owned()),
        _ => None,
    }
}

/// The literal that `typed` stands for: a number, `True`, `False`, `None`, or else a string.
fn literal(typed: &str) -> Value {
    let number = typed
        .parse::<serde_json::Number>()
        .ok()
        .filter(|number| number.to_string() == typed);
    let quoted = typed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'));

    match (number, typed) {
        (Some(number), _) if number.is_f64() => json!({ "Dyn": { "Float": number } }),
        (Some(number), _) => json!({ "Dyn": { "Int": number } }),
        (None, "True") => json!({ "Scalar": { "Boolean": true } }),
        (None, "False") => json!({ "Scalar": { "Boolean": false } }),
        (None, "None") => json!({ "Scalar": { "Null": "Null" } }),
        (None, _) => json!({ "Scalar": { "String": quoted.unwrap_or(typed) } }),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/chain.rs"]
mod tests;
