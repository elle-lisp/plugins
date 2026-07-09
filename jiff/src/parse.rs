//! Parsing: date/parse, time/parse, datetime/parse, timestamp/parse, etc.

use crate::{jiff_err, jiff_val, require_string, JiffValue};
use elle_plugin::{ElleCtx, ElleResult, ElleValue};

pub extern "C" fn prim_date_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "date/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::civil::Date>() {
        Ok(d) => a.ok(jiff_val(ctx, JiffValue::Date(d))),
        Err(e) => jiff_err(ctx, "date/parse", e),
    }
}
pub extern "C" fn prim_time_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "time/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::civil::Time>() {
        Ok(t) => a.ok(jiff_val(ctx, JiffValue::Time(t))),
        Err(e) => jiff_err(ctx, "time/parse", e),
    }
}
pub extern "C" fn prim_datetime_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "datetime/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::civil::DateTime>() {
        Ok(dt) => a.ok(jiff_val(ctx, JiffValue::DateTime(dt))),
        Err(e) => jiff_err(ctx, "datetime/parse", e),
    }
}
pub extern "C" fn prim_timestamp_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "timestamp/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::Timestamp>() {
        Ok(ts) => a.ok(jiff_val(ctx, JiffValue::Timestamp(ts))),
        Err(e) => jiff_err(ctx, "timestamp/parse", e),
    }
}
pub extern "C" fn prim_zoned_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "zoned/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::Zoned>() {
        Ok(z) => a.ok(jiff_val(ctx, JiffValue::Zoned(Box::new(z)))),
        Err(e) => jiff_err(ctx, "zoned/parse", e),
    }
}
pub extern "C" fn prim_span_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "span/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::Span>() {
        Ok(sp) => a.ok(jiff_val(ctx, JiffValue::Span(sp))),
        Err(e) => jiff_err(ctx, "span/parse", e),
    }
}
pub extern "C" fn prim_signed_duration_parse(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "signed-duration/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::SignedDuration>() {
        Ok(d) => a.ok(jiff_val(ctx, JiffValue::SignedDuration(d))),
        Err(e) => jiff_err(ctx, "signed-duration/parse", e),
    }
}
pub extern "C" fn prim_temporal_parse_with(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let fmt = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "temporal/parse-with") { Ok(s) => s, Err(e) => return e };
    let input = match require_string(ctx, unsafe { a.arg(args, nargs, 1) }, "temporal/parse-with") { Ok(s) => s, Err(e) => return e };
    let parser = jiff::fmt::strtime::BrokenDownTime::parse(&fmt, &input);
    match parser {
        Ok(bdt) => {
            if let Ok(z) = bdt.to_zoned() {
                return a.ok(jiff_val(ctx, JiffValue::Zoned(Box::new(z))));
            }
            match bdt.to_datetime() {
                Ok(dt) => a.ok(jiff_val(ctx, JiffValue::DateTime(dt))),
                Err(e) => jiff_err(ctx, "temporal/parse-with", e),
            }
        }
        Err(e) => jiff_err(ctx, "temporal/parse-with", e),
    }
}
