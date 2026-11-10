//! The methods of `.list`.

use polars::prelude::*;

use super::Payload::{Fields, Named};
use super::{Method, Namespace, WithDefault};

/// One row for each Method: its name, its inputs, how its payload gives its
/// options, and the call that builds its node. A function ignores the
/// receiver.
///
/// A call passes Python's default for each option that has one.
#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let list = Namespace { name: "list", at: "/Function/function/ListExpr" };
    let concat = Namespace { name: "", at: "/Function/function/ListExpr" };

    vec![
        list.method("arg_max",  [],                   Fields,                   |s, []| s.list().arg_max()),
        list.method("arg_min",  [],                   Fields,                   |s, []| s.list().arg_min()),
        list.method("contains", ["item"],             Fields,                   |s, [item]| s.list().contains(item, true)),
        list.method("get",      ["index"],            Named(&["null_on_oob"]),  |s, [index]| s.list().get(index, false)),
        list.method("join",     ["separator"],        Named(&["ignore_nulls"]), |s, [separator]| s.list().join(separator, true)),
        list.method("len",      [],                   Fields,                   |s, []| s.list().len()),
        list.method("max",      [],                   Fields,                   |s, []| s.list().max()),
        list.method("mean",     [],                   Fields,                   |s, []| s.list().mean()),
        list.method("median",   [],                   Fields,                   |s, []| s.list().median()),
        list.method("min",      [],                   Fields,                   |s, []| s.list().min()),
        list.method("shift",    ["n"],                Fields,                   |s, [n]| s.list().shift(n))
            .with_default(["n"], lit(1)),
        list.method("slice",    ["offset", "length"], Fields,                   |s, [offset, length]| s.list().slice(offset, length))
            .with_default(["length"], lit(NULL)),
        list.method("sort",     [],                   Fields,                   |s, []| s.list().sort(SortOptions::default())),
        list.method("std",      [],                   Named(&["ddof"]),         |s, []| s.list().std(1)),
        list.method("sum",      [],                   Fields,                   |s, []| s.list().sum()),
        list.method("to_array", [],                   Named(&["width"]),        |s, []| s.list().to_array(1)),
        list.method("var",      [],                   Named(&["ddof"]),         |s, []| s.list().var(1)),

        concat.method("concat_list", ["*exprs"], Fields, |_, [exprs]| concat_list([exprs])),
    ]
}
