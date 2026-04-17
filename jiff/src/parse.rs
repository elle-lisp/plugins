//! Parsing: date/parse, time/parse, datetime/parse, timestamp/parse, etc.

use crate::{jiff_err, jiff_val, require_string, JiffValue};
use elle_plugin::{ElleResult, ElleValue};

pub extern "C" fn prim_date_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(a.arg(args, nargs, 0), "date/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::civil::Date>() {
        Ok(d) => a.ok(jiff_val(JiffValue::Date(d))),
        Err(e) => jiff_err("date/parse", e),
    }
}
pub extern "C" fn prim_time_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(a.arg(args, nargs, 0), "time/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::civil::Time>() {
        Ok(t) => a.ok(jiff_val(JiffValue::Time(t))),
        Err(e) => jiff_err("time/parse", e),
    }
}
pub extern "C" fn prim_datetime_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(a.arg(args, nargs, 0), "datetime/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::civil::DateTime>() {
        Ok(dt) => a.ok(jiff_val(JiffValue::DateTime(dt))),
        Err(e) => jiff_err("datetime/parse", e),
    }
}
pub extern "C" fn prim_timestamp_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(a.arg(args, nargs, 0), "timestamp/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::Timestamp>() {
        Ok(ts) => a.ok(jiff_val(JiffValue::Timestamp(ts))),
        Err(e) => jiff_err("timestamp/parse", e),
    }
}
pub extern "C" fn prim_zoned_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(a.arg(args, nargs, 0), "zoned/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::Zoned>() {
        Ok(z) => a.ok(jiff_val(JiffValue::Zoned(Box::new(z)))),
        Err(e) => jiff_err("zoned/parse", e),
    }
}
pub extern "C" fn prim_span_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(a.arg(args, nargs, 0), "span/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::Span>() {
        Ok(sp) => a.ok(jiff_val(JiffValue::Span(sp))),
        Err(e) => jiff_err("span/parse", e),
    }
}
pub extern "C" fn prim_signed_duration_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_string(a.arg(args, nargs, 0), "signed-duration/parse") { Ok(s) => s, Err(e) => return e };
    match s.parse::<jiff::SignedDuration>() {
        Ok(d) => a.ok(jiff_val(JiffValue::SignedDuration(d))),
        Err(e) => jiff_err("signed-duration/parse", e),
    }
}
pub extern "C" fn prim_temporal_parse_with(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let fmt = match require_string(a.arg(args, nargs, 0), "temporal/parse-with") { Ok(s) => s, Err(e) => return e };
    let input = match require_string(a.arg(args, nargs, 1), "temporal/parse-with") { Ok(s) => s, Err(e) => return e };
    let parser = jiff::fmt::strtime::BrokenDownTime::parse(&fmt, &input);
    match parser {
        Ok(bdt) => {
            if let Ok(z) = bdt.to_zoned() {
                return a.ok(jiff_val(JiffValue::Zoned(Box::new(z))));
            }
            match bdt.to_datetime() {
                Ok(dt) => a.ok(jiff_val(JiffValue::DateTime(dt))),
                Err(e) => jiff_err("temporal/parse-with", e),
            }
        }
        Err(e) => jiff_err("temporal/parse-with", e),
    }
}
