//! The methods of `.bin`.

use super::Payload::{Fields, Named};
use super::{Method, Namespace};

/// One row for each Method: its name, its inputs, how its payload gives its
/// options, and the call that builds its node. A function ignores the
/// receiver.
///
/// A call passes Python's default for each option that has one.
#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let bin = Namespace { name: "bin", at: "/Function/function/BinaryExpr" };

    vec![
        bin.method("contains",    ["literal"],          Fields,                  |s, [literal]| s.binary().contains_literal(literal)),
        bin.method("ends_with",   ["suffix"],           Fields,                  |s, [suffix]| s.binary().ends_with(suffix)),
        bin.method("get",         ["index"],            Named(&["null_on_oob"]), |s, [index]| s.binary().get(index, false)),
        bin.method("head",        ["n"],                Fields,                  |s, [n]| s.binary().head(n)),
        bin.method("size",        [],                   Fields,                  |s, []| s.binary().size_bytes()),
        bin.method("slice",       ["offset", "length"], Fields,                  |s, [offset, length]| s.binary().slice(offset, length)),
        bin.method("starts_with", ["prefix"],           Fields,                  |s, [prefix]| s.binary().starts_with(prefix)),
        bin.method("tail",        ["n"],                Fields,                  |s, [n]| s.binary().tail(n)),
    ]
}
