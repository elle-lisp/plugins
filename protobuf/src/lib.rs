//! Elle protobuf plugin — dynamic protobuf encode/decode via descriptor pools.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR, SIG_OK};

elle_plugin::define_plugin!("protobuf/", &PRIMITIVES);

mod convert;
mod inspect;
mod schema;

// ---------------------------------------------------------------------------
// Primitive wrappers
// ---------------------------------------------------------------------------

extern "C" fn prim_schema(args: *const ElleValue, nargs: usize) -> ElleResult {
    schema::prim_schema(args, nargs)
}

extern "C" fn prim_schema_bytes(args: *const ElleValue, nargs: usize) -> ElleResult {
    schema::prim_schema_bytes(args, nargs)
}

extern "C" fn prim_encode(args: *const ElleValue, nargs: usize) -> ElleResult {
    convert::encode(args, nargs)
}

extern "C" fn prim_decode(args: *const ElleValue, nargs: usize) -> ElleResult {
    convert::decode(args, nargs)
}

extern "C" fn prim_messages(args: *const ElleValue, nargs: usize) -> ElleResult {
    inspect::prim_messages(args, nargs)
}

extern "C" fn prim_fields(args: *const ElleValue, nargs: usize) -> ElleResult {
    inspect::prim_fields(args, nargs)
}

extern "C" fn prim_enums(args: *const ElleValue, nargs: usize) -> ElleResult {
    inspect::prim_enums(args, nargs)
}

// ---------------------------------------------------------------------------
// Primitive registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range("protobuf/schema", prim_schema, SIG_ERROR, 1, 2, "Parse .proto source text into a descriptor pool. Optional second arg: {:path \"name.proto\" :includes [\"dir\"]}.", "protobuf", r#"(protobuf/schema "syntax = \"proto3\"; message Foo { string x = 1; }")"#),
    EllePrimDef::exact("protobuf/schema-bytes", prim_schema_bytes, SIG_ERROR, 1, "Load a pre-compiled binary FileDescriptorSet into a descriptor pool.", "protobuf", "(protobuf/schema-bytes my-fds-bytes)"),
    EllePrimDef::exact("protobuf/encode", prim_encode, SIG_ERROR, 3, "Encode an Elle struct to protobuf bytes using the given descriptor pool and message name.", "protobuf", r#"(protobuf/encode pool "Person" {:name "Alice" :age 30})"#),
    EllePrimDef::exact("protobuf/decode", prim_decode, SIG_ERROR, 3, "Decode protobuf bytes to an Elle struct using the given descriptor pool and message name.", "protobuf", r#"(protobuf/decode pool "Person" buf)"#),
    EllePrimDef::exact("protobuf/messages", prim_messages, SIG_ERROR, 1, "List fully-qualified message names in a descriptor pool.", "protobuf", "(protobuf/messages pool)"),
    EllePrimDef::exact("protobuf/fields", prim_fields, SIG_ERROR, 2, "List fields of a message. Returns array of {:name :number :type :label} structs.", "protobuf", r#"(protobuf/fields pool "Person")"#),
    EllePrimDef::exact("protobuf/enums", prim_enums, SIG_ERROR, 1, "List enum types in a descriptor pool. Returns array of {:name :values} structs.", "protobuf", "(protobuf/enums pool)"),
];
