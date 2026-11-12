//! Deriving a [`Method`] from a row of a table: a call of the Rust method
//! that builds its node, with marked expressions to find in that node.

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

        let mut node = marked(call).map_err(|e| problem(&e))?;
        let mut found = Vec::new();
        take_markers(&mut node, "", &mut found);
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
            let (name, at, is_variadic) = match name.strip_prefix('*') {
                None => (name, at, false),
                Some(name) => {
                    let list = alone_in_list(&mut node, &at)
                        .ok_or_else(|| problem("a `*` input is not alone in a list"))?;
                    (name, list, true)
                }
            };
            args.push(Arg {
                name: name.to_owned(),
                at,
                is_variadic,
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

        let (variant, content) = variant(&node, self.at)
            .ok_or_else(|| problem(&format!("the call builds no node at {}", self.at)))?;
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

/// The node that `call` builds of marked expressions, to be found in it.
fn marked<const N: usize, B: Built>(call: fn(Expr, [Expr; N]) -> B) -> Result<Value, String> {
    let marker = |index: usize| col(format!("\0{index}"));
    let built = call(marker(0), std::array::from_fn(|i| marker(i + 1)))
        .built()
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&built).map_err(|e| e.to_string())
}

/// Empties the list that the value at `at` is alone in, and gives its pointer.
fn alone_in_list(
    node: &mut Value,
    at: &str,
) -> Option<String> {
    let (list, _) = at.rsplit_once('/')?;
    let items = node
        .pointer_mut(list)?
        .as_array_mut()?;
    (items.len() == 1).then(|| {
        items.clear();
        list.to_owned()
    })
}

/// The variant of the enum at `at` in `node`, and its payload if it has one.
fn variant<'a>(
    node: &'a Value,
    at: &str,
) -> Option<(&'a String, Option<&'a Value>)> {
    match node.pointer(at)? {
        Value::String(variant) => Some((variant, None)),
        Value::Object(tagged) if tagged.len() == 1 => tagged
            .iter()
            .next()
            .map(|(variant, content)| (variant, Some(content))),
        _ => None,
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
    let part = |key: String| {
        let at = format!("{kind}/{key}");
        (key, at)
    };
    let parts: Vec<(String, String)> = match content {
        None => vec![],
        Some(Value::Object(fields)) if is_struct => fields
            .keys()
            .cloned()
            .map(part)
            .collect(),
        Some(Value::Array(parts)) if is_tuple => (0..parts.len())
            .map(|i| part(i.to_string()))
            .collect(),
        Some(_) => vec![(String::new(), kind.to_owned())],
    };
    let free: Vec<(String, String)> = parts
        .into_iter()
        .filter(|(_, at)| !holds(at))
        .collect();

    let option = |(name, at)| Arg {
        name,
        at,
        is_variadic: false,
    };
    let lists_fields = |others: &[&str]| {
        others
            .iter()
            .all(|other| {
                free.iter()
                    .any(|(key, _)| key == other)
            })
    };
    match payload {
        Payload::Fields if is_struct || free.is_empty() => Some(
            free.into_iter()
                .map(option)
                .collect(),
        ),
        Payload::Except(others) if is_struct && lists_fields(others) => Some(
            free.into_iter()
                .filter(|(key, _)| !others.contains(&key.as_str()))
                .map(option)
                .collect(),
        ),
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
    at: &str,
    found: &mut Vec<(usize, String)>,
) {
    let index = value["Column"]
        .as_str()
        .and_then(|name| name.strip_prefix('\0'))
        .and_then(|index| index.parse().ok());
    if let Some(index) = index {
        found.push((index, at.to_owned()));
        *value = Value::Null;
        return;
    }

    match value {
        Value::Object(fields) => fields
            .iter_mut()
            .for_each(|(key, value)| take_markers(value, &format!("{at}/{key}"), found)),
        Value::Array(items) => items
            .iter_mut()
            .enumerate()
            .for_each(|(i, value)| take_markers(value, &format!("{at}/{i}"), found)),
        _ => {}
    }
}
