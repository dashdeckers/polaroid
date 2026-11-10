//! The methods of `.dt`, and the functions that build dates and durations.

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
    let dt = Namespace { name: "dt", at: "/Function/function/TemporalExpr" };
    let temporal = Namespace { name: "", at: "/Function/function/TemporalExpr" };

    vec![
        dt.method("base_utc_offset",    [],            Fields,                |s, []| s.dt().base_utc_offset()),
        dt.method("cast_time_unit",     [],            Named(&["time_unit"]), |s, []| s.dt().cast_time_unit(TimeUnit::Microseconds)),
        dt.method("century",            [],            Fields,                |s, []| s.dt().century()),
        dt.method("combine",            ["time"],      Named(&["time_unit"]), |s, [time]| s.dt().combine(time, TimeUnit::Microseconds)),
        dt.method("convert_time_zone",  [],            Named(&["time_zone"]), |s, []| s.dt().convert_time_zone(TimeZone::UTC)),
        dt.method("date",               [],            Fields,                |s, []| s.dt().date()),
        dt.method("datetime",           [],            Fields,                |s, []| s.dt().datetime()),
        dt.method("day",                [],            Fields,                |s, []| s.dt().day()),
        dt.method("days_in_month",      [],            Fields,                |s, []| s.dt().days_in_month()),
        dt.method("dst_offset",         [],            Fields,                |s, []| s.dt().dst_offset()),
        dt.method("hour",               [],            Fields,                |s, []| s.dt().hour()),
        dt.method("is_leap_year",       [],            Fields,                |s, []| s.dt().is_leap_year()),
        dt.method("iso_year",           [],            Fields,                |s, []| s.dt().iso_year()),
        dt.method("microsecond",        [],            Fields,                |s, []| s.dt().microsecond()),
        dt.method("millennium",         [],            Fields,                |s, []| s.dt().millennium()),
        dt.method("millisecond",        [],            Fields,                |s, []| s.dt().millisecond()),
        dt.method("minute",             [],            Fields,                |s, []| s.dt().minute()),
        dt.method("month",              [],            Fields,                |s, []| s.dt().month()),
        dt.method("nanosecond",         [],            Fields,                |s, []| s.dt().nanosecond()),
        dt.method("ordinal_day",        [],            Fields,                |s, []| s.dt().ordinal_day()),
        dt.method("quarter",            [],            Fields,                |s, []| s.dt().quarter()),
        dt.method("replace",            ["year", "month", "day", "hour", "minute", "second", "microsecond", "ambiguous"], Fields,
            |s, [year, month, day, hour, minute, second, microsecond, ambiguous]| s.dt().replace(year, month, day, hour, minute, second, microsecond, ambiguous))
            .with_default(["year", "month", "day", "hour", "minute", "second", "microsecond"], lit(NULL))
            .with_default(["ambiguous"], lit("raise")),
        dt.method("replace_time_zone",  ["ambiguous"], Named(&["time_zone", "non_existent"]),
            |s, [ambiguous]| s.dt().replace_time_zone(None, ambiguous, NonExistent::Raise))
            .with_default(["ambiguous"], lit("raise")),
        dt.method("round",              ["every"],     Fields,                |s, [every]| s.dt().round(every)),
        dt.method("second",             [],            Fields,                |s, []| s.dt().second()),
        dt.method("time",               [],            Fields,                |s, []| s.dt().time()),
        dt.method("timestamp",          [],            Named(&["time_unit"]), |s, []| s.dt().timestamp(TimeUnit::Microseconds)),
        dt.method("to_string",          [],            Named(&["format"]),    |s, []| s.dt().to_string("iso")),
        dt.method("total_days",         [],            Fields,                |s, []| s.dt().total_days(false)),
        dt.method("total_hours",        [],            Fields,                |s, []| s.dt().total_hours(false)),
        dt.method("total_microseconds", [],            Fields,                |s, []| s.dt().total_microseconds(false)),
        dt.method("total_milliseconds", [],            Fields,                |s, []| s.dt().total_milliseconds(false)),
        dt.method("total_minutes",      [],            Fields,                |s, []| s.dt().total_minutes(false)),
        dt.method("total_nanoseconds",  [],            Fields,                |s, []| s.dt().total_nanoseconds(false)),
        dt.method("total_seconds",      [],            Fields,                |s, []| s.dt().total_seconds(false)),
        dt.method("truncate",           ["every"],     Fields,                |s, [every]| s.dt().truncate(every)),
        dt.method("week",               [],            Fields,                |s, []| s.dt().week()),
        dt.method("weekday",            [],            Fields,                |s, []| s.dt().weekday()),
        dt.method("with_time_unit",     [],            Named(&["time_unit"]), |s, []| s.dt().with_time_unit(TimeUnit::Microseconds)),
        dt.method("year",               [],            Fields,                |s, []| s.dt().year()),

        // built here, as the function `datetime` gives its node an alias
        temporal.method("datetime", ["year", "month", "day", "hour", "minute", "second", "microsecond", "ambiguous"], Fields,
            |_, parts| Expr::Function {
                input: parts.into(),
                function: FunctionExpr::TemporalExpr(TemporalFunction::DatetimeFunction { time_unit: TimeUnit::Microseconds, time_zone: None }),
            })
            .with_default(["hour", "minute", "second", "microsecond"], lit(0))
            .with_default(["ambiguous"], lit("raise")),
        temporal.method("duration", ["weeks", "days", "hours", "minutes", "seconds", "milliseconds", "microseconds", "nanoseconds"], Named(&["time_unit"]),
            |_, [weeks, days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds]| duration(DurationArgs {
                weeks, days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds,
                time_unit: TimeUnit::Microseconds,
            }))
            .with_default(["weeks", "days", "hours", "minutes", "seconds", "milliseconds", "microseconds", "nanoseconds"], lit(0)),
    ]
}
