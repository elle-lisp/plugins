//! Elle TOML plugin — TOML parsing and serialization via the `toml` crate.

use elle_plugin::{ElleCtx, ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

elle_plugin::define_plugin!("toml/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Type conversion: TOML → Elle
// ---------------------------------------------------------------------------

/// Recursively convert a `toml::Value` to an Elle `ElleValue`.
/// Tables become immutable structs with keyword keys.
/// Arrays become immutable arrays.
/// Datetimes become their ISO 8601 string representation.
fn toml_to_value(ctx: *mut ElleCtx, tv: toml::Value) -> ElleValue {
    let a = api();
    match tv {
        toml::Value::String(s) => a.string(ctx, &s),
        toml::Value::Integer(i) => a.int(i),
        toml::Value::Float(f) => a.float(f),
        toml::Value::Boolean(b) => a.boolean(b),
        toml::Value::Array(arr) => {
            let items: Vec<ElleValue> = arr.into_iter().map(|v| toml_to_value(ctx, v)).collect();
            a.array(ctx, &items)
        }
        toml::Value::Table(t) => {
            let owned_keys: Vec<String> = t.keys().cloned().collect();
            let values: Vec<ElleValue> = t.values().cloned().map(|v| toml_to_value(ctx, v)).collect();
            let kvs: Vec<(&str, ElleValue)> = owned_keys
                .iter()
                .zip(values.iter())
                .map(|(k, v)| (k.as_str(), *v))
                .collect();
            a.build_struct(ctx, &kvs)
        }
        toml::Value::Datetime(dt) => {
            let s = dt.to_string();
            a.string(ctx, &s)
        }
    }
}

// ---------------------------------------------------------------------------
// Type conversion: Elle → TOML
// ---------------------------------------------------------------------------

/// Recursively convert an Elle `ElleValue` to a `toml::Value`.
/// Returns an error for types that have no TOML equivalent (nil, closures, etc.).
fn value_to_toml(ctx: *mut ElleCtx, v: ElleValue, name: &str) -> Result<toml::Value, ElleResult> {
    let a = api();
    if let Some(s) = a.get_string(v) {
        return Ok(toml::Value::String(s.to_string()));
    }
    if let Some(i) = a.get_int(v) {
        return Ok(toml::Value::Integer(i));
    }
    if let Some(f) = a.get_float(v) {
        return Ok(toml::Value::Float(f));
    }
    if let Some(b) = a.get_bool(v) {
        return Ok(toml::Value::Boolean(b));
    }
    // Array
    if let Some(len) = a.get_array_len(v) {
        let mut items = Vec::with_capacity(len);
        for i in 0..len {
            items.push(value_to_toml(ctx, a.get_array_item(v, i), name)?);
        }
        return Ok(toml::Value::Array(items));
    }
    // Struct — keyword keys become TOML table keys
    if a.check_struct(v) {
        let entries = a.struct_entries(ctx, v);
        let mut table = toml::map::Map::new();
        for (key, val) in entries {
            table.insert(key.to_string(), value_to_toml(ctx, val, name)?);
        }
        return Ok(toml::Value::Table(table));
    }
    // nil → explicit error (TOML has no null type)
    if a.check_nil(v) {
        return Err(a.err(ctx, 
            "toml-error",
            &format!(
                "{}: cannot encode nil as TOML (TOML has no null type)",
                name
            ),
        ));
    }
    Err(a.err(ctx, 
        "toml-error",
        &format!("{}: cannot encode {} as TOML", name, a.type_name(v)),
    ))
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_toml_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "toml/parse";
    if nargs != 1 {
        return a.err(ctx, 
            "arity-error",
            &format!("{}: expected 1 argument, got {}", name, nargs),
        );
    }
    let text = match a.get_string(unsafe { a.arg(args, nargs, 0) }) {
        Some(s) => s.to_string(),
        None => {
            return a.err(ctx, 
                "type-error",
                &format!(
                    "{}: expected string, got {}",
                    name,
                    a.type_name(unsafe { a.arg(args, nargs, 0) })
                ),
            );
        }
    };
    match toml::from_str::<toml::Value>(&text) {
        Ok(tv) => a.ok(toml_to_value(ctx, tv)),
        Err(e) => a.err(ctx, "toml-error", &format!("{}: {}", name, e)),
    }
}

extern "C" fn prim_toml_encode(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "toml/encode";
    if nargs != 1 {
        return a.err(ctx, 
            "arity-error",
            &format!("{}: expected 1 argument, got {}", name, nargs),
        );
    }
    let tv = match value_to_toml(ctx, unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match toml::to_string(&tv) {
        Ok(s) => a.ok(a.string(ctx, &s)),
        Err(e) => a.err(ctx, "toml-error", &format!("{}: {}", name, e)),
    }
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact(
        "toml/parse",
        prim_toml_parse,
        SIG_ERROR,
        1,
        "Parse a TOML string to an Elle value. Tables become immutable structs with keyword keys. Arrays become immutable arrays. Datetimes become strings.",
        "toml",
        r#"(toml/parse "[package]\nname = \"hello\"")"#,
    ),
    EllePrimDef::exact(
        "toml/encode",
        prim_toml_encode,
        SIG_ERROR,
        1,
        "Encode an Elle value to a TOML string. Structs become TOML tables. Arrays become TOML arrays. nil values are an error (TOML has no null type).",
        "toml",
        r#"(toml/encode {:name "hello" :version 1})"#,
    ),
];
