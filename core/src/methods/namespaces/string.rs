//! The methods of `.str`.

use polars::prelude::*;

use crate::methods::Method;
use crate::methods::derive::Payload::{Fields, Named};
use crate::methods::derive::{Namespace, WithDefault};

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let str = Namespace { name: "str", at: "/Function/function/StringExpr" };

    let concat = Namespace { name: "", at: "/Function/function/StringExpr" };

    vec![
        concat.method("concat_str", ["*exprs"], Fields, |_, [exprs]| concat_str([exprs], "", false)),

        str.method("base64_decode",     [],                   Named(&["strict"]),           |s, []| s.str().base64_decode(true)),
        str.method("base64_encode",     [],                   Fields,                       |s, []| s.str().base64_encode()),
        str.method("contains",          ["pattern"],          Fields,                       |s, [pattern]| s.str().contains(pattern, true)),
        str.method("contains_any",      ["patterns"],         Fields,                       |s, [patterns]| s.str().contains_any(patterns, false)),
        str.method("count_matches",     ["pattern"],          Named(&["literal"]),          |s, [pattern]| s.str().count_matches(pattern, false)),
        str.method("ends_with",         ["suffix"],           Fields,                       |s, [suffix]| s.str().ends_with(suffix)),
        str.method("escape_regex",      [],                   Fields,                       |s, []| s.str().escape_regex()),
        str.method("extract",           ["pattern"],          Named(&["group_index"]),      |s, [pattern]| s.str().extract(pattern, 1)),
        str.method("extract_all",       ["pattern"],          Fields,                       |s, [pattern]| s.str().extract_all(pattern)),
        str.method("extract_many",      ["patterns"],         Fields,
            |s, [patterns]| s.str().extract_many(patterns, false, false, false)),
        str.method("find",              ["pattern"],          Fields,                       |s, [pattern]| s.str().find(pattern, true)),
        str.method("find_many",         ["patterns"],         Fields,
            |s, [patterns]| s.str().find_many(patterns, false, false, false)),
        str.method("head",              ["n"],                Fields,                       |s, [n]| s.str().head(n))
            .with_default(["n"], lit(5)),
        str.method("hex_decode",        [],                   Named(&["strict"]),           |s, []| s.str().hex_decode(true)),
        str.method("hex_encode",        [],                   Fields,                       |s, []| s.str().hex_encode()),
        str.method("join",              [],                   Fields,                       |s, []| s.str().join("", true)),
        str.method("json_decode",       [],                   Named(&["dtype"]),            |s, []| s.str().json_decode(DataType::String)),
        str.method("json_path_match",   ["json_path"],        Fields,                       |s, [json_path]| s.str().json_path_match(json_path)),
        str.method("len_bytes",         [],                   Fields,                       |s, []| s.str().len_bytes()),
        str.method("len_chars",         [],                   Fields,                       |s, []| s.str().len_chars()),
        str.method("normalize",         [],                   Named(&["form"]),             |s, []| s.str().normalize(UnicodeForm::NFC)),
        str.method("pad_end",           ["length"],           Named(&["fill_char"]),        |s, [length]| s.str().pad_end(length, ' ')),
        str.method("pad_start",         ["length"],           Named(&["fill_char"]),        |s, [length]| s.str().pad_start(length, ' ')),
        str.method("replace",           ["pattern", "value"], Fields,
            |s, [pattern, value]| s.str().replace(pattern, value, false)),
        str.method("replace_many",      ["patterns", "replace_with"], Fields,
            |s, [patterns, replace_with]| s.str().replace_many(patterns, replace_with, false, false)),
        str.method("reverse",           [],                   Fields,                       |s, []| s.str().reverse()),
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
        str.method("tail",              ["n"],                Fields,                       |s, [n]| s.str().tail(n))
            .with_default(["n"], lit(5)),
        str.method("to_decimal",        [],                   Fields,                       |s, []| s.str().to_decimal(0)),
        str.method("to_integer",        ["base"],             Fields,                       |s, [base]| s.str().to_integer(base, None, true))
            .with_default(["base"], lit(10)),
        str.method("to_lowercase",      [],                   Fields,                       |s, []| s.str().to_lowercase()),
        str.method("to_uppercase",      [],                   Fields,                       |s, []| s.str().to_uppercase()),
        str.method("zfill",             ["length"],           Fields,                       |s, [length]| s.str().zfill(length)),
    ]
}
