//! The nodes of an expression itself: roots, methods, aggregations, operators and renames.

use std::sync::Arc;

use polars::prelude::*;

use crate::methods::Method;
use crate::methods::derive::Payload::{Except, Fields, Named};
use crate::methods::derive::{Namespace, WithDefault};

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let expr = Namespace { name: "", at: "" };
    let name = Namespace { name: "name", at: "" };
    let structs = Namespace { name: "struct", at: "" };
    let agg = Namespace { name: "", at: "/Agg" };
    let op = Namespace { name: "", at: "/BinaryExpr/op" };
    let rename = Namespace { name: "name", at: "/RenameAlias/function" };
    let eval = Namespace { name: "", at: "/Eval/variant" };
    let list_eval = Namespace { name: "list", at: "/Eval/variant" };
    let arr_eval = Namespace { name: "arr", at: "/Eval/variant" };
    let cs = Namespace { name: "cs", at: "/Selector" };

    vec![
        expr.method("alias",   [],                   Named(&["name"]),            |s, []| s.alias("")),
        expr.method("cast",    [],                   Fields,                      |s, []| s.strict_cast(DataType::String)),
        expr.method("col",     [],                   Named(&["name"]),            |_, []| col("")),
        expr.method("element", [],                   Fields,                      |_, []| element()),
        expr.method("explode", [],                   Fields,
            |s, []| s.explode(ExplodeOptions { empty_as_null: true, keep_nulls: true })),
        expr.method("field",   [],                   Named(&["names"]),           |_, []| Expr::Field(Arc::new([]))),
        expr.method("filter",  ["predicate"],        Fields,                      |s, [predicate]| s.filter(predicate)),
        expr.method("gather",  ["indices"],          Except(&["returns_scalar"]), |s, [indices]| s.gather(indices, false)),
        expr.method("get",     ["index"],            Except(&["returns_scalar"]), |s, [index]| s.get(index, false)),
        expr.method("len",     [],                   Fields,                      |_, []| len()),
        expr.method("lit",     [],                   Named(&["value"]),           |_, []| lit(0)),
        expr.method("over",    ["*partition_by"],    Fields,                      |s, [partition_by]| s.over([partition_by])),
        expr.method("slice",   ["offset", "length"], Fields,                      |s, [offset, length]| s.slice(offset, length))
            .with_default(["length"], lit(NULL)),
        expr.method("sort",    [],                   Fields,                      |s, []| s.sort(SortOptions::default())),
        expr.method("sort_by", ["*by"],              Fields,                      |s, [by]| s.sort_by([by], SortMultipleOptions::default())),
        expr.method("when",    ["predicate", "then", "otherwise"], Fields,
            |_, [predicate, then, otherwise]| when(predicate).then(then).otherwise(otherwise))
            .with_default(["otherwise"], lit(NULL)),

        name.method("keep", [], Fields, |s, []| s.name().keep()),

        structs.method("with_fields", ["*exprs"], Fields, |s, [exprs]| s.struct_().with_fields(vec![exprs])),

        agg.method("agg_groups",     [], Fields,                     |s, []| s.agg_groups()),
        agg.method("count",          [], Except(&["include_nulls"]), |s, []| s.count()),
        agg.method("first",          [], Fields,                     |s, []| s.first()),
        agg.method("first_non_null", [], Fields,                     |s, []| s.first_non_null()),
        agg.method("implode",        [], Fields,                     |s, []| s.implode(true)),
        agg.method("item",           [], Fields,                     |s, []| s.item(false)),
        agg.method("last",           [], Fields,                     |s, []| s.last()),
        agg.method("last_non_null",  [], Fields,                     |s, []| s.last_non_null()),
        agg.method("len",            [], Except(&["include_nulls"]), |s, []| s.len()),
        agg.method("max",            [], Fields,                     |s, []| s.max()),
        agg.method("mean",           [], Fields,                     |s, []| s.mean()),
        agg.method("median",         [], Fields,                     |s, []| s.median()),
        agg.method("min",            [], Fields,                     |s, []| s.min()),
        agg.method("n_unique",       [], Fields,                     |s, []| s.n_unique()),
        agg.method("std",            [], Named(&["ddof"]),           |s, []| s.std(1)),
        agg.method("sum",            [], Fields,                     |s, []| s.sum()),
        agg.method("var",            [], Named(&["ddof"]),           |s, []| s.var(1)),

        op.method("add",         ["other"], Fields, |s, [other]| s + other),
        op.method("and_",        ["other"], Fields, |s, [other]| s.and(other)),
        op.method("eq",          ["other"], Fields, |s, [other]| s.eq(other)),
        op.method("eq_missing",  ["other"], Fields, |s, [other]| s.eq_missing(other)),
        op.method("floordiv",    ["other"], Fields, |s, [other]| s.floor_div(other)),
        op.method("ge",          ["other"], Fields, |s, [other]| s.gt_eq(other)),
        op.method("gt",          ["other"], Fields, |s, [other]| s.gt(other)),
        op.method("le",          ["other"], Fields, |s, [other]| s.lt_eq(other)),
        op.method("logical_and", ["other"], Fields, |s, [other]| s.logical_and(other)),
        op.method("logical_or",  ["other"], Fields, |s, [other]| s.logical_or(other)),
        op.method("lt",          ["other"], Fields, |s, [other]| s.lt(other)),
        op.method("mod",         ["other"], Fields, |s, [other]| s % other),
        op.method("mul",         ["other"], Fields, |s, [other]| s * other),
        op.method("ne",          ["other"], Fields, |s, [other]| s.neq(other)),
        op.method("ne_missing",  ["other"], Fields, |s, [other]| s.neq_missing(other)),
        op.method("or_",         ["other"], Fields, |s, [other]| s.or(other)),
        op.method("sub",         ["other"], Fields, |s, [other]| s - other),
        op.method("truediv",     ["other"], Fields, |s, [other]| s.true_div(other)),
        op.method("xor",         ["other"], Fields, |s, [other]| s.xor(other)),

        rename.method("prefix",       [], Named(&["prefix"]), |s, []| s.name().prefix("")),
        rename.method("replace",      [], Fields,             |s, []| s.name().replace("", "", false)),
        rename.method("suffix",       [], Named(&["suffix"]), |s, []| s.name().suffix("")),
        rename.method("to_lowercase", [], Fields,             |s, []| s.name().to_lowercase()),
        rename.method("to_uppercase", [], Fields,             |s, []| s.name().to_uppercase()),

        eval.method("cumulative_eval", ["expr"], Fields, |s, [expr]| s.cumulative_eval(expr, 1)),

        list_eval.method("agg",  ["expr"], Fields, |s, [expr]| s.list().agg(expr)),
        list_eval.method("eval", ["expr"], Fields, |s, [expr]| s.list().eval(expr)),

        arr_eval.method("agg",  ["expr"], Fields, |s, [expr]| s.arr().agg(expr)),
        arr_eval.method("eval", ["expr"], Fields, |s, [expr]| s.arr().eval(expr, false)),

        cs.method("all",      [], Fields,              |_, []| Expr::Selector(Selector::Wildcard)),
        cs.method("by_dtype", [], Named(&["dtypes"]),  |_, []| Expr::Selector(Selector::ByDType(DataTypeSelector::Empty))),
        cs.method("by_index", [], Fields,              |_, []| Expr::Selector(Selector::ByIndex { indices: [].into(), strict: true })),
        cs.method("by_name",  [], Fields,              |_, []| Expr::Selector(Selector::ByName { names: [].into(), strict: true })),
        cs.method("empty",    [], Fields,              |_, []| Expr::Selector(Selector::Empty)),
        cs.method("matches",  [], Named(&["pattern"]), |_, []| Expr::Selector(Selector::Matches("".into()))),
    ]
}
