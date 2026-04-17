//! Schema loading: parse `.proto` text or binary `FileDescriptorSet` into a
//! `prost_reflect::DescriptorPool`.

use std::io::Write;

use prost_reflect::DescriptorPool;
use protobuf::Message as ProtobufMessage;

use elle_plugin::{ElleResult, ElleValue};

// ---------------------------------------------------------------------------
// Internal: parse .proto text via temp file
// ---------------------------------------------------------------------------

pub(crate) fn parse_proto_string(
    proto_src: &str,
    virtual_name: &str,
    include_dirs: &[String],
) -> Result<DescriptorPool, String> {
    let dir = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {}", e))?;
    let proto_path = dir.path().join(virtual_name);

    {
        let mut f = std::fs::File::create(&proto_path)
            .map_err(|e| format!("failed to create temp file: {}", e))?;
        f.write_all(proto_src.as_bytes())
            .map_err(|e| format!("failed to write temp file: {}", e))?;
    }

    let mut parser = protobuf_parse::Parser::new();
    parser.pure();
    parser.include(dir.path());
    parser.input(&proto_path);

    for extra_dir in include_dirs {
        parser.include(extra_dir);
    }

    let parsed = parser.parse_and_typecheck().map_err(|e| format!("{}", e))?;

    let mut fds = protobuf::descriptor::FileDescriptorSet::new();
    fds.file = parsed.file_descriptors;

    let bytes = fds
        .write_to_bytes()
        .map_err(|e| format!("failed to serialize FileDescriptorSet: {}", e))?;

    DescriptorPool::decode(bytes.as_slice())
        .map_err(|e| format!("failed to decode FileDescriptorSet: {}", e))
}

// ---------------------------------------------------------------------------
// Primitive: protobuf/schema
// ---------------------------------------------------------------------------

pub fn prim_schema(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    const PRIM: &str = "protobuf/schema";

    let val0 = a.arg(args, nargs, 0);
    let proto_src = match a.get_string(val0) {
        Some(s) => s.to_string(),
        None => {
            return a.err("type-error", &format!("{}: expected string, got {}", PRIM, a.type_name(val0)));
        }
    };

    let mut virtual_name = "input.proto".to_string();
    let mut include_dirs: Vec<String> = Vec::new();

    if nargs >= 2 {
        let opts = a.arg(args, nargs, 1);
        if !a.check_nil(opts) {
            if !a.check_struct(opts) {
                return a.err("type-error", &format!("{}: expected struct for options, got {}", PRIM, a.type_name(opts)));
            }

            let path_val = a.get_struct_field(opts, "path");
            if !a.check_nil(path_val) {
                match a.get_string(path_val) {
                    Some(p) => virtual_name = p.to_string(),
                    None => {
                        return a.err("type-error", &format!("{}: :path must be a string, got {}", PRIM, a.type_name(path_val)));
                    }
                }
            }

            let includes_val = a.get_struct_field(opts, "includes");
            if !a.check_nil(includes_val) {
                match extract_string_array(includes_val, PRIM, ":includes") {
                    Ok(dirs) => include_dirs = dirs,
                    Err(e) => return e,
                }
            }
        }
    }

    match parse_proto_string(&proto_src, &virtual_name, &include_dirs) {
        Ok(pool) => a.ok(a.external("protobuf/pool", pool)),
        Err(e) => a.err("protobuf-error", &format!("{}: {}", PRIM, e)),
    }
}

// ---------------------------------------------------------------------------
// Primitive: protobuf/schema-bytes
// ---------------------------------------------------------------------------

pub fn prim_schema_bytes(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    const PRIM: &str = "protobuf/schema-bytes";

    let val0 = a.arg(args, nargs, 0);
    let bytes = match extract_bytes(val0, PRIM) {
        Ok(b) => b,
        Err(e) => return e,
    };

    match DescriptorPool::decode(bytes.as_slice()) {
        Ok(pool) => a.ok(a.external("protobuf/pool", pool)),
        Err(e) => a.err("protobuf-error", &format!("{}: {}", PRIM, e)),
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub(crate) fn get_pool<'a>(val: ElleValue, prim: &str) -> Result<&'a DescriptorPool, ElleResult> {
    let a = crate::api();
    a.get_external::<DescriptorPool>(val, "protobuf/pool")
        .ok_or_else(|| a.err("type-error", &format!("{}: expected protobuf/pool, got {}", prim, a.type_name(val))))
}

pub(crate) fn extract_bytes(val: ElleValue, prim: &str) -> Result<Vec<u8>, ElleResult> {
    let a = crate::api();
    if let Some(b) = a.get_bytes(val) {
        return Ok(b.to_vec());
    }
    Err(a.err("type-error", &format!("{}: expected bytes, got {}", prim, a.type_name(val))))
}

pub(crate) fn extract_string_array(
    val: ElleValue,
    prim: &str,
    field: &str,
) -> Result<Vec<String>, ElleResult> {
    let a = crate::api();
    let arr_len = a.get_array_len(val).ok_or_else(|| {
        a.err("type-error", &format!("{}: {} must be an array, got {}", prim, field, a.type_name(val)))
    })?;

    let mut result = Vec::with_capacity(arr_len);
    for i in 0..arr_len {
        let item = a.get_array_item(val, i);
        match a.get_string(item) {
            Some(s) => result.push(s.to_string()),
            None => {
                return Err(a.err("type-error", &format!("{}: {} elements must be strings, got {}", prim, field, a.type_name(item))));
            }
        }
    }
    Ok(result)
}
