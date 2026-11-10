//! The methods of `.cat`.

use polars::prelude::*;

use super::Payload::{Fields, Named};
use super::{Method, Namespace};

/// One row for each Method: its name, its inputs, how its payload gives its
/// options, and the call that builds its node. A function ignores the
/// receiver.
///
/// A call passes Python's default for each option that has one.
#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let cat = Namespace { name: "cat", at: "/Function/function/Categorical" };

    vec![
        cat.method("ends_with",      [], Named(&["suffix"]),           |s, []| s.cat().ends_with(String::new())),
        cat.method("get_categories", [], Fields,                       |s, []| s.cat().get_categories()),
        cat.method("len_bytes",      [], Fields,                       |s, []| s.cat().len_bytes()),
        cat.method("len_chars",      [], Fields,                       |s, []| s.cat().len_chars()),
        cat.method("physical",       [], Fields,                       |s, []| s.cat().physical()),
        cat.method("slice",          [], Named(&["offset", "length"]), |s, []| s.cat().slice(0, None)),
        cat.method("starts_with",    [], Named(&["prefix"]),           |s, []| s.cat().starts_with(String::new())),
        cat.method("to",             [], Named(&["dtype", "strict"]),  |s, []| s.cat().to(DataType::String, true)),
    ]
}
