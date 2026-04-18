//! Calendar helpers, timezone ops, and series generation.

use crate::{as_jiff, jiff_err, jiff_val, require_int, require_jiff, require_keyword, require_string, require_variant, JiffValue};
use elle_plugin::{ElleResult, ElleValue};

fn extract_date(v: ElleValue, fn_name: &str) -> Result<jiff::civil::Date, ElleResult> {
    let a = crate::api();
    match as_jiff(v) {
        Some(JiffValue::Date(d)) => Ok(*d),
        Some(JiffValue::DateTime(dt)) => Ok(dt.date()),
        Some(JiffValue::Zoned(z)) => Ok(z.date()),
        Some(other) => Err(a.err("type-error", &format!("{}: expected date/datetime/zoned, got {}", fn_name, other.type_name()))),
        None => Err(a.err("type-error", &format!("{}: expected date/datetime/zoned, got {}", fn_name, a.type_name(v)))),
    }
}

pub extern "C" fn prim_date_start_of_month(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match extract_date(unsafe { a.arg(args, nargs, 0) }, "date/start-of-month") { Ok(d) => d, Err(e) => return e };
    a.ok(jiff_val(JiffValue::Date(d.first_of_month())))
}
pub extern "C" fn prim_date_end_of_month(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match extract_date(unsafe { a.arg(args, nargs, 0) }, "date/end-of-month") { Ok(d) => d, Err(e) => return e };
    a.ok(jiff_val(JiffValue::Date(d.last_of_month())))
}
pub extern "C" fn prim_date_start_of_year(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match extract_date(unsafe { a.arg(args, nargs, 0) }, "date/start-of-year") { Ok(d) => d, Err(e) => return e };
    match jiff::civil::Date::new(d.year(), 1, 1) { Ok(d) => a.ok(jiff_val(JiffValue::Date(d))), Err(e) => jiff_err("date/start-of-year", e) }
}
pub extern "C" fn prim_date_end_of_year(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match extract_date(unsafe { a.arg(args, nargs, 0) }, "date/end-of-year") { Ok(d) => d, Err(e) => return e };
    match jiff::civil::Date::new(d.year(), 12, 31) { Ok(d) => a.ok(jiff_val(JiffValue::Date(d))), Err(e) => jiff_err("date/end-of-year", e) }
}

fn parse_weekday(s: &str) -> Option<jiff::civil::Weekday> {
    match s {
        "monday" => Some(jiff::civil::Weekday::Monday), "tuesday" => Some(jiff::civil::Weekday::Tuesday),
        "wednesday" => Some(jiff::civil::Weekday::Wednesday), "thursday" => Some(jiff::civil::Weekday::Thursday),
        "friday" => Some(jiff::civil::Weekday::Friday), "saturday" => Some(jiff::civil::Weekday::Saturday),
        "sunday" => Some(jiff::civil::Weekday::Sunday), _ => None,
    }
}

pub extern "C" fn prim_date_next_weekday(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match extract_date(unsafe { a.arg(args, nargs, 0) }, "date/next-weekday") { Ok(d) => d, Err(e) => return e };
    let kw = match require_keyword(unsafe { a.arg(args, nargs, 1) }, "date/next-weekday") { Ok(k) => k, Err(e) => return e };
    let wd = match parse_weekday(&kw) { Some(wd) => wd, None => return a.err("jiff-error", &format!("date/next-weekday: unknown weekday {:?}", kw)) };
    match d.nth_weekday(1, wd) { Ok(d) => a.ok(jiff_val(JiffValue::Date(d))), Err(e) => jiff_err("date/next-weekday", e) }
}
pub extern "C" fn prim_date_prev_weekday(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match extract_date(unsafe { a.arg(args, nargs, 0) }, "date/prev-weekday") { Ok(d) => d, Err(e) => return e };
    let kw = match require_keyword(unsafe { a.arg(args, nargs, 1) }, "date/prev-weekday") { Ok(k) => k, Err(e) => return e };
    let wd = match parse_weekday(&kw) { Some(wd) => wd, None => return a.err("jiff-error", &format!("date/prev-weekday: unknown weekday {:?}", kw)) };
    match d.nth_weekday(-1, wd) { Ok(d) => a.ok(jiff_val(JiffValue::Date(d))), Err(e) => jiff_err("date/prev-weekday", e) }
}

pub extern "C" fn prim_tz_list(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = crate::api();
    let names: Vec<ElleValue> = jiff::tz::db().available().map(|name| a.string(name.as_str())).collect();
    a.ok(a.array(&names))
}
pub extern "C" fn prim_tz_valid(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let name = match require_string(unsafe { a.arg(args, nargs, 0) }, "tz-valid?") { Ok(s) => s, Err(e) => return e };
    a.ok(a.boolean(jiff::tz::TimeZone::get(&name).is_ok()))
}
pub extern "C" fn prim_tz_system(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = crate::api();
    match jiff::tz::TimeZone::system().iana_name() {
        Some(name) => a.ok(a.string(name)),
        None => a.err("jiff-error", "tz-system: could not determine system timezone"),
    }
}
pub extern "C" fn prim_tz_fixed(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let offset_secs = match require_int(unsafe { a.arg(args, nargs, 0) }, "tz-fixed") { Ok(n) => n as i32, Err(e) => return e };
    let offset = match jiff::tz::Offset::from_seconds(offset_secs) { Ok(o) => o, Err(e) => return jiff_err("tz-fixed", e) };
    let s = offset.to_string();
    a.ok(a.string(&s))
}

pub extern "C" fn prim_temporal_series(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let start = match require_jiff(unsafe { a.arg(args, nargs, 0) }, "temporal/series") { Ok(jv) => jv.clone(), Err(e) => return e };
    let step = match require_variant!(unsafe { a.arg(args, nargs, 1) }, Span, "temporal/series", "span") { Ok(s) => *s, Err(e) => return e };
    let count = match require_int(unsafe { a.arg(args, nargs, 2) }, "temporal/series") { Ok(n) => n as usize, Err(e) => return e };
    let mut results = Vec::with_capacity(count);
    let mut current = start;
    for i in 0..count {
        results.push(jiff_val(current.clone()));
        if i + 1 < count {
            let next = match &current {
                JiffValue::Date(d) => d.checked_add(step).map(JiffValue::Date),
                JiffValue::Time(t) => t.checked_add(step).map(JiffValue::Time),
                JiffValue::DateTime(dt) => dt.checked_add(step).map(JiffValue::DateTime),
                JiffValue::Timestamp(ts) => ts.checked_add(step).map(JiffValue::Timestamp),
                JiffValue::Zoned(z) => z.as_ref().checked_add(step).map(|z| JiffValue::Zoned(Box::new(z))),
                _ => return a.err("type-error", &format!("temporal/series: cannot iterate over {}", current.type_name())),
            };
            match next { Ok(n) => current = n, Err(e) => return jiff_err("temporal/series", e) }
        }
    }
    a.ok(a.array(&results))
}
