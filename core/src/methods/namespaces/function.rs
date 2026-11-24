//! The function nodes that are in no namespace.

use polars::prelude::*;
use polars::series::ops::NullBehavior;
use polars_plan::dsl::functions as dsl;
use polars_plan::plans::AExprSorted;

use crate::methods::Method;
use crate::methods::derive::Payload::{Except, Fields, Named};
use crate::methods::derive::{Beside, Namespace, WithDefault};

/// The days that business is done on, from Monday, as Python has them.
pub(super) const WEEK: [bool; 7] = [
    true, true, true, true, true, false, false,
];

/// A window of rows to correlate over, as small as one can be.
const COV: RollingCovOptions = RollingCovOptions {
    window_size: 2,
    min_periods: 2,
    ddof: 1,
};

/// A window over a column of times, of a day as Python's examples have it.
fn window() -> RollingOptionsDynamicWindow {
    RollingOptionsDynamicWindow {
        window_size: Duration::parse("1d"),
        min_periods: 1,
        closed_window: ClosedWindow::Right,
        fn_params: None,
    }
}

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let function = Namespace { name: "", at: "/Function/function" };
    let boolean = Namespace { name: "", at: "/Function/function/Boolean" };
    let pow = Namespace { name: "", at: "/Function/function/Pow" };
    let range = Namespace { name: "", at: "/Function/function/Range" };
    let random = Namespace { name: "", at: "/Function/function/Random/method" };
    let bitwise = Namespace { name: "", at: "/Function/function/Bitwise" };
    let business = Namespace { name: "", at: "/Function/function/Business" };
    let correlation = Namespace { name: "", at: "/Function/function/Correlation/method" };
    let rolling = Namespace { name: "", at: "/Function/function/RollingExpr/function" };
    let rolling_by = Namespace { name: "", at: "/Function/function/RollingExprBy/function_by" };
    let trigonometry = Namespace { name: "", at: "/Function/function/Trigonometry" };

    vec![
        function.method("abs",                     [],                  Fields,                     |s, []| s.abs()),
        function.method("append",                  ["other"],           Fields,                     |s, [other]| s.append(other, true)),
        function.method("approx_n_unique",         [],                  Fields,                     |s, []| s.approx_n_unique()),
        function.method("arctan2",                 ["x"],               Fields,                     |s, [x]| s.arctan2(x)),
        function.method("arg_max",                 [],                  Fields,                     |s, []| s.arg_max()),
        function.method("arg_min",                 [],                  Fields,                     |s, []| s.arg_min()),
        function.method("arg_sort",                [],                  Fields,                     |s, []| s.arg_sort(false, false)),
        function.method("arg_unique",              [],                  Fields,                     |s, []| s.arg_unique()),
        function.method("arg_where",               ["condition"],       Fields,                     |_, [condition]| arg_where(condition)),
        function.method("as_list",                 ["*exprs"],          Fields,                     |_, [exprs]| as_list([exprs])),
        function.method("bottom_k",                ["k"],               Except(&["descending"]),    |s, [k]| s.bottom_k(k))
            .with_default(["k"], lit(5)),
        function.method("bottom_k_by",             ["k", "by"],         Except(&["descending"]),
            |s, [k, by]| s.bottom_k_by(k, [by], vec![false]))
            .with_default(["k"], lit(5)),
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
        function.method("cut",                     [],                  Fields,
            |s, []| s.cut(vec![], None::<Vec<PlSmallStr>>, false, false)),
        function.method("diff",                    ["n"],               Named(&["null_behavior"]),  |s, [n]| s.diff(n, NullBehavior::Ignore))
            .with_default(["n"], lit(1)),
        function.method("drop_nans",               [],                  Fields,                     |s, []| s.drop_nans()),
        function.method("drop_nulls",              [],                  Fields,                     |s, []| s.drop_nulls()),
        function.method("entropy",                 [],                  Fields,                     |s, []| s.entropy(std::f64::consts::E, true)),
        function.method("ewm_mean",                [],                  Fields,                     |s, []| s.ewm_mean(EWMOptions::default())),
        function.method("ewm_mean_by",             ["by"],              Fields,                     |s, [by]| s.ewm_mean_by(by, Duration::parse("1d"))),
        function.method("ewm_std",                 [],                  Fields,                     |s, []| s.ewm_std(EWMOptions::default())),
        function.method("ewm_sum",                 [],                  Fields,                     |s, []| s.ewm_sum(EWMOptions::default())),
        function.method("ewm_sum_by",              ["by"],              Fields,                     |s, [by]| s.ewm_sum_by(by, Duration::parse("1d"))),
        function.method("ewm_var",                 [],                  Fields,                     |s, []| s.ewm_var(EWMOptions::default())),
        function.method("exp",                     [],                  Fields,                     |s, []| s.exp()),
        function.method("extend_constant",         ["value", "n"],      Fields,                     |s, [value, n]| s.extend_constant(value, n)),
        function.method("fill_null",               ["value"],           Fields,                     |s, [value]| s.fill_null(value)),
        function.method("fill_null_with_strategy", [],                  Named(&["strategy"]),
            |s, []| s.fill_null_with_strategy(FillNullStrategy::Forward(None))),
        function.method("floor",                   [],                  Fields,                     |s, []| s.floor()),
        function.method("gather_every",            [],                  Fields,                     |s, []| s.gather_every(1, 0)),
        function.method("hash",                    [],                  Named(&["seed", "seed_1", "seed_2", "seed_3"]), |s, []| s.hash(0, 0, 0, 0)),
        function.method("hist",                    [],                  Fields,                     |s, []| s.hist(None, None, false, false)),
        function.method("index_of",                ["element"],         Fields,                     |s, [element]| s.index_of(element)),
        function.method("interpolate",             [],                  Named(&["method"]),         |s, []| s.interpolate(InterpolationMethod::Linear)),
        function.method("interpolate_by",          ["by"],              Fields,                     |s, [by]| s.interpolate_by(by)),
        function.method("kurtosis",                [],                  Named(&["fisher", "bias"]), |s, []| s.kurtosis(true, true)),
        function.method("log",                     ["base"],            Fields,                     |s, [base]| s.log(base))
            .with_default(["base"], lit(std::f64::consts::E)),
        function.method("log1p",                   [],                  Fields,                     |s, []| s.log1p()),
        function.method("lower_bound",             [],                  Fields,                     |s, []| s.lower_bound()),
        function.method("max_by",                  ["by"],              Fields,                     |s, [by]| s.max_by(by)),
        function.method("max_horizontal",          ["*exprs"],          Fields,
            |_, [exprs]| dsl::max_horizontal([exprs])),
        function.method("mean_horizontal",         ["*exprs"],          Fields,
            |_, [exprs]| dsl::mean_horizontal([exprs], true)),
        function.method("min_by",                  ["by"],              Fields,                     |s, [by]| s.min_by(by)),
        function.method("min_horizontal",          ["*exprs"],          Fields,
            |_, [exprs]| dsl::min_horizontal([exprs])),
        function.method("mode",                    [],                  Fields,                     |s, []| s.mode(false)),
        function.method("neg",                     [],                  Fields,                     |s, []| -s),
        function.method("null_count",              [],                  Fields,                     |s, []| s.null_count()),
        function.method("pct_change",              ["n"],               Fields,                     |s, [n]| s.pct_change(n))
            .with_default(["n"], lit(1)),
        function.method("peak_max",                [],                  Fields,                     |s, []| s.peak_max()),
        function.method("peak_min",                [],                  Fields,                     |s, []| s.peak_min()),
        function.method("product",                 [],                  Fields,                     |s, []| s.product()),
        function.method("qcut",                    [],                  Fields,
            |s, []| s.qcut(vec![], None::<Vec<PlSmallStr>>, false, false, false)),
        function.method("quantile",                ["quantile"],        Fields,
            |s, [quantile]| s.quantile(quantile, QuantileMethod::Nearest)),
        function.method("rank",                    [],                  Fields,                     |s, []| s.rank(RankOptions::default(), None)),
        function.method("rechunk",                 [],                  Fields,                     |s, []| s.rechunk()),
        function.method("reinterpret",             [],                  Named(&["signed", "dtype"]), |s, []| s.reinterpret(Some(true), None)),
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
        function.method("search_sorted",           ["element"],         Fields,
            |s, [element]| s.search_sorted(element, SearchSortedSide::Any, false)),
        function.method("set_sorted",              [],                  Fields,
            |s, []| s.set_sorted_flag(AExprSorted::default())),
        function.method("shift",                   ["n"],               Fields,                     |s, [n]| s.shift(n))
            .with_default(["n"], lit(1)),
        function.method("shift_and_fill",          ["n", "fill_value"], Fields,
            |s, [n, fill_value]| s.shift_and_fill(n, fill_value))
            .with_default(["n"], lit(1)),
        function.method("sign",                    [],                  Fields,                     |s, []| s.sign()),
        function.method("skew",                    [],                  Named(&["bias"]),           |s, []| s.skew(true)),
        function.method("struct",                  ["*exprs"],          Fields,                     |_, [exprs]| as_struct(vec![exprs])),
        function.method("sum_horizontal",          ["*exprs"],          Fields,
            |_, [exprs]| dsl::sum_horizontal([exprs], true)),
        function.method("to_physical",             [],                  Fields,                     |s, []| s.to_physical()),
        function.method("top_k",                   ["k"],               Except(&["descending"]),    |s, [k]| s.top_k(k))
            .with_default(["k"], lit(5)),
        function.method("top_k_by",                ["k", "by"],         Except(&["descending"]),
            |s, [k, by]| s.top_k_by(k, [by], vec![false]))
            .with_default(["k"], lit(5)),
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
        boolean.method("is_close",       ["other"],  Fields, |s, [other]| s.is_close(other, 0.0, 1e-9, false)),
        boolean.method("is_duplicated",  [],         Fields, |s, []| s.is_duplicated()),
        boolean.method("is_empty",       [],         Fields, |s, []| s.is_empty(true)),
        boolean.method("is_finite",      [],         Fields, |s, []| s.is_finite()),
        boolean.method("is_first_distinct", [],      Fields, |s, []| s.is_first_distinct()),
        boolean.method("is_in",          ["other"],  Fields, |s, [other]| s.is_in(other, false)),
        boolean.method("is_infinite",    [],         Fields, |s, []| s.is_infinite()),
        boolean.method("is_last_distinct", [],       Fields, |s, []| s.is_last_distinct()),
        boolean.method("is_nan",         [],         Fields, |s, []| s.is_nan()),
        boolean.method("is_not_nan",     [],         Fields, |s, []| s.is_not_nan()),
        boolean.method("is_not_null",    [],         Fields, |s, []| s.is_not_null()),
        boolean.method("is_null",        [],         Fields, |s, []| s.is_null()),
        boolean.method("is_sorted",      [],         Fields, |s, []| s.is_sorted(None, None)),
        boolean.method("is_unique",      [],         Fields, |s, []| s.is_unique()),
        boolean.method("not_",           [],         Fields, |s, []| s.not()),

        pow.method("cbrt", [],           Fields, |s, []| s.cbrt()),
        pow.method("pow",  ["exponent"], Fields, |s, [exponent]| s.pow(exponent)),
        pow.method("sqrt", [],           Fields, |s, []| s.sqrt()),

        random.method("sample_frac", ["frac"], Except(&["is_fraction"]), |s, [frac]| s.sample_frac(frac, false, None, None))
            .beside(["seed"]),
        random.method("sample_n",    ["n"],    Except(&["is_fraction"]), |s, [n]| s.sample_n(n, false, None, None))
            .beside(["seed"]),
        random.method("shuffle",     [],       Fields,                   |s, []| s.shuffle(None))
            .beside(["seed"]),

        bitwise.method("bitwise_and",            [], Fields, |s, []| s.bitwise_and()),
        bitwise.method("bitwise_count_ones",     [], Fields, |s, []| s.bitwise_count_ones()),
        bitwise.method("bitwise_count_zeros",    [], Fields, |s, []| s.bitwise_count_zeros()),
        bitwise.method("bitwise_leading_ones",   [], Fields, |s, []| s.bitwise_leading_ones()),
        bitwise.method("bitwise_leading_zeros",  [], Fields, |s, []| s.bitwise_leading_zeros()),
        bitwise.method("bitwise_or",             [], Fields, |s, []| s.bitwise_or()),
        bitwise.method("bitwise_trailing_ones",  [], Fields, |s, []| s.bitwise_trailing_ones()),
        bitwise.method("bitwise_trailing_zeros", [], Fields, |s, []| s.bitwise_trailing_zeros()),
        bitwise.method("bitwise_xor",            [], Fields, |s, []| s.bitwise_xor()),

        business.method("business_day_count", ["start", "end", "holidays"], Fields,
            |_, [start, end, holidays]| dsl::business_day_count(start, end, WEEK, holidays)),

        correlation.method("corr",               ["a", "b"], Fields,                     |_, [a, b]| pearson_corr(a, b)),
        correlation.method("cov",                ["a", "b"], Named(&["ddof"]),           |_, [a, b]| cov(a, b, 1)),
        correlation.method("spearman_rank_corr", ["a", "b"], Named(&["propagate_nans"]), |_, [a, b]| spearman_rank_corr(a, b, false)),

        rolling.method("rolling_corr",     ["a", "b"], Except(&["is_corr"]), |_, [a, b]| rolling_corr(a, b, COV))
            .beside(["options"]),
        rolling.method("rolling_cov",      ["a", "b"], Except(&["is_corr"]), |_, [a, b]| rolling_cov(a, b, COV))
            .beside(["options"]),
        rolling.method("rolling_kurtosis", [], Fields, |s, []| s.rolling_kurtosis(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_max",      [], Fields, |s, []| s.rolling_max(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_mean",     [], Fields, |s, []| s.rolling_mean(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_min",      [], Fields, |s, []| s.rolling_min(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_quantile", [], Fields,
            |s, []| s.rolling_quantile(QuantileMethod::Nearest, 0.5, RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_rank",     [], Fields, |s, []| s.rolling_rank(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_skew",     [], Fields, |s, []| s.rolling_skew(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_std",      [], Fields, |s, []| s.rolling_std(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_sum",      [], Fields, |s, []| s.rolling_sum(RollingOptionsFixedWindow::default()))
            .beside(["options"]),
        rolling.method("rolling_var",      [], Fields, |s, []| s.rolling_var(RollingOptionsFixedWindow::default()))
            .beside(["options"]),

        rolling_by.method("rolling_max_by",      ["by"], Fields, |s, [by]| s.rolling_max_by(by, window()))
            .beside(["options"]),
        rolling_by.method("rolling_mean_by",     ["by"], Fields, |s, [by]| s.rolling_mean_by(by, window()))
            .beside(["options"]),
        rolling_by.method("rolling_min_by",      ["by"], Fields, |s, [by]| s.rolling_min_by(by, window()))
            .beside(["options"]),
        rolling_by.method("rolling_quantile_by", ["by"], Fields,
            |s, [by]| s.rolling_quantile_by(by, QuantileMethod::Nearest, 0.5, window()))
            .beside(["options"]),
        rolling_by.method("rolling_rank_by",     ["by"], Fields, |s, [by]| s.rolling_rank_by(by, window()))
            .beside(["options"]),
        rolling_by.method("rolling_std_by",      ["by"], Fields, |s, [by]| s.rolling_std_by(by, window()))
            .beside(["options"]),
        rolling_by.method("rolling_sum_by",      ["by"], Fields, |s, [by]| s.rolling_sum_by(by, window()))
            .beside(["options"]),
        rolling_by.method("rolling_var_by",      ["by"], Fields, |s, [by]| s.rolling_var_by(by, window()))
            .beside(["options"]),

        trigonometry.method("arccos",  [], Fields, |s, []| s.arccos()),
        trigonometry.method("arccosh", [], Fields, |s, []| s.arccosh()),
        trigonometry.method("arcsin",  [], Fields, |s, []| s.arcsin()),
        trigonometry.method("arcsinh", [], Fields, |s, []| s.arcsinh()),
        trigonometry.method("arctan",  [], Fields, |s, []| s.arctan()),
        trigonometry.method("arctanh", [], Fields, |s, []| s.arctanh()),
        trigonometry.method("cos",     [], Fields, |s, []| s.cos()),
        trigonometry.method("cosh",    [], Fields, |s, []| s.cosh()),
        trigonometry.method("cot",     [], Fields, |s, []| s.cot()),
        trigonometry.method("degrees", [], Fields, |s, []| s.degrees()),
        trigonometry.method("radians", [], Fields, |s, []| s.radians()),
        trigonometry.method("sin",     [], Fields, |s, []| s.sin()),
        trigonometry.method("sinh",    [], Fields, |s, []| s.sinh()),
        trigonometry.method("tan",     [], Fields, |s, []| s.tan()),
        trigonometry.method("tanh",    [], Fields, |s, []| s.tanh()),

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
