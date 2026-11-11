//! Deriving a [`Method`] from a row of a table: a call of the Rust method
//! that builds its node, with marked expressions to find in that node.

use std::fmt::Write;

use polars::prelude::*;
use serde_json::Value;

use super::{Arg, Method};

/// The Methods whose nodes are the variants of one enum.
pub(super) struct Namespace {
    pub(super) name: &'static str,
    /// The JSON pointer, from a node, to that enum.
    pub(super) at: &'static str,
}

/// How the payload of a node gives a Method its options.
#[derive(Clone, Copy)]
pub(super) enum Payload {
    /// Its fields are the options. A node without payload has none.
    Fields,
    /// Its fields are the options, but for these: they tell the Method from
    /// the others of its node, or go with the inputs it takes.
    Except(&'static [&'static str]),
    /// It has no field names. These name it: one for all of it, or one for
    /// each part of a tuple.
    Named(&'static [&'static str]),
}

/// What the call of a row returns: an expression, or Polars' refusal to build
/// one.
pub(super) trait Built {
    fn built(self) -> PolarsResult<Expr>;
}

impl Built for Expr {
    fn built(self) -> PolarsResult<Expr> {
        Ok(self)
    }
}

impl Built for PolarsResult<Expr> {
    fn built(self) -> PolarsResult<Expr> {
        self
    }
}

/// A row may give inputs a default.
pub(super) trait WithDefault {
    /// Gives each of `inputs` the default `value`, in place of null.
    ///
    /// Returns `Err` if the Method has no such input, or it is variadic.
    fn with_default<const N: usize>(
        self,
        inputs: [&str; N],
        value: Expr,
    ) -> Self;
}

impl WithDefault for Result<Method, String> {
    fn with_default<const N: usize>(
        self,
        inputs: [&str; N],
        value: Expr,
    ) -> Self {
        let mut method = self?;
        let problem = |what: &str| format!("{}.{}: {what}", method.namespace, method.name);
        let value = serde_json::to_value(&value).map_err(|e| problem(&e.to_string()))?;

        for name in inputs {
            let at = method
                .inputs
                .iter()
                .find(|input| input.name == name && !input.is_variadic)
                .map(|input| &input.at)
                .ok_or_else(|| problem(&format!("no input `{name}` to give a default")))?;
            *method
                .node
                .pointer_mut(at)
                .expect("a node has its inputs") = value.clone();
        }
        Ok(method)
    }
}

impl Namespace {
    /// Derives the Method called `name` from `call`, which calls the Rust
    /// method that builds its node.
    ///
    /// `call` takes the receiver and then one expression for each of `inputs`,
    /// which names them. A function leaves the receiver out. An input named
    /// with a `*` is variadic: `call` puts it alone in a list. The other
    /// arguments that `call` passes become the defaults of the options, which
    /// `payload` finds.
    ///
    /// Returns `Err` if `call` fails or builds no node of this namespace, if
    /// the node does not hold each input exactly once, or if the payload is
    /// not what `payload` says.
    pub(super) fn method<const N: usize, B: Built>(
        &self,
        name: &'static str,
        inputs: [&'static str; N],
        payload: Payload,
        call: fn(Expr, [Expr; N]) -> B,
    ) -> Result<Method, String> {
        let problem = |what: &str| format!("{}.{name}: {what}", self.name);

        // marked expressions go in, to be found in the node that comes out
        let marker = |index: usize| col(format!("\0{index}"));
        let built = call(marker(0), std::array::from_fn(|i| marker(i + 1)))
            .built()
            .map_err(|e| problem(&e.to_string()))?;
        let mut node = serde_json::to_value(&built).map_err(|e| problem(&e.to_string()))?;

        let mut found = Vec::new();
        take_markers(&mut node, &mut String::new(), &mut found);
        found.sort();
        let receiver = match found.first() {
            Some((0, _)) => Some(found.remove(0).1),
            _ => None,
        };
        let is_each_once = found
            .iter()
            .map(|(index, _)| *index)
            .eq(1..=N);
        if !is_each_once {
            return Err(problem("the node does not hold each input exactly once"));
        }

        let mut args = Vec::new();
        for (name, (_, at)) in inputs
            .into_iter()
            .zip(found)
        {
            let Some(name) = name.strip_prefix('*') else {
                args.push(Arg {
                    name: name.to_owned(),
                    at,
                    is_variadic: false,
                });
                continue;
            };
            // it stands for the list that it is alone in
            let list = at
                .rsplit_once('/')
                .map(|(list, _)| list)
                .filter(|list| {
                    node.pointer(list)
                        .and_then(Value::as_array)
                        .is_some_and(|items| items.len() == 1)
                })
                .ok_or_else(|| problem("a `*` input is not alone in a list"))?;
            *node
                .pointer_mut(list)
                .expect("the list was just read") = Value::Array(vec![]);
            args.push(Arg {
                name: name.to_owned(),
                at: list.to_owned(),
                is_variadic: true,
            });
        }

        // whether the value at this pointer is or holds the receiver or an input
        let holds = |at: &str| {
            receiver
                .iter()
                .chain(
                    args.iter()
                        .map(|arg| &arg.at),
                )
                .filter_map(|slot| slot.strip_prefix(at))
                .any(|rest| rest.is_empty() || rest.starts_with('/'))
        };

        // a variant is its name, or its name around its payload
        let (variant, content) = match node.pointer(self.at) {
            Some(Value::String(variant)) => (variant, None),
            Some(Value::Object(tagged)) if tagged.len() == 1 => {
                let (variant, content) = tagged
                    .iter()
                    .next()
                    .expect("one field");
                (variant, Some(content))
            }
            _ => return Err(problem(&format!("the call builds no node at {}", self.at))),
        };
        let kind = format!("{}/{variant}", self.at);
        let options = options(&kind, content, payload, holds)
            .ok_or_else(|| problem("the payload is not what the row says"))?;

        Ok(Method {
            namespace: self.name,
            name,
            kind,
            receiver,
            inputs: args,
            options,
            node,
        })
    }
}

/// The options that `payload` finds in `content`, the payload of a node of
/// this `kind`. `holds` tells whether the value at a JSON pointer is or holds
/// the receiver or an input, which are no options.
///
/// Returns `None` if the payload is not what `payload` says.
fn options(
    kind: &str,
    content: Option<&Value>,
    payload: Payload,
    holds: impl Fn(&str) -> bool,
) -> Option<Vec<Arg>> {
    // the payload in parts, each by its key: the fields of a struct, the
    // parts of a tuple, or else the payload as a whole
    let is_struct = content.is_some_and(Value::is_object) && !matches!(payload, Payload::Named(_));
    let is_tuple = holds(kind) || matches!(payload, Payload::Named(names) if names.len() > 1);
    let keys: Vec<String> = match content {
        None => vec![],
        Some(Value::Object(fields)) if is_struct => fields
            .keys()
            .cloned()
            .collect(),
        Some(Value::Array(parts)) if is_tuple => (0..parts.len())
            .map(|i| i.to_string())
            .collect(),
        Some(_) => vec![String::new()],
    };
    let free: Vec<(String, String)> = keys
        .into_iter()
        .map(|key| {
            let at = match key.as_str() {
                "" => kind.to_owned(),
                key => format!("{kind}/{key}"),
            };
            (key, at)
        })
        .filter(|(_, at)| !holds(at))
        .collect();

    let option = |(name, at)| Arg {
        name,
        at,
        is_variadic: false,
    };
    let is_free = |key: &&str| {
        free.iter()
            .any(|(free, _)| free == key)
    };
    match payload {
        Payload::Fields if is_struct || free.is_empty() => Some(
            free.into_iter()
                .map(option)
                .collect(),
        ),
        Payload::Except(others)
            if is_struct
                && others
                    .iter()
                    .all(is_free) =>
        {
            Some(
                free.into_iter()
                    .filter(|(key, _)| !others.contains(&key.as_str()))
                    .map(option)
                    .collect(),
            )
        }
        Payload::Named(names) if names.len() == free.len() => Some(
            names
                .iter()
                .zip(free)
                .map(|(name, (_, at))| option(((*name).to_owned(), at)))
                .collect(),
        ),
        _ => None,
    }
}

/// Replaces each marked expression in `value` with null, and notes its index
/// and its JSON pointer. `at` is the pointer to `value`.
fn take_markers(
    value: &mut Value,
    at: &mut String,
    found: &mut Vec<(usize, String)>,
) {
    let index = value["Column"]
        .as_str()
        .and_then(|name| name.strip_prefix('\0'))
        .and_then(|index| index.parse().ok());
    if let Some(index) = index {
        found.push((index, at.clone()));
        *value = Value::Null;
        return;
    }

    let len = at.len();
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let _ = write!(at, "/{key}");
                take_markers(value, at, found);
                at.truncate(len);
            }
        }
        Value::Array(items) => {
            for (i, value) in items
                .iter_mut()
                .enumerate()
            {
                let _ = write!(at, "/{i}");
                take_markers(value, at, found);
                at.truncate(len);
            }
        }
        _ => {}
    }
}
