//! Elle MessagePack plugin — binary serialization for Elle values.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR, SIG_OK};
use rmp::decode::{read_marker, RmpRead};
use rmp::Marker;

elle_plugin::define_plugin!("msgpack/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Elle integer range (full-range i64 integers)
// ---------------------------------------------------------------------------

const ELLE_INT_MIN: i64 = i64::MIN;
const ELLE_INT_MAX: i64 = i64::MAX;

fn checked_int(n: i64, prim_name: &str) -> Result<ElleValue, String> {
    let a = api();
    if !(ELLE_INT_MIN..=ELLE_INT_MAX).contains(&n) {
        return Err(format!(
            "msgpack/{}: integer {} out of Elle i64 range [{}, {}]",
            prim_name, n, ELLE_INT_MIN, ELLE_INT_MAX
        ));
    }
    Ok(a.int(n))
}

// ---------------------------------------------------------------------------
// Ext type ID constants
// ---------------------------------------------------------------------------

const EXT_KEYWORD: i8 = 1;
const EXT_SET: i8 = 2;
const _EXT_LIST: i8 = 3;
const _EXT_SYMBOL: i8 = 4;

// ---------------------------------------------------------------------------
// Mode enum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Interop,
    Tagged,
}

// ---------------------------------------------------------------------------
// Encode helpers
// ---------------------------------------------------------------------------

fn encode_value(buf: &mut Vec<u8>, val: ElleValue, mode: Mode) -> Result<(), String> {
    let a = api();
    if a.check_nil(val) {
        rmp::encode::write_nil(buf).unwrap();
    } else if let Some(b) = a.get_bool(val) {
        rmp::encode::write_bool(buf, b).unwrap();
    } else if let Some(n) = a.get_int(val) {
        rmp::encode::write_sint(buf, n).unwrap();
    } else if let Some(f) = a.get_float(val) {
        rmp::encode::write_f64(buf, f).unwrap();
    } else if let Some(name) = a.get_keyword_name(val) {
        match mode {
            Mode::Interop => {
                rmp::encode::write_str(buf, name).unwrap();
            }
            Mode::Tagged => {
                let mut payload = Vec::new();
                rmp::encode::write_str(&mut payload, name).unwrap();
                rmp::encode::write_ext_meta(buf, payload.len() as u32, EXT_KEYWORD).unwrap();
                buf.extend_from_slice(&payload);
            }
        }
    } else if let Some(s) = a.get_string(val) {
        rmp::encode::write_str(buf, s).unwrap();
    } else if let Some(data) = a.get_bytes(val) {
        rmp::encode::write_bin(buf, data).unwrap();
    } else if a.check_array(val) {
        let len = a.get_array_len(val).unwrap_or(0);
        rmp::encode::write_array_len(buf, len as u32).unwrap();
        for i in 0..len {
            let elem = a.get_array_item(val, i);
            encode_value(buf, elem, mode)?;
        }
    } else if a.check_struct(val) {
        // For structs, we need to iterate fields. Use struct_get with known keys
        // is not feasible since we don't know the keys. Instead, we'll use the
        // opaque approach — structs in the new API are limited. We serialize
        // what we can access via the API.
        //
        // NOTE: The stable ABI doesn't expose struct iteration. For now, we
        // can't encode structs in the pure stable ABI. This is a limitation.
        // We'll error on structs for now.
        let prim_name = if mode == Mode::Tagged {
            "encode-tagged"
        } else {
            "encode"
        };
        return Err(format!(
            "msgpack/{}: struct encoding not supported in stable ABI plugin",
            prim_name
        ));
    } else {
        let prim_name = if mode == Mode::Tagged {
            "encode-tagged"
        } else {
            "encode"
        };
        return Err(format!(
            "msgpack/{}: cannot encode {}",
            prim_name,
            a.type_name(val)
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Decode helpers
// ---------------------------------------------------------------------------

fn decode_value(rd: &mut &[u8], mode: Mode) -> Result<ElleValue, String> {
    let prim_name = if mode == Mode::Tagged {
        "decode-tagged"
    } else {
        "decode"
    };
    let marker = read_marker(rd).map_err(|e| format!("msgpack/{}: {}", prim_name, e.0))?;
    decode_value_from_marker(rd, marker, mode, prim_name)
}

fn decode_value_from_marker(
    rd: &mut &[u8],
    marker: Marker,
    mode: Mode,
    prim_name: &str,
) -> Result<ElleValue, String> {
    let a = api();
    match marker {
        Marker::Null => Ok(a.nil()),
        Marker::True => Ok(a.boolean(true)),
        Marker::False => Ok(a.boolean(false)),
        Marker::FixPos(n) => Ok(a.int(n as i64)),
        Marker::FixNeg(n) => Ok(a.int(n as i64)),
        Marker::U8 => {
            let n = rd.read_data_u8().map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.int(n as i64))
        }
        Marker::U16 => {
            let n = rd
                .read_data_u16()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.int(n as i64))
        }
        Marker::U32 => {
            let n = rd
                .read_data_u32()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.int(n as i64))
        }
        Marker::U64 => {
            let n = rd
                .read_data_u64()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            if n > ELLE_INT_MAX as u64 {
                return Err(format!(
                    "msgpack/{}: uint64 value {} out of Elle i64 range",
                    prim_name, n
                ));
            }
            Ok(a.int(n as i64))
        }
        Marker::I8 => {
            let n = rd.read_data_i8().map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.int(n as i64))
        }
        Marker::I16 => {
            let n = rd
                .read_data_i16()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.int(n as i64))
        }
        Marker::I32 => {
            let n = rd
                .read_data_i32()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.int(n as i64))
        }
        Marker::I64 => {
            let n = rd
                .read_data_i64()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            checked_int(n, prim_name)
        }
        Marker::F32 => {
            let f = rd
                .read_data_f32()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.float(f as f64))
        }
        Marker::F64 => {
            let f = rd
                .read_data_f64()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            Ok(a.float(f))
        }
        Marker::FixStr(len) => decode_string(rd, len as u32, prim_name),
        Marker::Str8 => {
            let len = rd.read_data_u8().map_err(|e| fmt_vread_err(prim_name, e))? as u32;
            decode_string(rd, len, prim_name)
        }
        Marker::Str16 => {
            let len = rd
                .read_data_u16()
                .map_err(|e| fmt_vread_err(prim_name, e))? as u32;
            decode_string(rd, len, prim_name)
        }
        Marker::Str32 => {
            let len = rd
                .read_data_u32()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            decode_string(rd, len, prim_name)
        }
        Marker::Bin8 => {
            let len = rd.read_data_u8().map_err(|e| fmt_vread_err(prim_name, e))? as u32;
            decode_bytes(rd, len, prim_name)
        }
        Marker::Bin16 => {
            let len = rd
                .read_data_u16()
                .map_err(|e| fmt_vread_err(prim_name, e))? as u32;
            decode_bytes(rd, len, prim_name)
        }
        Marker::Bin32 => {
            let len = rd
                .read_data_u32()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            decode_bytes(rd, len, prim_name)
        }
        Marker::FixArray(len) => decode_array(rd, len as u32, mode, prim_name),
        Marker::Array16 => {
            let len = rd
                .read_data_u16()
                .map_err(|e| fmt_vread_err(prim_name, e))? as u32;
            decode_array(rd, len, mode, prim_name)
        }
        Marker::Array32 => {
            let len = rd
                .read_data_u32()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            decode_array(rd, len, mode, prim_name)
        }
        Marker::FixMap(len) => decode_map(rd, len as u32, mode, prim_name),
        Marker::Map16 => {
            let len = rd
                .read_data_u16()
                .map_err(|e| fmt_vread_err(prim_name, e))? as u32;
            decode_map(rd, len, mode, prim_name)
        }
        Marker::Map32 => {
            let len = rd
                .read_data_u32()
                .map_err(|e| fmt_vread_err(prim_name, e))?;
            decode_map(rd, len, mode, prim_name)
        }
        m @ (Marker::FixExt1
        | Marker::FixExt2
        | Marker::FixExt4
        | Marker::FixExt8
        | Marker::FixExt16
        | Marker::Ext8
        | Marker::Ext16
        | Marker::Ext32) => decode_ext(rd, m, mode, prim_name),
        Marker::Reserved => Err(format!("msgpack/{}: reserved marker (0xc1)", prim_name)),
    }
}

fn decode_string(rd: &mut &[u8], len: u32, prim_name: &str) -> Result<ElleValue, String> {
    let a = api();
    let mut data = vec![0u8; len as usize];
    rd.read_exact_buf(&mut data)
        .map_err(|e| format!("msgpack/{}: {}", prim_name, e))?;
    let s = std::str::from_utf8(&data)
        .map_err(|_| format!("msgpack/{}: invalid UTF-8 in string", prim_name))?;
    Ok(a.string(s))
}

fn decode_bytes(rd: &mut &[u8], len: u32, prim_name: &str) -> Result<ElleValue, String> {
    let a = api();
    let mut data = vec![0u8; len as usize];
    rd.read_exact_buf(&mut data)
        .map_err(|e| format!("msgpack/{}: {}", prim_name, e))?;
    Ok(a.bytes(&data))
}

fn decode_array(rd: &mut &[u8], len: u32, mode: Mode, _prim_name: &str) -> Result<ElleValue, String> {
    let a = api();
    let mut elements = Vec::with_capacity(len as usize);
    for _ in 0..len {
        elements.push(decode_value(rd, mode)?);
    }
    Ok(a.array(&elements))
}

fn decode_map(rd: &mut &[u8], len: u32, mode: Mode, prim_name: &str) -> Result<ElleValue, String> {
    let a = api();
    let mut fields: Vec<(String, ElleValue)> = Vec::with_capacity(len as usize);
    for _ in 0..len {
        let key_str = decode_map_key_string(rd, mode, prim_name)?;
        let val = decode_value(rd, mode)?;
        fields.push((key_str, val));
    }
    let kvs: Vec<(&str, ElleValue)> = fields.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    Ok(a.build_struct(&kvs))
}

fn decode_map_key_string(rd: &mut &[u8], mode: Mode, prim_name: &str) -> Result<String, String> {
    let marker = read_marker(rd).map_err(|e| format!("msgpack/{}: {}", prim_name, e.0))?;

    // In tagged mode, intercept ext markers: ext(1) means keyword key
    if mode == Mode::Tagged {
        if let m @ (Marker::FixExt1
        | Marker::FixExt2
        | Marker::FixExt4
        | Marker::FixExt8
        | Marker::FixExt16
        | Marker::Ext8
        | Marker::Ext16
        | Marker::Ext32) = marker
        {
            let (typeid, size) = read_ext_type_and_size(rd, m, prim_name)?;
            if typeid == EXT_KEYWORD {
                let mut payload = vec![0u8; size as usize];
                rd.read_exact_buf(&mut payload)
                    .map_err(|e| format!("msgpack/{}: {}", prim_name, e))?;
                let name_val = decode_value(&mut payload.as_slice(), mode)?;
                let a = api();
                if let Some(s) = a.get_string(name_val) {
                    return Ok(s.to_string());
                } else {
                    return Err(format!(
                        "msgpack/{}: ext(1) keyword payload must be a string",
                        prim_name
                    ));
                }
            } else {
                return Err(format!(
                    "msgpack/{}: unsupported map key type: ext({})",
                    prim_name, typeid
                ));
            }
        }
    }

    // For non-ext markers (or interop mode), decode the value and extract string key
    let val = decode_value_from_marker(rd, marker, mode, prim_name)?;
    let a = api();
    if let Some(s) = a.get_string(val) {
        Ok(s.to_string())
    } else if let Some(n) = a.get_int(val) {
        Ok(n.to_string())
    } else if let Some(b) = a.get_bool(val) {
        Ok(b.to_string())
    } else if a.check_nil(val) {
        Ok("nil".to_string())
    } else {
        Err(format!(
            "msgpack/{}: unsupported map key type: {}",
            prim_name,
            a.type_name(val)
        ))
    }
}

fn decode_ext(
    rd: &mut &[u8],
    marker: Marker,
    mode: Mode,
    prim_name: &str,
) -> Result<ElleValue, String> {
    let a = api();
    if mode == Mode::Interop {
        return Err(format!(
            "msgpack/{}: ext types not supported in interop mode",
            prim_name
        ));
    }

    let (typeid, size) = read_ext_type_and_size(rd, marker, prim_name)?;

    let mut payload = vec![0u8; size as usize];
    rd.read_exact_buf(&mut payload)
        .map_err(|e| format!("msgpack/{}: {}", prim_name, e))?;

    match typeid {
        EXT_KEYWORD => {
            let name_val = decode_value(&mut payload.as_slice(), mode)?;
            match a.get_string(name_val) {
                Some(name) => Ok(a.keyword(name)),
                None => Err(format!(
                    "msgpack/{}: ext(1) keyword payload must be a string, got {}",
                    prim_name,
                    a.type_name(name_val)
                )),
            }
        }
        EXT_SET => {
            let arr_val = decode_value(&mut payload.as_slice(), mode)?;
            match a.get_array_len(arr_val) {
                Some(len) => {
                    let mut elems = Vec::with_capacity(len);
                    for i in 0..len {
                        elems.push(a.get_array_item(arr_val, i));
                    }
                    Ok(a.set(&elems))
                }
                None => Err(format!(
                    "msgpack/{}: ext(2) set payload must be an array",
                    prim_name
                )),
            }
        }
        _EXT_LIST => {
            // Lists become arrays in the new API
            let arr_val = decode_value(&mut payload.as_slice(), mode)?;
            Ok(arr_val)
        }
        _EXT_SYMBOL => Err(format!(
            "msgpack/{}: cannot decode symbol (name resolution unavailable in plugins)",
            prim_name
        )),
        other => Err(format!("msgpack/{}: unknown ext type {}", prim_name, other)),
    }
}

fn fmt_vread_err(prim_name: &str, e: rmp::decode::ValueReadError<std::io::Error>) -> String {
    format!("msgpack/{}: {:?}", prim_name, e)
}

fn read_ext_type_and_size(
    rd: &mut &[u8],
    marker: Marker,
    prim_name: &str,
) -> Result<(i8, u32), String> {
    let size = match marker {
        Marker::FixExt1 => 1u32,
        Marker::FixExt2 => 2,
        Marker::FixExt4 => 4,
        Marker::FixExt8 => 8,
        Marker::FixExt16 => 16,
        Marker::Ext8 => rd.read_data_u8().map_err(|e| fmt_vread_err(prim_name, e))? as u32,
        Marker::Ext16 => rd
            .read_data_u16()
            .map_err(|e| fmt_vread_err(prim_name, e))? as u32,
        Marker::Ext32 => rd
            .read_data_u32()
            .map_err(|e| fmt_vread_err(prim_name, e))?,
        _ => unreachable!("read_ext_type_and_size called with non-ext marker"),
    };
    let typeid = rd.read_data_i8().map_err(|e| fmt_vread_err(prim_name, e))?;
    Ok((typeid, size))
}

// ---------------------------------------------------------------------------
// Validate (structural validity check, no Elle value construction)
// ---------------------------------------------------------------------------

fn validate(rd: &mut &[u8]) -> bool {
    validate_value(rd)
}

fn validate_value(rd: &mut &[u8]) -> bool {
    let marker = match read_marker(rd) {
        Ok(m) => m,
        Err(_) => return false,
    };
    validate_from_marker(rd, marker)
}

fn validate_from_marker(rd: &mut &[u8], marker: Marker) -> bool {
    match marker {
        Marker::Null | Marker::True | Marker::False => true,
        Marker::FixPos(_) | Marker::FixNeg(_) => true,
        Marker::U8 | Marker::I8 => skip_bytes(rd, 1),
        Marker::U16 | Marker::I16 => skip_bytes(rd, 2),
        Marker::U32 | Marker::I32 | Marker::F32 => skip_bytes(rd, 4),
        Marker::U64 | Marker::I64 | Marker::F64 => skip_bytes(rd, 8),
        Marker::FixStr(len) => skip_bytes(rd, len as usize),
        Marker::Str8 => {
            let len = match read_u8_raw(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, len)
        }
        Marker::Str16 => {
            let len = match read_u16_be(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, len)
        }
        Marker::Str32 => {
            let len = match read_u32_be(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, len)
        }
        Marker::Bin8 => {
            let len = match read_u8_raw(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, len)
        }
        Marker::Bin16 => {
            let len = match read_u16_be(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, len)
        }
        Marker::Bin32 => {
            let len = match read_u32_be(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, len)
        }
        Marker::FixArray(len) => {
            for _ in 0..len {
                if !validate_value(rd) {
                    return false;
                }
            }
            true
        }
        Marker::Array16 => {
            let len = match read_u16_be(rd) {
                Some(n) => n,
                None => return false,
            };
            for _ in 0..len {
                if !validate_value(rd) {
                    return false;
                }
            }
            true
        }
        Marker::Array32 => {
            let len = match read_u32_be(rd) {
                Some(n) => n,
                None => return false,
            };
            for _ in 0..len {
                if !validate_value(rd) {
                    return false;
                }
            }
            true
        }
        Marker::FixMap(len) => {
            for _ in 0..len {
                if !validate_value(rd) { return false; }
                if !validate_value(rd) { return false; }
            }
            true
        }
        Marker::Map16 => {
            let len = match read_u16_be(rd) {
                Some(n) => n,
                None => return false,
            };
            for _ in 0..len {
                if !validate_value(rd) { return false; }
                if !validate_value(rd) { return false; }
            }
            true
        }
        Marker::Map32 => {
            let len = match read_u32_be(rd) {
                Some(n) => n,
                None => return false,
            };
            for _ in 0..len {
                if !validate_value(rd) { return false; }
                if !validate_value(rd) { return false; }
            }
            true
        }
        Marker::FixExt1 => skip_bytes(rd, 1 + 1),
        Marker::FixExt2 => skip_bytes(rd, 1 + 2),
        Marker::FixExt4 => skip_bytes(rd, 1 + 4),
        Marker::FixExt8 => skip_bytes(rd, 1 + 8),
        Marker::FixExt16 => skip_bytes(rd, 1 + 16),
        Marker::Ext8 => {
            let len = match read_u8_raw(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, 1 + len)
        }
        Marker::Ext16 => {
            let len = match read_u16_be(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, 1 + len)
        }
        Marker::Ext32 => {
            let len = match read_u32_be(rd) {
                Some(n) => n as usize,
                None => return false,
            };
            skip_bytes(rd, 1 + len)
        }
        Marker::Reserved => false,
    }
}

fn skip_bytes(rd: &mut &[u8], n: usize) -> bool {
    if rd.len() < n { return false; }
    *rd = &rd[n..];
    true
}

fn read_u8_raw(rd: &mut &[u8]) -> Option<u8> {
    if rd.is_empty() { return None; }
    let b = rd[0];
    *rd = &rd[1..];
    Some(b)
}

fn read_u16_be(rd: &mut &[u8]) -> Option<u16> {
    if rd.len() < 2 { return None; }
    let n = u16::from_be_bytes([rd[0], rd[1]]);
    *rd = &rd[2..];
    Some(n)
}

fn read_u32_be(rd: &mut &[u8]) -> Option<u32> {
    if rd.len() < 4 { return None; }
    let n = u32::from_be_bytes([rd[0], rd[1], rd[2], rd[3]]);
    *rd = &rd[4..];
    Some(n)
}

// ---------------------------------------------------------------------------
// Primitive wrappers
// ---------------------------------------------------------------------------

extern "C" fn prim_msgpack_encode(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let val = unsafe { a.arg(args, nargs, 0) };
    let mut buf = Vec::new();
    match encode_value(&mut buf, val, Mode::Interop) {
        Ok(()) => a.ok(a.bytes(&buf)),
        Err(msg) => a.err("msgpack-error", &msg),
    }
}

extern "C" fn prim_msgpack_decode(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let val = unsafe { a.arg(args, nargs, 0) };
    let data = match a.get_bytes(val) {
        Some(b) => b.to_vec(),
        None => {
            return a.err(
                "type-error",
                &format!("msgpack/decode: expected bytes, got {}", a.type_name(val)),
            );
        }
    };

    let mut rd = data.as_slice();
    match decode_value(&mut rd, Mode::Interop) {
        Ok(result) => {
            if !rd.is_empty() {
                a.err(
                    "msgpack-error",
                    &format!("msgpack/decode: {} trailing bytes after value", rd.len()),
                )
            } else {
                a.ok(result)
            }
        }
        Err(msg) => a.err("msgpack-error", &msg),
    }
}

extern "C" fn prim_msgpack_valid(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let val = unsafe { a.arg(args, nargs, 0) };
    let data = match a.get_bytes(val) {
        Some(b) => b,
        None => return a.ok(a.boolean(false)),
    };

    if data.is_empty() {
        return a.ok(a.boolean(false));
    }

    let mut rd = data;
    let ok = validate(&mut rd);
    let result = ok && rd.is_empty();
    a.ok(a.boolean(result))
}

extern "C" fn prim_msgpack_encode_tagged(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let val = unsafe { a.arg(args, nargs, 0) };
    let mut buf = Vec::new();
    match encode_value(&mut buf, val, Mode::Tagged) {
        Ok(()) => a.ok(a.bytes(&buf)),
        Err(msg) => a.err("msgpack-error", &msg),
    }
}

extern "C" fn prim_msgpack_decode_tagged(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let val = unsafe { a.arg(args, nargs, 0) };
    let data = match a.get_bytes(val) {
        Some(b) => b.to_vec(),
        None => {
            return a.err(
                "type-error",
                &format!(
                    "msgpack/decode-tagged: expected bytes, got {}",
                    a.type_name(val)
                ),
            );
        }
    };

    let mut rd = data.as_slice();
    match decode_value(&mut rd, Mode::Tagged) {
        Ok(result) => {
            if !rd.is_empty() {
                a.err(
                    "msgpack-error",
                    &format!(
                        "msgpack/decode-tagged: {} trailing bytes after value",
                        rd.len()
                    ),
                )
            } else {
                a.ok(result)
            }
        }
        Err(msg) => a.err("msgpack-error", &msg),
    }
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("msgpack/encode", prim_msgpack_encode, SIG_ERROR, 1, "Encode an Elle value to msgpack bytes (interop mode)", "msgpack", r#"(msgpack/encode {:x 1 :y "hello"})"#),
    EllePrimDef::exact("msgpack/decode", prim_msgpack_decode, SIG_ERROR, 1, "Decode msgpack bytes to an Elle value (interop mode)", "msgpack", r#"(msgpack/decode (msgpack/encode 42))"#),
    EllePrimDef::exact("msgpack/valid?", prim_msgpack_valid, SIG_OK, 1, "Check if bytes are structurally valid msgpack", "msgpack", r#"(msgpack/valid? (msgpack/encode 42))"#),
    EllePrimDef::exact("msgpack/encode-tagged", prim_msgpack_encode_tagged, SIG_ERROR, 1, "Encode an Elle value to msgpack bytes with ext types for keywords, sets, lists", "msgpack", r#"(msgpack/encode-tagged {:x 1 :y (list 2 3)})"#),
    EllePrimDef::exact("msgpack/decode-tagged", prim_msgpack_decode_tagged, SIG_ERROR, 1, "Decode msgpack bytes with Elle ext types", "msgpack", r#"(msgpack/decode-tagged (msgpack/encode-tagged :hello))"#),
];
