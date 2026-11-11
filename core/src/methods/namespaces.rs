//! The tables of Methods: one for each namespace of Python Polars, and one
//! for the functions that are in none.

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

/// The kinds of node that are no Methods.
#[cfg(test)]
pub(super) fn others() -> Vec<&'static str> {
    [
        expr::OTHERS,
        function::OTHERS,
        string::OTHERS,
        structs::OTHERS,
    ]
    .concat()
}
