//! Arithmetic: add/sub for all temporal types, since/until, span ops.
//! Comparison: temporal/compare, temporal/before?, temporal/after?, temporal/equal?

use crate::{as_jiff, jiff_err, jiff_val, require_int, require_jiff, require_keyword, require_variant, JiffValue};
use elle_plugin::{ElleResult, ElleValue, SIG_OK};
use jiff::Unit;

fn require_span_like(v: ElleValue, fn_name: &str) -> Result<&'static JiffValue, ElleResult> {
    let a = crate::api();
    match as_jiff(v) {
        Some(jv @ JiffValue::Span(_)) | Some(jv @ JiffValue::SignedDuration(_)) => Ok(jv),
        Some(other) => Err(a.err("type-error", &format!("{}: expected span or signed-duration, got {}", fn_name, other.type_name()))),
        None => Err(a.err("type-error", &format!("{}: expected span or signed-duration, got {}", fn_name, a.type_name(v)))),
    }
}

fn parse_unit(s: &str) -> Option<Unit> {
    match s {
        "year" | "years" => Some(Unit::Year), "month" | "months" => Some(Unit::Month),
        "week" | "weeks" => Some(Unit::Week), "day" | "days" => Some(Unit::Day),
        "hour" | "hours" => Some(Unit::Hour), "minute" | "minutes" => Some(Unit::Minute),
        "second" | "seconds" => Some(Unit::Second), "millisecond" | "milliseconds" => Some(Unit::Millisecond),
        "microsecond" | "microseconds" => Some(Unit::Microsecond), "nanosecond" | "nanoseconds" => Some(Unit::Nanosecond),
        _ => None,
    }
}

macro_rules! arith_prim {
    ($fn_name:ident, $prim_name:expr, $variant:ident, $type_name:expr, $op:ident) => {
        pub extern "C" fn $fn_name(args: *const ElleValue, nargs: usize) -> ElleResult {
            let a = crate::api();
            let v = match require_variant!(unsafe { a.arg(args, nargs, 0) }, $variant, $prim_name, $type_name) { Ok(v) => v.clone(), Err(e) => return e };
            let rhs = match require_span_like(unsafe { a.arg(args, nargs, 1) }, $prim_name) { Ok(jv) => jv, Err(e) => return e };
            match rhs {
                JiffValue::Span(s) => match v.$op(*s) { Ok(r) => a.ok(jiff_val(JiffValue::$variant(r))), Err(e) => jiff_err($prim_name, e) },
                JiffValue::SignedDuration(d) => match v.$op(*d) { Ok(r) => a.ok(jiff_val(JiffValue::$variant(r))), Err(e) => jiff_err($prim_name, e) },
                _ => unreachable!(),
            }
        }
    };
}

arith_prim!(prim_date_add, "date/add", Date, "date", checked_add);
arith_prim!(prim_date_sub, "date/sub", Date, "date", checked_sub);
arith_prim!(prim_time_add, "time/add", Time, "time", checked_add);
arith_prim!(prim_time_sub, "time/sub", Time, "time", checked_sub);
arith_prim!(prim_datetime_add, "datetime/add", DateTime, "datetime", checked_add);
arith_prim!(prim_datetime_sub, "datetime/sub", DateTime, "datetime", checked_sub);
arith_prim!(prim_timestamp_add, "timestamp/add", Timestamp, "timestamp", checked_add);
arith_prim!(prim_timestamp_sub, "timestamp/sub", Timestamp, "timestamp", checked_sub);

pub extern "C" fn prim_zoned_add(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let z = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Zoned, "zoned/add", "zoned") { Ok(z) => z, Err(e) => return e };
    let rhs = match require_span_like(unsafe { a.arg(args, nargs, 1) }, "zoned/add") { Ok(jv) => jv, Err(e) => return e };
    match rhs {
        JiffValue::Span(s) => match z.as_ref().checked_add(*s) { Ok(r) => a.ok(jiff_val(JiffValue::Zoned(Box::new(r)))), Err(e) => jiff_err("zoned/add", e) },
        JiffValue::SignedDuration(d) => match z.as_ref().checked_add(*d) { Ok(r) => a.ok(jiff_val(JiffValue::Zoned(Box::new(r)))), Err(e) => jiff_err("zoned/add", e) },
        _ => unreachable!(),
    }
}

pub extern "C" fn prim_zoned_sub(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let z = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Zoned, "zoned/sub", "zoned") { Ok(z) => z, Err(e) => return e };
    let rhs = match require_span_like(unsafe { a.arg(args, nargs, 1) }, "zoned/sub") { Ok(jv) => jv, Err(e) => return e };
    match rhs {
        JiffValue::Span(s) => match z.as_ref().checked_sub(*s) { Ok(r) => a.ok(jiff_val(JiffValue::Zoned(Box::new(r)))), Err(e) => jiff_err("zoned/sub", e) },
        JiffValue::SignedDuration(d) => match z.as_ref().checked_sub(*d) { Ok(r) => a.ok(jiff_val(JiffValue::Zoned(Box::new(r)))), Err(e) => jiff_err("zoned/sub", e) },
        _ => unreachable!(),
    }
}

pub extern "C" fn prim_timestamp_since(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ts_a = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Timestamp, "timestamp/since", "timestamp") { Ok(ts) => *ts, Err(e) => return e };
    let ts_b = match require_variant!(unsafe { a.arg(args, nargs, 1) }, Timestamp, "timestamp/since", "timestamp") { Ok(ts) => *ts, Err(e) => return e };
    if nargs > 2 {
        let unit_kw = match require_keyword(unsafe { a.arg(args, nargs, 2) }, "timestamp/since") { Ok(k) => k, Err(e) => return e };
        let unit = match parse_unit(&unit_kw) { Some(u) => u, None => return a.err("jiff-error", &format!("timestamp/since: unknown unit {:?}", unit_kw)) };
        match ts_a.since((unit, ts_b)) { Ok(s) => a.ok(jiff_val(JiffValue::Span(s))), Err(e) => jiff_err("timestamp/since", e) }
    } else {
        a.ok(jiff_val(JiffValue::SignedDuration(ts_a.duration_since(ts_b))))
    }
}

pub extern "C" fn prim_timestamp_until(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let ts_a = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Timestamp, "timestamp/until", "timestamp") { Ok(ts) => *ts, Err(e) => return e };
    let ts_b = match require_variant!(unsafe { a.arg(args, nargs, 1) }, Timestamp, "timestamp/until", "timestamp") { Ok(ts) => *ts, Err(e) => return e };
    if nargs > 2 {
        let unit_kw = match require_keyword(unsafe { a.arg(args, nargs, 2) }, "timestamp/until") { Ok(k) => k, Err(e) => return e };
        let unit = match parse_unit(&unit_kw) { Some(u) => u, None => return a.err("jiff-error", &format!("timestamp/until: unknown unit {:?}", unit_kw)) };
        match ts_a.until((unit, ts_b)) { Ok(s) => a.ok(jiff_val(JiffValue::Span(s))), Err(e) => jiff_err("timestamp/until", e) }
    } else {
        a.ok(jiff_val(JiffValue::SignedDuration(ts_a.duration_until(ts_b))))
    }
}

pub extern "C" fn prim_zoned_until(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let za = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Zoned, "zoned/until", "zoned") { Ok(z) => z, Err(e) => return e };
    let zb = match require_variant!(unsafe { a.arg(args, nargs, 1) }, Zoned, "zoned/until", "zoned") { Ok(z) => z, Err(e) => return e };
    if nargs > 2 {
        let unit_kw = match require_keyword(unsafe { a.arg(args, nargs, 2) }, "zoned/until") { Ok(k) => k, Err(e) => return e };
        let unit = match parse_unit(&unit_kw) { Some(u) => u, None => return a.err("jiff-error", &format!("zoned/until: unknown unit {:?}", unit_kw)) };
        match za.as_ref().until((unit, zb.as_ref())) { Ok(s) => a.ok(jiff_val(JiffValue::Span(s))), Err(e) => jiff_err("zoned/until", e) }
    } else {
        match za.as_ref().until(zb.as_ref()) { Ok(s) => a.ok(jiff_val(JiffValue::Span(s))), Err(e) => jiff_err("zoned/until", e) }
    }
}

pub extern "C" fn prim_span_add(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let sa = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Span, "span/add", "span") { Ok(s) => *s, Err(e) => return e };
    let sb = match require_variant!(unsafe { a.arg(args, nargs, 1) }, Span, "span/add", "span") { Ok(s) => *s, Err(e) => return e };
    match sa.checked_add(sb) { Ok(s) => a.ok(jiff_val(JiffValue::Span(s))), Err(e) => jiff_err("span/add", e) }
}

pub extern "C" fn prim_span_mul(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Span, "span/mul", "span") { Ok(s) => *s, Err(e) => return e };
    let n = match require_int(unsafe { a.arg(args, nargs, 1) }, "span/mul") { Ok(n) => n, Err(e) => return e };
    match s.checked_mul(n) { Ok(r) => a.ok(jiff_val(JiffValue::Span(r))), Err(e) => jiff_err("span/mul", e) }
}

pub extern "C" fn prim_span_negate(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Span, "span/negate", "span") { Ok(s) => *s, Err(e) => return e };
    a.ok(jiff_val(JiffValue::Span(s.negate())))
}

pub extern "C" fn prim_span_abs(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Span, "span/abs", "span") { Ok(s) => *s, Err(e) => return e };
    a.ok(jiff_val(JiffValue::Span(s.abs())))
}

pub extern "C" fn prim_span_total(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(unsafe { a.arg(args, nargs, 0) }, Span, "span-total", "span") { Ok(s) => *s, Err(e) => return e };
    let unit_kw = match require_keyword(unsafe { a.arg(args, nargs, 1) }, "span-total") { Ok(k) => k, Err(e) => return e };
    let unit = match parse_unit(&unit_kw) { Some(u) => u, None => return a.err("jiff-error", &format!("span-total: unknown unit {:?}", unit_kw)) };
    match s.total(unit) { Ok(f) => a.ok(a.float(f)), Err(e) => jiff_err("span-total", e) }
}

pub extern "C" fn prim_sd_add(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let da = match require_variant!(unsafe { a.arg(args, nargs, 0) }, SignedDuration, "signed-duration/add", "signed-duration") { Ok(d) => *d, Err(e) => return e };
    let db = match require_variant!(unsafe { a.arg(args, nargs, 1) }, SignedDuration, "signed-duration/add", "signed-duration") { Ok(d) => *d, Err(e) => return e };
    match da.checked_add(db) { Some(r) => a.ok(jiff_val(JiffValue::SignedDuration(r))), None => a.err("jiff-error", "signed-duration/add: overflow") }
}

pub extern "C" fn prim_sd_negate(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match require_variant!(unsafe { a.arg(args, nargs, 0) }, SignedDuration, "signed-duration/negate", "signed-duration") { Ok(d) => *d, Err(e) => return e };
    match d.checked_neg() { Some(r) => a.ok(jiff_val(JiffValue::SignedDuration(r))), None => a.err("jiff-error", "signed-duration/negate: overflow") }
}

pub extern "C" fn prim_sd_abs(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match require_variant!(unsafe { a.arg(args, nargs, 0) }, SignedDuration, "signed-duration/abs", "signed-duration") { Ok(d) => *d, Err(e) => return e };
    a.ok(jiff_val(JiffValue::SignedDuration(d.abs())))
}

fn span_fields(s: &jiff::Span) -> [i64; 10] {
    [s.get_years() as i64, s.get_months() as i64, s.get_weeks() as i64, s.get_days() as i64,
     s.get_hours() as i64, s.get_minutes(), s.get_seconds(), s.get_milliseconds(),
     s.get_microseconds(), s.get_nanoseconds()]
}

pub extern "C" fn prim_temporal_compare(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let jv_a = match require_jiff(unsafe { a.arg(args, nargs, 0) }, "temporal/compare") { Ok(jv) => jv, Err(e) => return e };
    let jv_b = match require_jiff(unsafe { a.arg(args, nargs, 1) }, "temporal/compare") { Ok(jv) => jv, Err(e) => return e };
    use std::cmp::Ordering;
    let ord = match (jv_a, jv_b) {
        (JiffValue::Timestamp(x), JiffValue::Timestamp(y)) => x.cmp(y),
        (JiffValue::Date(x), JiffValue::Date(y)) => x.cmp(y),
        (JiffValue::Time(x), JiffValue::Time(y)) => x.cmp(y),
        (JiffValue::DateTime(x), JiffValue::DateTime(y)) => x.cmp(y),
        (JiffValue::Zoned(x), JiffValue::Zoned(y)) => x.timestamp().cmp(&y.timestamp()),
        (JiffValue::SignedDuration(x), JiffValue::SignedDuration(y)) => x.cmp(y),
        (JiffValue::Span(x), JiffValue::Span(y)) => span_fields(x).cmp(&span_fields(y)),
        _ => return a.err("type-error", &format!("temporal/compare: cannot compare {} with {}", jv_a.type_name(), jv_b.type_name())),
    };
    let n = match ord { Ordering::Less => -1, Ordering::Equal => 0, Ordering::Greater => 1 };
    a.ok(a.int(n))
}

pub extern "C" fn prim_temporal_before(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let r = prim_temporal_compare(args, nargs);
    if r.signal != SIG_OK { return r; }
    a.ok(a.boolean(a.get_int(r.value) == Some(-1)))
}

pub extern "C" fn prim_temporal_after(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let r = prim_temporal_compare(args, nargs);
    if r.signal != SIG_OK { return r; }
    a.ok(a.boolean(a.get_int(r.value) == Some(1)))
}

pub extern "C" fn prim_temporal_equal(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let r = prim_temporal_compare(args, nargs);
    if r.signal != SIG_OK { return r; }
    a.ok(a.boolean(a.get_int(r.value) == Some(0)))
}
