//! The methods of `.bin`.

use polars::prelude::*;

use crate::methods::Method;
use crate::methods::derive::Payload::{Fields, Named};
use crate::methods::derive::{Namespace, WithDefault};

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let bin = Namespace { name: "bin", at: "/Function/function/BinaryExpr" };

    vec![
        bin.method("contains",    ["literal"],          Fields,                  |s, [literal]| s.binary().contains_literal(literal)),
        bin.method("base64_decode", [],                 Named(&["strict"]),      |s, []| s.binary().base64_decode(true)),
        bin.method("base64_encode", [],                 Fields,                  |s, []| s.binary().base64_encode()),
        bin.method("ends_with",   ["suffix"],           Fields,                  |s, [suffix]| s.binary().ends_with(suffix)),
        bin.method("get",         ["index"],            Named(&["null_on_oob"]), |s, [index]| s.binary().get(index, false)),
        bin.method("head",        ["n"],                Fields,                  |s, [n]| s.binary().head(n))
            .with_default(["n"], lit(5)),
        bin.method("hex_decode",  [],                   Named(&["strict"]),      |s, []| s.binary().hex_decode(true)),
        bin.method("hex_encode",  [],                   Fields,                  |s, []| s.binary().hex_encode()),
        bin.method("reinterpret", [],                   Named(&["dtype", "is_little_endian"]),               |s, []| s.binary().reinterpret(DataType::Int64, true)),
        bin.method("size",        [],                   Fields,                  |s, []| s.binary().size_bytes()),
        bin.method("slice",       ["offset", "length"], Fields,                  |s, [offset, length]| s.binary().slice(offset, length))
            .with_default(["length"], lit(NULL)),
        bin.method("starts_with", ["prefix"],           Fields,                  |s, [prefix]| s.binary().starts_with(prefix)),
        bin.method("tail",        ["n"],                Fields,                  |s, [n]| s.binary().tail(n))
            .with_default(["n"], lit(5)),
    ]
}
