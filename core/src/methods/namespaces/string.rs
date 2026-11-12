//! The methods of `.str`.

use polars::prelude::*;

use crate::methods::Method;
use crate::methods::derive::Payload::{Fields, Named};
use crate::methods::derive::{Namespace, WithDefault};

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let str = Namespace { name: "str", at: "/Function/function/StringExpr" };

    vec![
        str.method("contains",          ["pattern"],          Fields,                       |s, [pattern]| s.str().contains(pattern, true)),
        str.method("count_matches",     ["pattern"],          Named(&["literal"]),          |s, [pattern]| s.str().count_matches(pattern, false)),
        str.method("ends_with",         ["suffix"],           Fields,                       |s, [suffix]| s.str().ends_with(suffix)),
        str.method("escape_regex",      [],                   Fields,                       |s, []| s.str().escape_regex()),
        str.method("extract",           ["pattern"],          Named(&["group_index"]),      |s, [pattern]| s.str().extract(pattern, 1)),
        str.method("extract_all",       ["pattern"],          Fields,                       |s, [pattern]| s.str().extract_all(pattern)),
        str.method("find",              ["pattern"],          Fields,                       |s, [pattern]| s.str().find(pattern, true)),
        str.method("head",              ["n"],                Fields,                       |s, [n]| s.str().head(n)),
        str.method("len_bytes",         [],                   Fields,                       |s, []| s.str().len_bytes()),
        str.method("len_chars",         [],                   Fields,                       |s, []| s.str().len_chars()),
        str.method("replace",           ["pattern", "value"], Fields,
            |s, [pattern, value]| s.str().replace(pattern, value, false)),
        str.method("slice",             ["offset", "length"], Fields,                       |s, [offset, length]| s.str().slice(offset, length))
            .with_default(["length"], lit(NULL)),
        str.method("split",             ["by"],               Named(&["inclusive"]),        |s, [by]| s.str().split(by)),
        str.method("split_exact",       ["by"],               Fields,                       |s, [by]| s.str().split_exact(by, 1)),
        str.method("split_regex",       ["pattern"],          Fields,                       |s, [pattern]| s.str().split_regex(pattern, true)),
        str.method("splitn",            ["by"],               Named(&["n"]),                |s, [by]| s.str().splitn(by, 2)),
        str.method("starts_with",       ["prefix"],           Fields,                       |s, [prefix]| s.str().starts_with(prefix)),
        str.method("strip_chars",       ["characters"],       Fields,                       |s, [characters]| s.str().strip_chars(characters))
            .with_default(["characters"], lit(NULL)),
        str.method("strip_chars_end",   ["characters"],       Fields,                       |s, [characters]| s.str().strip_chars_end(characters))
            .with_default(["characters"], lit(NULL)),
        str.method("strip_chars_start", ["characters"],       Fields,                       |s, [characters]| s.str().strip_chars_start(characters))
            .with_default(["characters"], lit(NULL)),
        str.method("strip_prefix",      ["prefix"],           Fields,                       |s, [prefix]| s.str().strip_prefix(prefix)),
        str.method("strip_suffix",      ["suffix"],           Fields,                       |s, [suffix]| s.str().strip_suffix(suffix)),
        str.method("strptime",          ["ambiguous"],        Named(&["dtype", "options"]),
            |s, [ambiguous]| s.str().strptime(DataType::Date, StrptimeOptions::default(), ambiguous))
            .with_default(["ambiguous"], lit("raise")),
        str.method("tail",              ["n"],                Fields,                       |s, [n]| s.str().tail(n)),
        str.method("to_decimal",        [],                   Fields,                       |s, []| s.str().to_decimal(0)),
        str.method("to_lowercase",      [],                   Fields,                       |s, []| s.str().to_lowercase()),
        str.method("to_uppercase",      [],                   Fields,                       |s, []| s.str().to_uppercase()),
    ]
}
