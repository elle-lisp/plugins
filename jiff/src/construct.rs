//! Constructors: now, timestamp, date, time, datetime, zoned, span, signed-duration.

use crate::{jiff_err, jiff_val, require_int, require_jiff, require_string, struct_get_int, JiffValue};
use elle_plugin::{ElleCtx, ElleResult, ElleValue};

pub extern "C" fn prim_now(ctx: *mut ElleCtx, _args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = crate::api();
    let z = jiff::Zoned::now();
    a.ok(jiff_val(ctx, JiffValue::Zoned(Box::new(z))))
}

pub extern "C" fn prim_timestamp(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    if nargs == 0 {
        return a.ok(jiff_val(ctx, JiffValue::Timestamp(jiff::Timestamp::now())));
    }
    let secs = match require_int(ctx, unsafe { a.arg(args, nargs, 0) }, "timestamp") {
        Ok(n) => n,
        Err(e) => return e,
    };
    let nanos = if nargs > 1 {
        match require_int(ctx, unsafe { a.arg(args, nargs, 1) }, "timestamp") {
            Ok(n) => n as i32,
            Err(e) => return e,
        }
    } else { 0 };
    match jiff::Timestamp::new(secs, nanos) {
        Ok(ts) => a.ok(jiff_val(ctx, JiffValue::Timestamp(ts))),
        Err(e) => jiff_err(ctx, "timestamp", e),
    }
}

pub extern "C" fn prim_date(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let y = match require_int(ctx, unsafe { a.arg(args, nargs, 0) }, "date") { Ok(n) => n as i16, Err(e) => return e };
    let m = match require_int(ctx, unsafe { a.arg(args, nargs, 1) }, "date") { Ok(n) => n as i8, Err(e) => return e };
    let d = match require_int(ctx, unsafe { a.arg(args, nargs, 2) }, "date") { Ok(n) => n as i8, Err(e) => return e };
    match jiff::civil::Date::new(y, m, d) {
        Ok(date) => a.ok(jiff_val(ctx, JiffValue::Date(date))),
        Err(e) => jiff_err(ctx, "date", e),
    }
}

pub extern "C" fn prim_time(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let h = match require_int(ctx, unsafe { a.arg(args, nargs, 0) }, "time") { Ok(n) => n as i8, Err(e) => return e };
    let m = match require_int(ctx, unsafe { a.arg(args, nargs, 1) }, "time") { Ok(n) => n as i8, Err(e) => return e };
    let s = match require_int(ctx, unsafe { a.arg(args, nargs, 2) }, "time") { Ok(n) => n as i8, Err(e) => return e };
    let ns = if nargs > 3 {
        match require_int(ctx, unsafe { a.arg(args, nargs, 3) }, "time") { Ok(n) => n as i32, Err(e) => return e }
    } else { 0 };
    match jiff::civil::Time::new(h, m, s, ns) {
        Ok(t) => a.ok(jiff_val(ctx, JiffValue::Time(t))),
        Err(e) => jiff_err(ctx, "time", e),
    }
}

pub extern "C" fn prim_datetime(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let y = match require_int(ctx, unsafe { a.arg(args, nargs, 0) }, "datetime") { Ok(n) => n as i16, Err(e) => return e };
    let mo = match require_int(ctx, unsafe { a.arg(args, nargs, 1) }, "datetime") { Ok(n) => n as i8, Err(e) => return e };
    let d = match require_int(ctx, unsafe { a.arg(args, nargs, 2) }, "datetime") { Ok(n) => n as i8, Err(e) => return e };
    let h = match require_int(ctx, unsafe { a.arg(args, nargs, 3) }, "datetime") { Ok(n) => n as i8, Err(e) => return e };
    let min = match require_int(ctx, unsafe { a.arg(args, nargs, 4) }, "datetime") { Ok(n) => n as i8, Err(e) => return e };
    let s = match require_int(ctx, unsafe { a.arg(args, nargs, 5) }, "datetime") { Ok(n) => n as i8, Err(e) => return e };
    match jiff::civil::DateTime::new(y, mo, d, h, min, s, 0) {
        Ok(dt) => a.ok(jiff_val(ctx, JiffValue::DateTime(dt))),
        Err(e) => jiff_err(ctx, "datetime", e),
    }
}

pub extern "C" fn prim_zoned(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let tz_str = match require_string(ctx, unsafe { a.arg(args, nargs, 1) }, "zoned") { Ok(s) => s, Err(e) => return e };
    let tz = match jiff::tz::TimeZone::get(&tz_str) { Ok(tz) => tz, Err(e) => return jiff_err(ctx, "zoned", e) };
    let jv = match require_jiff(ctx, unsafe { a.arg(args, nargs, 0) }, "zoned") { Ok(jv) => jv, Err(e) => return e };
    match jv {
        JiffValue::DateTime(dt) => match dt.to_zoned(tz) {
            Ok(z) => a.ok(jiff_val(ctx, JiffValue::Zoned(Box::new(z)))),
            Err(e) => jiff_err(ctx, "zoned", e),
        },
        JiffValue::Timestamp(ts) => {
            let z = ts.to_zoned(tz);
            a.ok(jiff_val(ctx, JiffValue::Zoned(Box::new(z))))
        }
        JiffValue::Date(d) => {
            let dt = d.to_datetime(jiff::civil::Time::midnight());
            match dt.to_zoned(tz) {
                Ok(z) => a.ok(jiff_val(ctx, JiffValue::Zoned(Box::new(z)))),
                Err(e) => jiff_err(ctx, "zoned", e),
            }
        }
        other => a.err(ctx, "type-error", &format!("zoned: expected datetime, timestamp, or date, got {}", other.type_name())),
    }
}

pub extern "C" fn prim_span(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let opts = unsafe { a.arg(args, nargs, 0) };
    if !a.check_struct(opts) {
        return a.err(ctx, "type-error", &format!("span: expected struct, got {}", a.type_name(opts)));
    }
    let mut s = jiff::Span::new();
    if let Some(n) = struct_get_int(opts, "years") { s = match s.try_years(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "months") { s = match s.try_months(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "weeks") { s = match s.try_weeks(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "days") { s = match s.try_days(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "hours") { s = match s.try_hours(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "minutes") { s = match s.try_minutes(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "seconds") { s = match s.try_seconds(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "milliseconds") { s = match s.try_milliseconds(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "microseconds") { s = match s.try_microseconds(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    if let Some(n) = struct_get_int(opts, "nanoseconds") { s = match s.try_nanoseconds(n) { Ok(s) => s, Err(e) => return jiff_err(ctx, "span", e) }; }
    a.ok(jiff_val(ctx, JiffValue::Span(s)))
}

pub extern "C" fn prim_signed_duration(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let secs = match require_int(ctx, unsafe { a.arg(args, nargs, 0) }, "signed-duration") { Ok(n) => n, Err(e) => return e };
    let nanos = if nargs > 1 {
        match require_int(ctx, unsafe { a.arg(args, nargs, 1) }, "signed-duration") { Ok(n) => n as i32, Err(e) => return e }
    } else { 0 };
    a.ok(jiff_val(ctx, JiffValue::SignedDuration(jiff::SignedDuration::new(secs, nanos))))
}
