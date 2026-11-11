//! The function nodes that are in no namespace.

use polars::prelude::*;
use polars_plan::dsl::functions as dsl;
use polars_plan::plans::AExprSorted;

use crate::methods::Method;
use crate::methods::derive::Payload::{Except, Fields, Named};
use crate::methods::derive::{Namespace, WithDefault};

/// The kinds of node here that are no Methods. The tests want every kind to
/// be read by a Method or listed.
#[cfg(test)]
pub(super) const OTHERS: &[&str] = &[
    // take a Rust closure
    "/Function/function/FoldHorizontal",
    "/Function/function/ReduceHorizontal",
    "/Function/function/CumReduceHorizontal",
    "/Function/function/CumFoldHorizontal",
    // internal to Polars
    "/Function/function/RowEncode",
    "/Function/function/RowDecode",
];

/// One row for each Method: its name, its inputs, how its payload gives its
/// options, and the call that builds its node. A function ignores the
/// receiver.
///
/// A call passes Python's default for each option that has one.
#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let function = Namespace { name: "", at: "/Function/function" };
    let boolean = Namespace { name: "", at: "/Function/function/Boolean" };
    let pow = Namespace { name: "", at: "/Function/function/Pow" };
    let range = Namespace { name: "", at: "/Function/function/Range" };

    vec![
        function.method("abs",                     [],                  Fields,                     |s, []| s.abs()),
        function.method("append",                  ["other"],           Fields,                     |s, [other]| s.append(other, true)),
        function.method("arg_max",                 [],                  Fields,                     |s, []| s.arg_max()),
        function.method("arg_min",                 [],                  Fields,                     |s, []| s.arg_min()),
        function.method("arg_sort",                [],                  Fields,                     |s, []| s.arg_sort(false, false)),
        function.method("arg_unique",              [],                  Fields,                     |s, []| s.arg_unique()),
        function.method("arg_where",               ["condition"],       Fields,                     |_, [condition]| arg_where(condition)),
        function.method("as_list",                 ["*exprs"],          Fields,                     |_, [exprs]| as_list([exprs])),
        function.method("ceil",                    [],                  Fields,                     |s, []| s.ceil()),
        function.method("clip",                    ["lower_bound", "upper_bound"], Except(&["has_min", "has_max"]),
            |s, [lower_bound, upper_bound]| s.clip(lower_bound, upper_bound)),
        function.method("clip_max",                ["upper_bound"],     Except(&["has_min", "has_max"]), |s, [upper_bound]| s.clip_max(upper_bound)),
        function.method("clip_min",                ["lower_bound"],     Except(&["has_min", "has_max"]), |s, [lower_bound]| s.clip_min(lower_bound)),
        function.method("coalesce",                ["*exprs"],          Fields,                     |_, [exprs]| coalesce(&[exprs])),
        function.method("concat_expr",             ["*exprs"],          Named(&["rechunk"]),        |_, [exprs]| concat_expr([exprs], true)),
        function.method("cum_count",               [],                  Fields,                     |s, []| s.cum_count(false)),
        function.method("cum_max",                 [],                  Fields,                     |s, []| s.cum_max(false)),
        function.method("cum_min",                 [],                  Fields,                     |s, []| s.cum_min(false)),
        function.method("cum_prod",                [],                  Fields,                     |s, []| s.cum_prod(false)),
        function.method("cum_sum",                 [],                  Fields,                     |s, []| s.cum_sum(false)),
        function.method("drop_nans",               [],                  Fields,                     |s, []| s.drop_nans()),
        function.method("drop_nulls",              [],                  Fields,                     |s, []| s.drop_nulls()),
        function.method("extend_constant",         ["value", "n"],      Fields,                     |s, [value, n]| s.extend_constant(value, n)),
        function.method("fill_null",               ["value"],           Fields,                     |s, [value]| s.fill_null(value)),
        function.method("fill_null_with_strategy", [],                  Named(&["strategy"]),
            |s, []| s.fill_null_with_strategy(FillNullStrategy::Forward(None))),
        function.method("floor",                   [],                  Fields,                     |s, []| s.floor()),
        function.method("gather_every",            [],                  Fields,                     |s, []| s.gather_every(1, 0)),
        function.method("lower_bound",             [],                  Fields,                     |s, []| s.lower_bound()),
        function.method("max_by",                  ["by"],              Fields,                     |s, [by]| s.max_by(by)),
        function.method("max_horizontal",          ["*exprs"],          Fields,
            |_, [exprs]| dsl::max_horizontal([exprs])),
        function.method("mean_horizontal",         ["*exprs"],          Fields,
            |_, [exprs]| dsl::mean_horizontal([exprs], true)),
        function.method("min_by",                  ["by"],              Fields,                     |s, [by]| s.min_by(by)),
        function.method("min_horizontal",          ["*exprs"],          Fields,
            |_, [exprs]| dsl::min_horizontal([exprs])),
        function.method("neg",                     [],                  Fields,                     |s, []| -s),
        function.method("null_count",              [],                  Fields,                     |s, []| s.null_count()),
        function.method("peak_max",                [],                  Fields,                     |s, []| s.peak_max()),
        function.method("peak_min",                [],                  Fields,                     |s, []| s.peak_min()),
        function.method("product",                 [],                  Fields,                     |s, []| s.product()),
        function.method("quantile",                ["quantile"],        Fields,
            |s, [quantile]| s.quantile(quantile, QuantileMethod::Nearest)),
        function.method("rechunk",                 [],                  Fields,                     |s, []| s.rechunk()),
        // built here, as the function `repeat` gives its node an alias
        function.method("repeat",                  ["value", "n"],      Fields,
            |_, [value, n]| Expr::Function { input: vec![value, n], function: FunctionExpr::Repeat }),
        function.method("repeat_by",               ["by"],              Fields,                     |s, [by]| s.repeat_by(by)),
        function.method("replace",                 ["old", "new"],      Fields,                     |s, [old, new]| s.replace(old, new)),
        function.method("replace_strict",          ["old", "new"],      Fields,
            |s, [old, new]| s.replace_strict(old, new, None, None::<DataType>)),
        function.method("reshape",                 [],                  Named(&["dimensions"]),     |s, []| s.reshape(&[-1])),
        function.method("reverse",                 [],                  Fields,                     |s, []| s.reverse()),
        function.method("rle",                     [],                  Fields,                     |s, []| s.rle()),
        function.method("rle_id",                  [],                  Fields,                     |s, []| s.rle_id()),
        function.method("round",                   [],                  Fields,                     |s, []| s.round(0, RoundMode::HalfToEven)),
        function.method("round_sig_figs",          [],                  Fields,                     |s, []| s.round_sig_figs(1)),
        function.method("set_sorted",              [],                  Fields,
            |s, []| s.set_sorted_flag(AExprSorted::default())),
        function.method("shift",                   ["n"],               Fields,                     |s, [n]| s.shift(n))
            .with_default(["n"], lit(1)),
        function.method("shift_and_fill",          ["n", "fill_value"], Fields,
            |s, [n, fill_value]| s.shift_and_fill(n, fill_value))
            .with_default(["n"], lit(1)),
        function.method("struct",                  ["*exprs"],          Fields,                     |_, [exprs]| as_struct(vec![exprs])),
        function.method("sum_horizontal",          ["*exprs"],          Fields,
            |_, [exprs]| dsl::sum_horizontal([exprs], true)),
        function.method("to_physical",             [],                  Fields,                     |s, []| s.to_physical()),
        function.method("truncate",                [],                  Fields,                     |s, []| s.truncate(0)),
        function.method("unique",                  [],                  Named(&["maintain_order"]), |s, []| s.unique()),
        function.method("unique_counts",           [],                  Fields,                     |s, []| s.unique_counts()),
        function.method("upper_bound",             [],                  Fields,                     |s, []| s.upper_bound()),
        function.method("value_counts",            [],                  Fields,
            |s, []| s.value_counts(false, false, "count", false)),

        boolean.method("all",            [],         Fields, |s, []| s.all(true)),
        boolean.method("all_horizontal", ["*exprs"], Fields, |_, [exprs]| dsl::all_horizontal([exprs])),
        boolean.method("any",            [],         Fields, |s, []| s.any(true)),
        boolean.method("any_horizontal", ["*exprs"], Fields, |_, [exprs]| dsl::any_horizontal([exprs])),
        boolean.method("has_nulls",      [],         Fields, |s, []| s.has_nulls()),
        boolean.method("is_between",     ["lower_bound", "upper_bound"], Fields,
            |s, [lower_bound, upper_bound]| s.is_between(lower_bound, upper_bound, ClosedInterval::Both)),
        boolean.method("is_empty",       [],         Fields, |s, []| s.is_empty(true)),
        boolean.method("is_finite",      [],         Fields, |s, []| s.is_finite()),
        boolean.method("is_in",          ["other"],  Fields, |s, [other]| s.is_in(other, false)),
        boolean.method("is_infinite",    [],         Fields, |s, []| s.is_infinite()),
        boolean.method("is_nan",         [],         Fields, |s, []| s.is_nan()),
        boolean.method("is_not_nan",     [],         Fields, |s, []| s.is_not_nan()),
        boolean.method("is_not_null",    [],         Fields, |s, []| s.is_not_null()),
        boolean.method("is_null",        [],         Fields, |s, []| s.is_null()),
        boolean.method("is_sorted",      [],         Fields, |s, []| s.is_sorted(None, None)),
        boolean.method("not_",           [],         Fields, |s, []| s.not()),

        pow.method("cbrt", [],           Fields, |s, []| s.cbrt()),
        pow.method("pow",  ["exponent"], Fields, |s, [exponent]| s.pow(exponent)),
        pow.method("sqrt", [],           Fields, |s, []| s.sqrt()),

        range.method("date_range",      ["start", "end"], Except(&["arg_type"]),
            |_, [start, end]| dsl::date_range(Some(start), Some(end), Some(Duration::parse("1d")), None, ClosedWindow::Both)),
        range.method("date_ranges",     ["start", "end"], Except(&["arg_type"]),
            |_, [start, end]| dsl::date_ranges(Some(start), Some(end), Some(Duration::parse("1d")), None, ClosedWindow::Both)),
        range.method("datetime_range",  ["start", "end"], Except(&["arg_type"]),
            |_, [start, end]| dsl::datetime_range(Some(start), Some(end), Some(Duration::parse("1d")), None, ClosedWindow::Both, None, None)),
        range.method("datetime_ranges", ["start", "end"], Except(&["arg_type"]),
            |_, [start, end]| dsl::datetime_ranges(Some(start), Some(end), Some(Duration::parse("1d")), None, ClosedWindow::Both, None, None)),
        range.method("int_range",       ["start", "end"], Fields,                   |_, [start, end]| int_range(start, end, 1, DataType::Int64))
            .with_default(["start"], lit(0)),
        range.method("int_ranges",      ["start", "end", "step"], Fields,
            |_, [start, end, step]| int_ranges(start, end, step, DataType::Int64))
            .with_default(["start"], lit(0))
            .with_default(["step"], lit(1)),
        range.method("linear_space",    ["start", "end", "num_samples"], Fields,
            |_, [start, end, num_samples]| linear_space(start, end, num_samples, ClosedInterval::Both)),
        range.method("linear_spaces",   ["start", "end", "num_samples"], Except(&["array_width"]),
            |_, [start, end, num_samples]| linear_spaces(start, end, num_samples, ClosedInterval::Both, false)),
        range.method("time_range",      ["start", "end"], Fields,
            |_, [start, end]| dsl::time_range(start, end, Duration::parse("1h"), ClosedWindow::Both)),
        range.method("time_ranges",     ["start", "end"], Fields,
            |_, [start, end]| dsl::time_ranges(start, end, Duration::parse("1h"), ClosedWindow::Both)),
    ]
}
