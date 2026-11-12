//! The methods of `.arr`.

use polars::prelude::*;

use crate::methods::Method;
use crate::methods::derive::Payload::{Fields, Named};
use crate::methods::derive::{Namespace, WithDefault};

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let arr = Namespace { name: "arr", at: "/Function/function/ArrayExpr" };
    let concat = Namespace { name: "", at: "/Function/function/ArrayExpr" };

    vec![
        arr.method("arg_max",  [],            Fields,                       |s, []| s.arr().arg_max()),
        arr.method("arg_min",  [],            Fields,                       |s, []| s.arr().arg_min()),
        arr.method("contains", ["item"],      Fields,                       |s, [item]| s.arr().contains(item, true)),
        arr.method("explode",  [],            Fields,
            |s, []| s.arr().explode(ExplodeOptions { empty_as_null: true, keep_nulls: true })),
        arr.method("get",      ["index"],     Named(&["null_on_oob"]),      |s, [index]| s.arr().get(index, false)),
        arr.method("join",     ["separator"], Named(&["ignore_nulls"]),     |s, [separator]| s.arr().join(separator, true)),
        arr.method("len",      [],            Fields,                       |s, []| s.arr().len()),
        arr.method("max",      [],            Fields,                       |s, []| s.arr().max()),
        arr.method("mean",     [],            Fields,                       |s, []| s.arr().mean()),
        arr.method("median",   [],            Fields,                       |s, []| s.arr().median()),
        arr.method("min",      [],            Fields,                       |s, []| s.arr().min()),
        arr.method("shift",    ["n"],         Fields,                       |s, [n]| s.arr().shift(n))
            .with_default(["n"], lit(1)),
        arr.method("slice",    [],            Named(&["offset", "length"]), |s, []| s.arr().slice(lit(0), lit(1), true)),
        arr.method("sort",     [],            Fields,                       |s, []| s.arr().sort(SortOptions::default())),
        arr.method("std",      [],            Named(&["ddof"]),             |s, []| s.arr().std(1)),
        arr.method("sum",      [],            Fields,                       |s, []| s.arr().sum()),
        arr.method("to_list",  [],            Fields,                       |s, []| s.arr().to_list()),
        arr.method("var",      [],            Named(&["ddof"]),             |s, []| s.arr().var(1)),

        concat.method("concat_arr", ["*exprs"], Fields, |_, [exprs]| concat_arr(vec![exprs])),
    ]
}
