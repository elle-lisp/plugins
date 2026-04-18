//! Type conversions: between temporal types, zoned/in-tz, span->signed-duration.
//! Rounding: temporal/round.

use crate::{as_jiff, jiff_err, jiff_val, require_jiff, require_string, require_variant, struct_get_kw, JiffValue};
use elle_plugin::{ElleResult, ElleValue};

pub extern "C" fn prim_to_date(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let val = unsafe { a.arg(args, nargs, 0) };
    match as_jiff(val) {
        Some(JiffValue::Date(d)) => a.ok(jiff_val(JiffValue::Date(*d))),
        Some(JiffValue::DateTime(dt)) => a.ok(jiff_val(JiffValue::Date(dt.date()))),
        Some(JiffValue::Zoned(z)) => a.ok(jiff_val(JiffValue::Date(z.date()))),
        Some(other) => a.err("type-error", &format!("date/->date: expected date/datetime/zoned, got {}", other.type_name())),
        None => a.err("type-error", &format!("date/->date: expected temporal value, got {}", a.type_name(val))),
    }
}

pub extern "C" fn prim_to_time(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let val = unsafe { a.arg(args, nargs, 0) };
    match as_jiff(val) {
        Some(JiffValue::Time(t)) => a.ok(jiff_val(JiffValue::Time(*t))),
        Some(JiffValue::DateTime(dt)) => a.ok(jiff_val(JiffValue::Time(dt.time()))),
        Some(JiffValue::Zoned(z)) => a.ok(jiff_val(JiffValue::Time(z.time()))),
        Some(other) => a.err("type-error", &format!("time/->time: expected time/datetime/zoned, got {}", other.type_name())),
        None => a.err("type-error", &format!("time/->time: expected temporal value, got {}", a.type_name(val))),
    }
}

pub extern "C" fn prim_to_datetime(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    if nargs == 2 {
        let d = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Date, "datetime/->datetime", "date") { Ok(d) => *d, Err(e) => return e };
        let t = match require_variant!(unsafe { a.arg(args, nargs, 1) }, Time, "datetime/->datetime", "time") { Ok(t) => *t, Err(e) => return e };
        return a.ok(jiff_val(JiffValue::DateTime(d.to_datetime(t))));
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    match as_jiff(val) {
        Some(JiffValue::DateTime(dt)) => a.ok(jiff_val(JiffValue::DateTime(*dt))),
        Some(JiffValue::Zoned(z)) => a.ok(jiff_val(JiffValue::DateTime(z.datetime()))),
        Some(JiffValue::Date(d)) => a.ok(jiff_val(JiffValue::DateTime(d.to_datetime(jiff::civil::Time::midnight())))),
        Some(other) => a.err("type-error", &format!("datetime/->datetime: expected datetime/zoned/date, got {}", other.type_name())),
        None => a.err("type-error", &format!("datetime/->datetime: expected temporal value, got {}", a.type_name(val))),
    }
}

pub extern "C" fn prim_to_timestamp(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let val = unsafe { a.arg(args, nargs, 0) };
    match as_jiff(val) {
        Some(JiffValue::Timestamp(ts)) => a.ok(jiff_val(JiffValue::Timestamp(*ts))),
        Some(JiffValue::Zoned(z)) => a.ok(jiff_val(JiffValue::Timestamp(z.timestamp()))),
        Some(other) => a.err("type-error", &format!("timestamp/->timestamp: expected timestamp or zoned, got {}", other.type_name())),
        None => a.err("type-error", &format!("timestamp/->timestamp: expected temporal value, got {}", a.type_name(val))),
    }
}

pub extern "C" fn prim_zoned_in_tz(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let z = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Zoned, "zoned/in-tz", "zoned") { Ok(z) => z, Err(e) => return e };
    let tz_str = match require_string(unsafe { a.arg(args, nargs, 1) }, "zoned/in-tz") { Ok(s) => s, Err(e) => return e };
    let tz = match jiff::tz::TimeZone::get(&tz_str) { Ok(tz) => tz, Err(e) => return jiff_err("zoned/in-tz", e) };
    let result = z.with_time_zone(tz);
    a.ok(jiff_val(JiffValue::Zoned(Box::new(result))))
}

pub extern "C" fn prim_span_to_sd(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Span, "span/->signed-duration", "span") { Ok(s) => *s, Err(e) => return e };
    match jiff::SignedDuration::try_from(s) {
        Ok(d) => a.ok(jiff_val(JiffValue::SignedDuration(d))),
        Err(e) => jiff_err("span/->signed-duration", e),
    }
}

fn parse_unit(s: &str) -> Option<jiff::Unit> {
    match s {
        "year" | "years" => Some(jiff::Unit::Year), "month" | "months" => Some(jiff::Unit::Month),
        "week" | "weeks" => Some(jiff::Unit::Week), "day" | "days" => Some(jiff::Unit::Day),
        "hour" | "hours" => Some(jiff::Unit::Hour), "minute" | "minutes" => Some(jiff::Unit::Minute),
        "second" | "seconds" => Some(jiff::Unit::Second), "millisecond" | "milliseconds" => Some(jiff::Unit::Millisecond),
        "microsecond" | "microseconds" => Some(jiff::Unit::Microsecond), "nanosecond" | "nanoseconds" => Some(jiff::Unit::Nanosecond),
        _ => None,
    }
}

pub extern "C" fn prim_temporal_round(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let jv = match require_jiff(unsafe { a.arg(args, nargs, 0) }, "temporal/round") { Ok(jv) => jv.clone(), Err(e) => return e };
    let opts = unsafe { a.arg(args, nargs, 1) };
    let unit_val = match struct_get_kw(opts, "unit") {
        Some(v) => v,
        None => return a.err("jiff-error", "temporal/round: opts must contain :unit keyword"),
    };
    let unit_name = match a.get_keyword_name(unit_val) {
        Some(k) => k.to_string(),
        None => return a.err("type-error", "temporal/round: :unit must be a keyword"),
    };
    let unit = match parse_unit(&unit_name) {
        Some(u) => u,
        None => return a.err("jiff-error", &format!("temporal/round: unknown unit {:?}", unit_name)),
    };
    match jv {
        JiffValue::Timestamp(ts) => match ts.round(unit) { Ok(r) => a.ok(jiff_val(JiffValue::Timestamp(r))), Err(e) => jiff_err("temporal/round", e) },
        JiffValue::Time(t) => match t.round(unit) { Ok(r) => a.ok(jiff_val(JiffValue::Time(r))), Err(e) => jiff_err("temporal/round", e) },
        JiffValue::DateTime(dt) => match dt.round(unit) { Ok(r) => a.ok(jiff_val(JiffValue::DateTime(r))), Err(e) => jiff_err("temporal/round", e) },
        JiffValue::Zoned(z) => match z.round(unit) { Ok(r) => a.ok(jiff_val(JiffValue::Zoned(Box::new(r)))), Err(e) => jiff_err("temporal/round", e) },
        other => a.err("type-error", &format!("temporal/round: cannot round {}", other.type_name())),
    }
}
