//! Conversion between Elle `ElleValue`s and `prost_reflect::Value`s.

use std::collections::HashMap;

use prost_reflect::{
    DynamicMessage, FieldDescriptor, Kind, MapKey, ReflectMessage, Value as PbValue,
};

use elle_plugin::{ElleResult, ElleValue};

// ---------------------------------------------------------------------------
// Elle -> Protobuf (encode)
// ---------------------------------------------------------------------------

/// Convert an Elle `ElleValue` to a `prost_reflect::Value` for the given field.
fn elle_to_pb(val: ElleValue, field: &FieldDescriptor) -> Result<PbValue, String> {
    let a = crate::api();

    if a.check_nil(val) {
        return Err(format!(
            "nil is not a valid value for field '{}'",
            field.name()
        ));
    }

    match field.kind() {
        Kind::Bool => match a.get_bool(val) {
            Some(b) => Ok(PbValue::Bool(b)),
            None => Err(format!("expected bool, got {}", a.type_name(val))),
        },
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => {
            let n = elle_to_i32(val, field.name())?;
            Ok(PbValue::I32(n))
        }
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => {
            let n = elle_int_val(val)?;
            Ok(PbValue::I64(n))
        }
        Kind::Uint32 | Kind::Fixed32 => {
            let n = elle_to_u32(val, field.name())?;
            Ok(PbValue::U32(n))
        }
        Kind::Uint64 | Kind::Fixed64 => {
            let n = elle_to_u64(val, field.name())?;
            Ok(PbValue::U64(n))
        }
        Kind::Float => {
            let f = a.get_float(val)
                .ok_or_else(|| format!("expected float, got {}", a.type_name(val)))?;
            Ok(PbValue::F32(f as f32))
        }
        Kind::Double => {
            let f = a.get_float(val)
                .ok_or_else(|| format!("expected float, got {}", a.type_name(val)))?;
            Ok(PbValue::F64(f))
        }
        Kind::String => {
            let s = elle_to_string(val)?;
            Ok(PbValue::String(s))
        }
        Kind::Bytes => {
            let b = elle_to_bytes(val)?;
            Ok(PbValue::Bytes(b.into()))
        }
        Kind::Enum(enum_desc) => {
            if let Some(n) = a.get_int(val) {
                return Ok(PbValue::EnumNumber(n as i32));
            }
            if let Some(kw) = a.get_keyword_name(val) {
                match enum_desc.get_value_by_name(kw) {
                    Some(v) => return Ok(PbValue::EnumNumber(v.number())),
                    None => {
                        return Err(format!(
                            "unknown enum value :{} for enum '{}'",
                            kw,
                            enum_desc.full_name()
                        ));
                    }
                }
            }
            Err(format!(
                "expected keyword or int for enum field '{}', got {}",
                field.name(),
                a.type_name(val)
            ))
        }
        Kind::Message(msg_desc) => {
            if msg_desc.is_map_entry() {
                Err(format!(
                    "map field '{}' must be encoded via encode_map, not elle_to_pb",
                    field.name()
                ))
            } else {
                let dyn_msg = encode_message(val, &msg_desc)?;
                Ok(PbValue::Message(dyn_msg))
            }
        }
    }
}

/// Encode an Elle struct into a `DynamicMessage` for `msg_desc`.
///
/// Uses the message descriptor's field list to look up fields via struct_get.
/// Map fields are encoded by iterating the struct's entries via `struct_entries`.
fn encode_message(
    val: ElleValue,
    msg_desc: &prost_reflect::MessageDescriptor,
) -> Result<DynamicMessage, String> {
    let a = crate::api();

    if !a.check_struct(val) {
        return Err(format!("expected struct, got {}", a.type_name(val)));
    }

    let mut msg = DynamicMessage::new(msg_desc.clone());

    for field_desc in msg_desc.fields() {
        let field_name = field_desc.name();
        let field_val = a.get_struct_field(val, field_name);

        // nil means "field not set" — skip it
        if a.check_nil(field_val) {
            continue;
        }

        if field_desc.is_map() {
            let pb_map = encode_map(field_val, &field_desc)?;
            msg.set_field(&field_desc, PbValue::Map(pb_map));
        } else if field_desc.is_list() {
            let pb_list = encode_repeated(field_val, &field_desc)?;
            msg.set_field(&field_desc, PbValue::List(pb_list));
        } else {
            let pb_val = elle_to_pb(field_val, &field_desc)
                .map_err(|e| format!("field '{}': {}", field_name, e))?;
            msg.set_field(&field_desc, pb_val);
        }
    }

    Ok(msg)
}

/// Encode an Elle array into a repeated protobuf list.
fn encode_repeated(val: ElleValue, field: &FieldDescriptor) -> Result<Vec<PbValue>, String> {
    let a = crate::api();
    let arr_len = a.get_array_len(val).ok_or_else(|| {
        format!(
            "field '{}': expected array for repeated field, got {}",
            field.name(),
            a.type_name(val)
        )
    })?;

    let mut result = Vec::with_capacity(arr_len);
    for i in 0..arr_len {
        let item = a.get_array_item(val, i);
        if a.check_nil(item) {
            continue;
        }
        let pb_val = match field.kind() {
            Kind::Message(msg_desc) => {
                let dyn_msg = encode_message(item, &msg_desc)?;
                PbValue::Message(dyn_msg)
            }
            _ => elle_to_pb(item, field)?,
        };
        result.push(pb_val);
    }
    Ok(result)
}

/// Encode an Elle struct into a protobuf map field.
///
/// Iterates the struct's key-value pairs via `struct_entries` and converts
/// each pair into a `(MapKey, PbValue)` entry matching the map field's
/// key/value types.
fn encode_map(
    val: ElleValue,
    field: &FieldDescriptor,
) -> Result<HashMap<MapKey, PbValue>, String> {
    let a = crate::api();

    if !a.check_struct(val) {
        return Err(format!(
            "field '{}': expected struct for map field, got {}",
            field.name(),
            a.type_name(val)
        ));
    }

    let map_entry_desc = match field.kind() {
        Kind::Message(d) => d,
        _ => {
            return Err(format!(
                "field '{}': map field does not have message kind",
                field.name()
            ));
        }
    };

    let key_field = map_entry_desc
        .get_field_by_name("key")
        .ok_or_else(|| format!("field '{}': map entry has no 'key' field", field.name()))?;
    let value_field = map_entry_desc
        .get_field_by_name("value")
        .ok_or_else(|| format!("field '{}': map entry has no 'value' field", field.name()))?;

    let entries = a.struct_entries(val);
    let mut result = HashMap::with_capacity(entries.len());

    for (key_str, entry_val) in entries {
        let map_key = match key_field.kind() {
            Kind::String => MapKey::String(key_str.to_string()),
            Kind::Bool => {
                let b = key_str.parse::<bool>().map_err(|_| {
                    format!(
                        "field '{}': cannot parse map key '{}' as bool",
                        field.name(),
                        key_str
                    )
                })?;
                MapKey::Bool(b)
            }
            Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => {
                let n = key_str.parse::<i32>().map_err(|_| {
                    format!(
                        "field '{}': cannot parse map key '{}' as int32",
                        field.name(),
                        key_str
                    )
                })?;
                MapKey::I32(n)
            }
            Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => {
                let n = key_str.parse::<i64>().map_err(|_| {
                    format!(
                        "field '{}': cannot parse map key '{}' as int64",
                        field.name(),
                        key_str
                    )
                })?;
                MapKey::I64(n)
            }
            Kind::Uint32 | Kind::Fixed32 => {
                let n = key_str.parse::<u32>().map_err(|_| {
                    format!(
                        "field '{}': cannot parse map key '{}' as uint32",
                        field.name(),
                        key_str
                    )
                })?;
                MapKey::U32(n)
            }
            Kind::Uint64 | Kind::Fixed64 => {
                let n = key_str.parse::<u64>().map_err(|_| {
                    format!(
                        "field '{}': cannot parse map key '{}' as uint64",
                        field.name(),
                        key_str
                    )
                })?;
                MapKey::U64(n)
            }
            other => {
                return Err(format!(
                    "field '{}': unsupported map key kind {:?}",
                    field.name(),
                    other
                ));
            }
        };

        let pb_val = elle_to_pb(entry_val, &value_field)
            .map_err(|e| format!("field '{}', key '{}': {}", field.name(), key_str, e))?;
        result.insert(map_key, pb_val);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Protobuf -> Elle (decode)
// ---------------------------------------------------------------------------

fn pb_to_elle(val: &PbValue, field: &FieldDescriptor) -> Result<ElleValue, String> {
    let a = crate::api();
    match val {
        PbValue::Bool(b) => Ok(a.boolean(*b)),
        PbValue::I32(n) => Ok(a.int(*n as i64)),
        PbValue::I64(n) => Ok(a.int(*n)),
        PbValue::U32(n) => Ok(a.int(*n as i64)),
        PbValue::U64(n) => {
            const ELLE_INT_MAX: u64 = i64::MAX as u64;
            if *n > ELLE_INT_MAX {
                Err(format!(
                    "field '{}': uint64 value {} out of Elle i64 range",
                    field.name(), n
                ))
            } else {
                Ok(a.int(*n as i64))
            }
        }
        PbValue::F32(f) => Ok(a.float(*f as f64)),
        PbValue::F64(f) => Ok(a.float(*f)),
        PbValue::String(s) => Ok(a.string(s.as_str())),
        PbValue::Bytes(b) => Ok(a.bytes(b.as_ref())),
        PbValue::EnumNumber(n) => {
            let enum_desc = match field.kind() {
                Kind::Enum(e) => e,
                _ => {
                    return Err(format!(
                        "field '{}': got EnumNumber but field is not enum",
                        field.name()
                    ));
                }
            };
            match enum_desc.get_value(*n) {
                Some(v) => Ok(a.keyword(v.name())),
                None => Ok(a.int(*n as i64)),
            }
        }
        PbValue::Message(dyn_msg) => decode_message(dyn_msg),
        PbValue::List(items) => {
            let element_vals: Result<Vec<ElleValue>, String> =
                items.iter().map(|item| pb_to_elle(item, field)).collect();
            let elems = element_vals?;
            Ok(a.array(&elems))
        }
        PbValue::Map(map) => decode_map(map, field),
    }
}

fn decode_message(msg: &DynamicMessage) -> Result<ElleValue, String> {
    let a = crate::api();
    let mut fields: Vec<(String, ElleValue)> = Vec::new();

    for field in msg.descriptor().fields() {
        if !msg.has_field(&field) {
            continue;
        }

        let pb_val = msg.get_field(&field);
        let elle_val = if field.is_map() {
            decode_map_field(pb_val.as_ref(), &field)?
        } else if field.is_list() {
            decode_list_field(pb_val.as_ref(), &field)?
        } else {
            pb_to_elle(pb_val.as_ref(), &field)?
        };

        fields.push((field.name().to_string(), elle_val));
    }

    let kvs: Vec<(&str, ElleValue)> = fields.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    Ok(a.build_struct(&kvs))
}

fn decode_map_field(val: &PbValue, field: &FieldDescriptor) -> Result<ElleValue, String> {
    match val {
        PbValue::Map(map) => decode_map(map, field),
        _ => Err(format!("field '{}': expected Map, got {:?}", field.name(), val)),
    }
}

fn decode_map(map: &HashMap<MapKey, PbValue>, field: &FieldDescriptor) -> Result<ElleValue, String> {
    let a = crate::api();
    let msg_desc = match field.kind() {
        Kind::Message(d) => d,
        _ => return Err(format!("field '{}': not a map field", field.name())),
    };

    let value_field = msg_desc
        .get_field_by_name("value")
        .ok_or_else(|| format!("field '{}': map entry has no 'value' field", field.name()))?;

    let mut result: Vec<(String, ElleValue)> = Vec::new();
    for (k, v) in map {
        let key_str = match k {
            MapKey::String(s) => s.clone(),
            MapKey::Bool(b) => b.to_string(),
            MapKey::I32(n) => n.to_string(),
            MapKey::I64(n) => n.to_string(),
            MapKey::U32(n) => n.to_string(),
            MapKey::U64(n) => n.to_string(),
        };
        let elle_val = pb_to_elle(v, &value_field)?;
        result.push((key_str, elle_val));
    }

    let kvs: Vec<(&str, ElleValue)> = result.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    Ok(a.build_struct(&kvs))
}

fn decode_list_field(val: &PbValue, field: &FieldDescriptor) -> Result<ElleValue, String> {
    let a = crate::api();
    match val {
        PbValue::List(items) => {
            let element_vals: Result<Vec<ElleValue>, String> =
                items.iter().map(|item| pb_to_elle(item, field)).collect();
            let elems = element_vals?;
            Ok(a.array(&elems))
        }
        _ => Err(format!("field '{}': expected List, got {:?}", field.name(), val)),
    }
}

// ---------------------------------------------------------------------------
// Elle value extraction helpers
// ---------------------------------------------------------------------------

fn elle_int_val(val: ElleValue) -> Result<i64, String> {
    let a = crate::api();
    a.get_int(val)
        .ok_or_else(|| format!("expected int, got {}", a.type_name(val)))
}

fn elle_to_i32(val: ElleValue, field_name: &str) -> Result<i32, String> {
    let n = elle_int_val(val)?;
    if n < i32::MIN as i64 || n > i32::MAX as i64 {
        Err(format!(
            "value {} out of int32 range [{}, {}] for field '{}'",
            n, i32::MIN, i32::MAX, field_name
        ))
    } else {
        Ok(n as i32)
    }
}

fn elle_to_u32(val: ElleValue, field_name: &str) -> Result<u32, String> {
    let n = elle_int_val(val)?;
    if n < 0 || n > u32::MAX as i64 {
        Err(format!(
            "value {} out of uint32 range [0, {}] for field '{}'",
            n, u32::MAX, field_name
        ))
    } else {
        Ok(n as u32)
    }
}

fn elle_to_u64(val: ElleValue, field_name: &str) -> Result<u64, String> {
    let a = crate::api();
    if let Some(n) = a.get_int(val) {
        if n < 0 {
            Err(format!(
                "negative value {} cannot be encoded as uint64 for field '{}'",
                n, field_name
            ))
        } else {
            Ok(n as u64)
        }
    } else if let Some(s) = a.get_string(val) {
        s.parse::<u64>()
            .map_err(|_| format!("cannot parse '{}' as uint64 for field '{}'", s, field_name))
    } else {
        Err(format!(
            "expected int or string for uint64 field '{}', got {}",
            field_name,
            a.type_name(val)
        ))
    }
}

fn elle_to_string(val: ElleValue) -> Result<String, String> {
    let a = crate::api();
    if let Some(s) = a.get_string(val) {
        return Ok(s.to_string());
    }
    Err(format!("expected string, got {}", a.type_name(val)))
}

fn elle_to_bytes(val: ElleValue) -> Result<Vec<u8>, String> {
    let a = crate::api();
    if let Some(b) = a.get_bytes(val) {
        return Ok(b.to_vec());
    }
    if let Some(s) = a.get_string(val) {
        return Ok(s.as_bytes().to_vec());
    }
    Err(format!("expected bytes, got {}", a.type_name(val)))
}

// ---------------------------------------------------------------------------
// Encode/decode primitives (called from lib.rs)
// ---------------------------------------------------------------------------

pub fn encode(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    const PRIM: &str = "protobuf/encode";

    let pool = match crate::schema::get_pool(unsafe { a.arg(args, nargs, 0) }, PRIM) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let msg_name_val = unsafe { a.arg(args, nargs, 1) };
    let msg_name = match a.get_string(msg_name_val) {
        Some(s) => s.to_string(),
        None => {
            return a.err("type-error", &format!("{}: message name must be a string, got {}", PRIM, a.type_name(msg_name_val)));
        }
    };

    let struct_val = unsafe { a.arg(args, nargs, 2) };
    if !a.check_struct(struct_val) {
        return a.err("type-error", &format!("{}: expected struct, got {}", PRIM, a.type_name(struct_val)));
    }

    let msg_desc = match pool.get_message_by_name(&msg_name) {
        Some(d) => d,
        None => {
            return a.err("protobuf-error", &format!("{}: message '{}' not found in pool", PRIM, msg_name));
        }
    };

    match encode_message(struct_val, &msg_desc) {
        Ok(dyn_msg) => {
            use prost::Message;
            let encoded = dyn_msg.encode_to_vec();
            a.ok(a.bytes(&encoded))
        }
        Err(e) => a.err("protobuf-error", &format!("{}: {}", PRIM, e)),
    }
}

pub fn decode(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    const PRIM: &str = "protobuf/decode";

    let pool = match crate::schema::get_pool(unsafe { a.arg(args, nargs, 0) }, PRIM) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let msg_name_val = unsafe { a.arg(args, nargs, 1) };
    let msg_name = match a.get_string(msg_name_val) {
        Some(s) => s.to_string(),
        None => {
            return a.err("type-error", &format!("{}: message name must be a string, got {}", PRIM, a.type_name(msg_name_val)));
        }
    };

    let bytes_val = unsafe { a.arg(args, nargs, 2) };
    let bytes = match crate::schema::extract_bytes(bytes_val, PRIM) {
        Ok(b) => b,
        Err(e) => return e,
    };

    let msg_desc = match pool.get_message_by_name(&msg_name) {
        Some(d) => d,
        None => {
            return a.err("protobuf-error", &format!("{}: message '{}' not found in pool", PRIM, msg_name));
        }
    };

    match DynamicMessage::decode(msg_desc, bytes.as_slice()) {
        Ok(dyn_msg) => match decode_message(&dyn_msg) {
            Ok(struct_val) => a.ok(struct_val),
            Err(e) => a.err("protobuf-error", &format!("{}: {}", PRIM, e)),
        },
        Err(e) => a.err("protobuf-error", &format!("{}: {}", PRIM, e)),
    }
}
