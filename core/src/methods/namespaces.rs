//! The tables. A row calls the Rust method with Python's defaults; `*` marks a variadic input.

mod array;
mod binary;
mod categorical;
mod expr;
mod function;
mod list;
mod string;
mod structs;
mod temporal;

use super::Method;

/// Every Method, or why it cannot be derived.
pub(super) fn rows() -> Vec<Result<Method, String>> {
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
