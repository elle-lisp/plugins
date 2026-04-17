//! Formatting and epoch conversions.

use crate::{jiff_err, jiff_val, require_int, require_jiff, require_string, require_variant, JiffValue};
use elle_plugin::{ElleResult, ElleValue};

pub extern "C" fn prim_temporal_string(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let jv = match require_jiff(a.arg(args, nargs, 0), "temporal/string") { Ok(jv) => jv, Err(e) => return e };
    let s: String = match jv {
        JiffValue::Timestamp(ts) => ts.to_string(),
        JiffValue::Date(d) => d.to_string(),
        JiffValue::Time(t) => t.to_string(),
        JiffValue::DateTime(dt) => dt.to_string(),
        JiffValue::Zoned(z) => z.to_string(),
        JiffValue::Span(s) => s.to_string(),
        JiffValue::SignedDuration(d) => d.to_string(),
    };
    a.ok(a.string(&s))
}

pub extern "C" fn prim_temporal_format(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let fmt = match require_string(a.arg(args, nargs, 0), "temporal/format") { Ok(s) => s, Err(e) => return e };
    let jv = match require_jiff(a.arg(args, nargs, 1), "temporal/format") { Ok(jv) => jv, Err(e) => return e };
    let result = match jv {
        JiffValue::Timestamp(ts) => jiff::fmt::strtime::format(&fmt, *ts),
        JiffValue::Date(d) => jiff::fmt::strtime::format(&fmt, *d),
        JiffValue::Time(t) => jiff::fmt::strtime::format(&fmt, *t),
        JiffValue::DateTime(dt) => jiff::fmt::strtime::format(&fmt, *dt),
        JiffValue::Zoned(z) => jiff::fmt::strtime::format(&fmt, z.as_ref()),
        _ => return a.err("type-error", &format!("temporal/format: cannot format {}", jv.type_name())),
    };
    match result {
        Ok(s) => a.ok(a.string(&s)),
        Err(e) => jiff_err("temporal/format", e),
    }
}

pub extern "C" fn prim_ts_epoch(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ts = match require_variant!(a.arg(args, nargs, 0), Timestamp, "timestamp/->epoch", "timestamp") { Ok(ts) => ts, Err(e) => return e };
    a.ok(a.float(ts.as_second() as f64 + ts.subsec_nanosecond() as f64 / 1e9))
}
pub extern "C" fn prim_ts_epoch_millis(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ts = match require_variant!(a.arg(args, nargs, 0), Timestamp, "timestamp/->epoch-millis", "timestamp") { Ok(ts) => ts, Err(e) => return e };
    a.ok(a.int(ts.as_millisecond()))
}
pub extern "C" fn prim_ts_epoch_micros(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ts = match require_variant!(a.arg(args, nargs, 0), Timestamp, "timestamp/->epoch-micros", "timestamp") { Ok(ts) => ts, Err(e) => return e };
    a.ok(a.int(ts.as_microsecond()))
}
pub extern "C" fn prim_ts_epoch_nanos(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ts = match require_variant!(a.arg(args, nargs, 0), Timestamp, "timestamp/->epoch-nanos", "timestamp") { Ok(ts) => ts, Err(e) => return e };
    a.ok(a.int(ts.as_nanosecond() as i64))
}
pub extern "C" fn prim_ts_from_epoch_seconds(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let val = a.arg(args, nargs, 0);
    if let Some(n) = a.get_int(val) {
        match jiff::Timestamp::new(n, 0) {
            Ok(ts) => return a.ok(jiff_val(JiffValue::Timestamp(ts))),
            Err(e) => return jiff_err("timestamp/from-epoch-seconds", e),
        }
    }
    if let Some(f) = a.get_float(val) {
        let secs = f.trunc() as i64;
        let nanos = ((f.fract()) * 1e9) as i32;
        match jiff::Timestamp::new(secs, nanos) {
            Ok(ts) => return a.ok(jiff_val(JiffValue::Timestamp(ts))),
            Err(e) => return jiff_err("timestamp/from-epoch-seconds", e),
        }
    }
    a.err("type-error", &format!("timestamp/from-epoch-seconds: expected number, got {}", a.type_name(val)))
}
pub extern "C" fn prim_ts_from_epoch_millis(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ms = match require_int(a.arg(args, nargs, 0), "timestamp/from-epoch-millis") { Ok(n) => n, Err(e) => return e };
    match jiff::Timestamp::from_millisecond(ms) {
        Ok(ts) => a.ok(jiff_val(JiffValue::Timestamp(ts))),
        Err(e) => jiff_err("timestamp/from-epoch-millis", e),
    }
}
pub extern "C" fn prim_ts_from_epoch_micros(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let us = match require_int(a.arg(args, nargs, 0), "timestamp/from-epoch-micros") { Ok(n) => n, Err(e) => return e };
    match jiff::Timestamp::from_microsecond(us) {
        Ok(ts) => a.ok(jiff_val(JiffValue::Timestamp(ts))),
        Err(e) => jiff_err("timestamp/from-epoch-micros", e),
    }
}
pub extern "C" fn prim_ts_from_epoch_nanos(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ns = match require_int(a.arg(args, nargs, 0), "timestamp/from-epoch-nanos") { Ok(n) => n as i128, Err(e) => return e };
    match jiff::Timestamp::from_nanosecond(ns) {
        Ok(ts) => a.ok(jiff_val(JiffValue::Timestamp(ts))),
        Err(e) => jiff_err("timestamp/from-epoch-nanos", e),
    }
}
