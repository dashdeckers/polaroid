//! The methods of `.struct`, and those of `.name` that rename its fields.

use crate::methods::Method;
use crate::methods::derive::Namespace;
use crate::methods::derive::Payload::{Fields, Named};

#[rustfmt::skip]
pub(super) fn methods() -> Vec<Result<Method, String>> {
    let structs = Namespace { name: "struct", at: "/Function/function/StructExpr" };
    let name = Namespace { name: "name", at: "/Function/function/StructExpr" };

    vec![
        structs.method("drop",          [], Named(&["names", "strict"]), |s, []| s.struct_().drop([""; 0], true)),
        structs.method("field",         [], Named(&["name"]),            |s, []| s.struct_().field_by_name("")),
        structs.method("json_encode",   [], Fields,                      |s, []| s.struct_().json_encode()),
        structs.method("rename_fields", [], Named(&["names"]),           |s, []| s.struct_().rename_fields([""; 0])),

        name.method("prefix_fields", [], Named(&["prefix"]), |s, []| s.name().prefix_fields("")),
        name.method("suffix_fields", [], Named(&["suffix"]), |s, []| s.name().suffix_fields("")),
    ]
}
