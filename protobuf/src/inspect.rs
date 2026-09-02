//! Introspection primitives: messages, fields, enums.

use prost_reflect::Kind;

use elle_plugin::{ElleCtx, ElleResult, ElleValue};

use crate::schema::get_pool;

// ---------------------------------------------------------------------------
// protobuf/messages
// ---------------------------------------------------------------------------

pub fn prim_messages(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    const PRIM: &str = "protobuf/messages";

    let pool = match get_pool(ctx, unsafe { a.arg(args, nargs, 0) }, PRIM) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let names: Vec<ElleValue> = pool
        .all_messages()
        .map(|desc| a.string(ctx, desc.full_name()))
        .collect();

    a.ok(a.array(ctx, &names))
}

// ---------------------------------------------------------------------------
// protobuf/fields
// ---------------------------------------------------------------------------

pub fn prim_fields(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    const PRIM: &str = "protobuf/fields";

    let pool = match get_pool(ctx, unsafe { a.arg(args, nargs, 0) }, PRIM) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let msg_name_val = unsafe { a.arg(args, nargs, 1) };
    let msg_name = match a.get_string(msg_name_val) {
        Some(s) => s.to_string(),
        None => {
            return a.err(ctx, "type-error", &format!("{}: message name must be a string, got {}", PRIM, a.type_name(msg_name_val)));
        }
    };

    let msg_desc = match pool.get_message_by_name(&msg_name) {
        Some(d) => d,
        None => {
            return a.err(ctx, "protobuf-error", &format!("{}: message '{}' not found in pool", PRIM, msg_name));
        }
    };

    let field_structs: Vec<ElleValue> = msg_desc
        .fields()
        .map(|f| {
            let type_kw = kind_to_keyword(&f.kind());
            let label_kw = if f.is_list() { "repeated" } else { "optional" };

            let mut kvs: Vec<(&str, ElleValue)> = vec![
                ("name", a.string(ctx, f.name())),
                ("number", a.int(f.number() as i64)),
                ("type", a.keyword(ctx, type_kw)),
                ("label", a.keyword(ctx, label_kw)),
            ];

            let message_type_opt = match &f.kind() {
                Kind::Message(msg) => Some(msg.full_name().to_string()),
                Kind::Enum(e) => Some(e.full_name().to_string()),
                _ => None,
            };
            // Need to hold the string alive for the build_struct call
            let mt_string;
            if let Some(mt) = message_type_opt {
                mt_string = mt;
                kvs.push(("message-type", a.string(ctx, &mt_string)));
            }

            a.build_struct(ctx, &kvs)
        })
        .collect();

    a.ok(a.array(ctx, &field_structs))
}

// ---------------------------------------------------------------------------
// protobuf/enums
// ---------------------------------------------------------------------------

pub fn prim_enums(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    const PRIM: &str = "protobuf/enums";

    let pool = match get_pool(ctx, unsafe { a.arg(args, nargs, 0) }, PRIM) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let enum_structs: Vec<ElleValue> = pool
        .all_enums()
        .map(|e_desc| {
            let values: Vec<ElleValue> = e_desc
                .values()
                .map(|v| {
                    a.build_struct(ctx, &[
                        ("name", a.string(ctx, v.name())),
                        ("number", a.int(v.number() as i64)),
                    ])
                })
                .collect();

            a.build_struct(ctx, &[
                ("name", a.string(ctx, e_desc.full_name())),
                ("values", a.array(ctx, &values)),
            ])
        })
        .collect();

    a.ok(a.array(ctx, &enum_structs))
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn kind_to_keyword(kind: &Kind) -> &'static str {
    match kind {
        Kind::Double => "double",
        Kind::Float => "float",
        Kind::Int32 => "int32",
        Kind::Int64 => "int64",
        Kind::Uint32 => "uint32",
        Kind::Uint64 => "uint64",
        Kind::Sint32 => "sint32",
        Kind::Sint64 => "sint64",
        Kind::Fixed32 => "fixed32",
        Kind::Fixed64 => "fixed64",
        Kind::Sfixed32 => "sfixed32",
        Kind::Sfixed64 => "sfixed64",
        Kind::Bool => "bool",
        Kind::String => "string",
        Kind::Bytes => "bytes",
        Kind::Message(_) => "message",
        Kind::Enum(_) => "enum",
    }
}
