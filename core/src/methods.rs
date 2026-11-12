//! Expression nodes read as method calls, the way Python Polars writes them.
//!
//! An expression is stored in Polars' JSON format. A [`Method`] says how one
//! kind of node in it reads as a call such as `.str.contains(pattern)`: what
//! the node looks like, and where its receiver and arguments sit.
//!
//! Each Method is derived from a call of the Rust method that builds its node,
//! so it cannot disagree with Polars. The tests check the Methods against the
//! JSON Schema of the expressions, which lists every node there is.
//!
//! The Methods are listed in [`namespaces`], a table for each namespace.
//! [`derive`] makes a Method of a row there, and [`render`] writes the calls
//! out as text.

mod derive;
mod namespaces;
mod render;

use std::fmt::{Display, Write};

use serde_json::Value;

use self::namespaces::rows;
pub use self::render::{arguments, text};

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
        .map(|method| method.unwrap_or_else(|problem| panic!("{problem}")))
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
