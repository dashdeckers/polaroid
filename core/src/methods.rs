//! Expression nodes read as method calls, the way Python Polars writes them.
//!
//! An expression is stored in Polars' JSON format. A [`Method`] says how one
//! kind of node in it reads as a call such as `.str.contains(pattern)`: what
//! the node looks like, and where its receiver and arguments sit.
//!
//! Each Method is derived from a call of the Rust method that builds its node,
//! so it cannot disagree with Polars. The tests check the Methods against the
//! JSON Schema of the expressions, which lists every node there is.

mod array;
mod binary;
mod categorical;
mod expr;
mod function;
mod list;
mod string;
mod structs;
mod temporal;
#[cfg(test)]
mod tests;
mod text;

use std::fmt::{Display, Write};

use polars::prelude::*;
use serde_json::Value;

pub use self::text::{arguments, text};

/// How one kind of expression node reads as a method call.
#[derive(Debug, Clone)]
pub struct Method {
    /// The `str` of `.str.contains()`.
    pub namespace: &'static str,
    pub name: &'static str,
    /// The node's kind: the JSON pointer to the enum that tells it, then its
    /// variant there. `/Function/function/StringExpr/Contains`.
    pub kind: String,
    /// The JSON pointer, from the node, to the expression the method is
    /// called on. `None` for a function, which is called on nothing.
    pub receiver: Option<String>,
    /// The expressions the method takes.
    pub inputs: Vec<Arg>,
    /// Its other arguments.
    pub options: Vec<Arg>,
    /// The node, with its options and inputs at their defaults. Its receiver
    /// is null, as is an input without default, and a variadic input is
    /// empty.
    node: Value,
}

/// An argument of a [`Method`].
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: String,
    /// The JSON pointer, from the node, to its value.
    pub at: String,
    /// Whether its value is an array of any number of expressions, not one
    /// expression.
    pub is_variadic: bool,
}

impl Method {
    /// The node for this method called on `receiver`, which a function
    /// ignores. Its options and inputs are at their defaults. An input
    /// without default is null, still to be filled.
    pub fn call(
        &self,
        receiver: Value,
    ) -> Value {
        let mut node = self.node.clone();
        if let Some(at) = &self.receiver {
            *node
                .pointer_mut(at)
                .expect("a node has its receiver") = receiver;
        }
        node
    }

    /// Whether `node` is a call of this method, whatever its receiver and
    /// arguments are.
    pub fn reads(
        &self,
        node: &Value,
    ) -> bool {
        let slots: Vec<&str> = self
            .receiver
            .iter()
            .chain(
                self.inputs
                    .iter()
                    .chain(&self.options)
                    .map(|arg| &arg.at),
            )
            .map(String::as_str)
            .collect();
        fits(&self.node, node, &mut String::new(), &slots)
    }
}

/// Every Method.
///
/// # Panics
///
/// Panics if one cannot be derived, which the tests rule out.
pub fn methods() -> Vec<Method> {
    rows()
        .into_iter()
        .map(|method| method.expect("the tests derive every method"))
        .collect()
}

/// The Method that `node` is a call of, if it is one of `methods`.
pub fn read<'a>(
    methods: &'a [Method],
    node: &Value,
) -> Option<&'a Method> {
    methods
        .iter()
        .find(|method| method.reads(node))
}

/// Every Method, or why it cannot be derived.
fn rows() -> Vec<Result<Method, String>> {
    [
        expr::methods(),
        function::methods(),
        string::methods(),
        temporal::methods(),
        list::methods(),
        array::methods(),
        structs::methods(),
        binary::methods(),
        categorical::methods(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The kinds of node that are no Methods.
#[cfg(test)]
fn others() -> Vec<&'static str> {
    [
        expr::OTHERS,
        function::OTHERS,
        string::OTHERS,
        structs::OTHERS,
    ]
    .concat()
}

/// Whether `node` equals `template` everywhere but at the JSON pointers
/// `slots`, where it may hold anything. `at` is the pointer to both.
fn fits(
    template: &Value,
    node: &Value,
    at: &mut String,
    slots: &[&str],
) -> bool {
    if slots.contains(&at.as_str()) {
        return true;
    }

    let len = at.len();
    let mut child = |key: &dyn Display, template, node| {
        let _ = write!(at, "/{key}");
        let fits = fits(template, node, at, slots);
        at.truncate(len);
        fits
    };
    match (template, node) {
        (Value::Object(template), Value::Object(node)) => {
            template.len() == node.len()
                && template
                    .iter()
                    .all(|(key, value)| {
                        node.get(key)
                            .is_some_and(|other| child(key, value, other))
                    })
        }
        (Value::Array(template), Value::Array(node)) => {
            template.len() == node.len()
                && template
                    .iter()
                    .zip(node)
                    .enumerate()
                    .all(|(i, (value, other))| child(&i, value, other))
        }
        _ => template == node,
    }
}

/// The Methods whose nodes are the variants of one enum.
struct Namespace {
    name: &'static str,
    /// The JSON pointer, from a node, to that enum.
    at: &'static str,
}

/// How the payload of a node gives a Method its options.
#[derive(Clone, Copy)]
enum Payload {
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
trait Built {
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
trait WithDefault {
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
    fn method<const N: usize, B: Built>(
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
