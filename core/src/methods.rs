//! Expression nodes read as method calls, the way Python Polars writes them.

mod derive;
mod docs;
mod namespaces;
mod render;

use std::fmt::{Display, Write};
use std::sync::LazyLock;

use serde_json::Value;

pub use self::docs::Doc;
use self::namespaces::rows;
pub use self::render::{arguments, literal, literal_of, text, typed};

/// How one kind of expression node reads as a method call.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    /// The `str` of `.str.contains()`.
    pub namespace: &'static str,
    pub name: &'static str,
    /// The enum that tells the node, then its variant: `/Function/function/StringExpr/Contains`.
    pub kind: String,
    /// The pointer to the expression it is called on; `None` for a function.
    pub receiver: Option<String>,
    /// The expressions the method takes.
    pub inputs: Vec<Arg>,
    /// Its other arguments.
    pub options: Vec<Arg>,
    /// The node at its defaults: its receiver null, a variadic input empty.
    node: Value,
    /// What Polars says of it, if Python has it under this name.
    pub doc: Option<&'static Doc>,
}

/// An argument of a [`Method`].
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: String,
    /// The JSON pointer, from the node, to its value.
    pub at: String,
    /// Whether its value is a list of any number of expressions.
    pub is_variadic: bool,
    /// Whether it may be left out: the last of its list, and in it only where it is given.
    pub is_optional: bool,
}

impl Method {
    /// What Polars says the argument of a name is for.
    pub fn says_of(
        &self,
        argument: &str,
    ) -> Option<&'static str> {
        self.doc?
            .args
            .iter()
            .find(|(name, _)| *name == argument)
            .map(|(_, says)| *says)
    }

    /// The node of this method called on `receiver`, its arguments at their defaults.
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

    /// Whether `node` is a call of this method, whatever its receiver and arguments.
    pub fn reads(
        &self,
        node: &Value,
    ) -> bool {
        self.fits(&self.node, node, &mut String::new())
    }

    /// Whether `node` equals `template` but at the receiver and arguments; `at` points to both.
    fn fits(
        &self,
        template: &Value,
        node: &Value,
        at: &mut String,
    ) -> bool {
        if self.is_slot(at) {
            return true;
        }

        // one more than the template holds is an input that may be left out, given
        let is_given = match (template, node) {
            (Value::Array(template), Value::Array(node)) => {
                node.len() == template.len() + 1
                    && self.is_slot(&format!("{at}/{}", template.len()))
            }
            _ => false,
        };
        let len = at.len();
        let mut child = |key: &dyn Display, template, node| {
            let _ = write!(at, "/{key}");
            let fits = self.fits(template, node, at);
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
                (template.len() == node.len() || is_given)
                    && template
                        .iter()
                        .zip(node)
                        .enumerate()
                        .all(|(i, (value, other))| child(&i, value, other))
            }
            _ => template == node,
        }
    }

    /// Whether the JSON pointer `at` leads to the receiver or an argument.
    fn is_slot(
        &self,
        at: &str,
    ) -> bool {
        self.receiver
            .as_deref()
            == Some(at)
            || self
                .inputs
                .iter()
                .chain(&self.options)
                .any(|arg| arg.at == at)
    }
}

/// Every Method, derived once; panics if a row cannot be, which the tests rule out.
pub fn methods() -> &'static [Method] {
    static METHODS: LazyLock<Vec<Method>> = LazyLock::new(|| {
        let (methods, problems): (Vec<_>, Vec<_>) = rows()
            .into_iter()
            .partition(Result::is_ok);
        let problems: Vec<String> = problems
            .into_iter()
            .filter_map(Result::err)
            .collect();
        assert!(problems.is_empty(), "{}", problems.join("\n"));
        methods
            .into_iter()
            .flatten()
            .map(|method| Method {
                doc: docs::DOCS
                    .iter()
                    .find(|doc| {
                        doc.namespace == method.namespace
                            && doc.name == method.name
                            && doc.is_function
                                == method
                                    .receiver
                                    .is_none()
                    }),
                ..method
            })
            .collect()
    });
    &METHODS
}

/// The Method that `node` is a call of, if it is one.
pub fn read(node: &Value) -> Option<&'static Method> {
    methods()
        .iter()
        .find(|method| method.reads(node))
}
