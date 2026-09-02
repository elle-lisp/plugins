//! Elle YAML plugin — YAML parsing and serialization via the `serde_yaml_ng` crate.

use serde::Deserialize;

use elle_plugin::{ElleCtx, ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

elle_plugin::define_plugin!("yaml/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Type conversion: YAML → Elle
// ---------------------------------------------------------------------------

/// Recursively convert a `serde_yaml_ng::Value` to an Elle `ElleValue`.
/// Mappings become immutable structs with keyword keys.
/// Sequences become immutable arrays.
/// Null becomes nil.
fn yaml_to_value(ctx: *mut ElleCtx, yv: serde_yaml_ng::Value) -> Result<ElleValue, String> {
    let a = api();
    match yv {
        serde_yaml_ng::Value::Null => Ok(a.nil()),
        serde_yaml_ng::Value::Bool(b) => Ok(a.boolean(b)),
        serde_yaml_ng::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(a.int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(a.float(f))
            } else {
                Err(format!("yaml: unsupported number: {}", n))
            }
        }
        serde_yaml_ng::Value::String(s) => Ok(a.string(ctx, &s)),
        serde_yaml_ng::Value::Sequence(seq) => {
            let items: Result<Vec<_>, _> = seq.into_iter().map(|v| yaml_to_value(ctx, v)).collect();
            let items = items?;
            Ok(a.array(ctx, &items))
        }
        serde_yaml_ng::Value::Mapping(map) => {
            let mut fields: Vec<(&str, ElleValue)> = Vec::new();
            // We need owned strings to live long enough for build_struct
            let mut owned_keys: Vec<String> = Vec::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml_ng::Value::String(s) => s,
                    other => {
                        return Err(format!("yaml: non-string map key: {:?}", other));
                    }
                };
                owned_keys.push(key);
                fields.push(("", yaml_to_value(ctx, v)?));
            }
            // Patch in key references now that owned_keys is stable
            let kvs: Vec<(&str, ElleValue)> = owned_keys
                .iter()
                .zip(fields.iter())
                .map(|(k, (_, v))| (k.as_str(), *v))
                .collect();
            Ok(a.build_struct(ctx, &kvs))
        }
        serde_yaml_ng::Value::Tagged(tagged) => yaml_to_value(ctx, tagged.value),
    }
}

// ---------------------------------------------------------------------------
// Type conversion: Elle → YAML
// ---------------------------------------------------------------------------

/// Recursively convert an Elle `ElleValue` to a `serde_yaml_ng::Value`.
/// Returns an error for types that have no YAML equivalent (closures, etc.).
/// nil → Null (YAML supports null, unlike TOML).
fn value_to_yaml(ctx: *mut ElleCtx, v: ElleValue, name: &str) -> Result<serde_yaml_ng::Value, ElleResult> {
    let a = api();
    if a.check_nil(v) {
        return Ok(serde_yaml_ng::Value::Null);
    }
    if let Some(b) = a.get_bool(v) {
        return Ok(serde_yaml_ng::Value::Bool(b));
    }
    if let Some(i) = a.get_int(v) {
        return Ok(serde_yaml_ng::Value::Number(i.into()));
    }
    if let Some(f) = a.get_float(v) {
        return Ok(serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(f)));
    }
    if let Some(s) = a.get_string(v) {
        return Ok(serde_yaml_ng::Value::String(s.to_string()));
    }
    // Array
    if let Some(len) = a.get_array_len(v) {
        let mut items = Vec::with_capacity(len);
        for i in 0..len {
            items.push(value_to_yaml(ctx, a.get_array_item(v, i), name)?);
        }
        return Ok(serde_yaml_ng::Value::Sequence(items));
    }
    // Struct — keyword keys become YAML mapping string keys
    if a.check_struct(v) {
        let entries = a.struct_entries(ctx, v);
        let mut map = serde_yaml_ng::Mapping::new();
        for (key, field_val) in entries {
            let yaml_key = serde_yaml_ng::Value::String(key.to_string());
            let yaml_val = value_to_yaml(ctx, field_val, name)?;
            map.insert(yaml_key, yaml_val);
        }
        return Ok(serde_yaml_ng::Value::Mapping(map));
    }
    Err(a.err(ctx, 
        "yaml-error",
        &format!("{}: cannot encode {} as YAML", name, a.type_name(v)),
    ))
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_yaml_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "yaml/parse";
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
    match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text) {
        Ok(yv) => match yaml_to_value(ctx, yv) {
            Ok(v) => a.ok(v),
            Err(e) => a.err(ctx, "yaml-error", &format!("{}: {}", name, e)),
        },
        Err(e) => a.err(ctx, "yaml-error", &format!("{}: {}", name, e)),
    }
}

extern "C" fn prim_yaml_parse_all(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "yaml/parse-all";
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
    let mut docs = Vec::new();
    for doc in serde_yaml_ng::Deserializer::from_str(&text) {
        let yv = match serde_yaml_ng::Value::deserialize(doc) {
            Ok(v) => v,
            Err(e) => {
                return a.err(ctx, "yaml-error", &format!("{}: {}", name, e));
            }
        };
        match yaml_to_value(ctx, yv) {
            Ok(v) => docs.push(v),
            Err(e) => {
                return a.err(ctx, "yaml-error", &format!("{}: {}", name, e));
            }
        }
    }
    a.ok(a.array(ctx, &docs))
}

extern "C" fn prim_yaml_encode(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "yaml/encode";
    if nargs != 1 {
        return a.err(ctx, 
            "arity-error",
            &format!("{}: expected 1 argument, got {}", name, nargs),
        );
    }
    let yv = match value_to_yaml(ctx, unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match serde_yaml_ng::to_string(&yv) {
        Ok(s) => a.ok(a.string(ctx, &s)),
        Err(e) => a.err(ctx, "yaml-error", &format!("{}: {}", name, e)),
    }
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact(
        "yaml/parse",
        prim_yaml_parse,
        SIG_ERROR,
        1,
        "Parse a YAML string (first document) to an Elle value. Mappings become immutable structs with keyword keys. Sequences become immutable arrays. Null becomes nil.",
        "yaml",
        r#"(yaml/parse "name: hello\nversion: 1")"#,
    ),
    EllePrimDef::exact(
        "yaml/parse-all",
        prim_yaml_parse_all,
        SIG_ERROR,
        1,
        "Parse all YAML documents in a string. Returns an array of values, one per document. Documents are separated by `---`.",
        "yaml",
        r#"(yaml/parse-all "---\na: 1\n---\nb: 2")"#,
    ),
    EllePrimDef::exact(
        "yaml/encode",
        prim_yaml_encode,
        SIG_ERROR,
        1,
        "Encode an Elle value to a YAML string. Structs become YAML mappings. Arrays become YAML sequences. nil becomes YAML null.",
        "yaml",
        r#"(yaml/encode {:name "hello" :version 1})"#,
    ),
];
