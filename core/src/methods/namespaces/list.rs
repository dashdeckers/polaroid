//! The methods of `.list`.

use polars::prelude::*;
use polars::series::ops::NullBehavior;

use crate::methods::Method;
use crate::methods::derive::Payload::{Except, Fields, Named};
use crate::methods::derive::{Namespace, WithDefault};

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let list = Namespace { name: "list", at: "/Function/function/ListExpr" };
    let concat = Namespace { name: "", at: "/Function/function/ListExpr" };
    let sets = Namespace { name: "list", at: "/Function/function/ListExpr/SetOperation" };

    vec![
        list.method("arg_max",  [],                   Fields,                   |s, []| s.list().arg_max()),
        list.method("arg_min",  [],                   Fields,                   |s, []| s.list().arg_min()),
        list.method("contains", ["item"],             Fields,                   |s, [item]| s.list().contains(item, true)),
        list.method("count_matches", ["element"],     Fields,                   |s, [element]| s.list().count_matches(element)),
        list.method("diff",     [],                   Fields,                   |s, []| s.list().diff(1, NullBehavior::Ignore)),
        list.method("drop_nulls", [],                 Fields,                   |s, []| s.list().drop_nulls()),
        list.method("gather",   ["indices"],          Named(&["null_on_oob"]),  |s, [indices]| s.list().gather(indices, false)),
        list.method("gather_every", ["n", "offset"],  Fields,                   |s, [n, offset]| s.list().gather_every(n, offset))
            .with_default(["offset"], lit(0)),
        list.method("get",      ["index"],            Named(&["null_on_oob"]),  |s, [index]| s.list().get(index, false)),
        list.method("join",     ["separator"],        Named(&["ignore_nulls"]), |s, [separator]| s.list().join(separator, true)),
        list.method("len",      [],                   Fields,                   |s, []| s.list().len()),
        list.method("max",      [],                   Fields,                   |s, []| s.list().max()),
        list.method("mean",     [],                   Fields,                   |s, []| s.list().mean()),
        list.method("median",   [],                   Fields,                   |s, []| s.list().median()),
        list.method("min",      [],                   Fields,                   |s, []| s.list().min()),
        list.method("sample_fraction", ["fraction"],  Except(&["is_fraction"]),
            |s, [fraction]| s.list().sample_fraction(fraction, false, None, None)),
        list.method("sample_n", ["n"],                Except(&["is_fraction"]), |s, [n]| s.list().sample_n(n, false, None, None)),
        list.method("shift",    ["n"],                Fields,                   |s, [n]| s.list().shift(n))
            .with_default(["n"], lit(1)),
        list.method("slice",    ["offset", "length"], Fields,                   |s, [offset, length]| s.list().slice(offset, length))
            .with_default(["length"], lit(NULL)),
        list.method("sort",     [],                   Fields,                   |s, []| s.list().sort(SortOptions::default())),
        list.method("std",      [],                   Named(&["ddof"]),         |s, []| s.list().std(1)),
        list.method("sum",      [],                   Fields,                   |s, []| s.list().sum()),
        list.method("to_array", [],                   Named(&["width"]),        |s, []| s.list().to_array(1)),
        list.method("var",      [],                   Named(&["ddof"]),         |s, []| s.list().var(1)),

        list.method("to_struct", [],                  Named(&["fields"]),       |s, []| s.list().to_struct([].into())),

        sets.method("set_difference",           ["other"], Fields, |s, [other]| s.list().set_difference(other)),
        sets.method("set_intersection",         ["other"], Fields, |s, [other]| s.list().set_intersection(other)),
        sets.method("set_symmetric_difference", ["other"], Fields, |s, [other]| s.list().set_symmetric_difference(other)),
        sets.method("set_union",                ["other"], Fields, |s, [other]| s.list().union(other)),

        concat.method("concat_list", ["*exprs"], Fields, |_, [exprs]| concat_list([exprs])),
    ]
}
