//! Component accessors: date/year, date/month, date/day, time/hour, etc.

use crate::{as_jiff, require_variant, JiffValue};
use elle_plugin::{ElleResult, ElleValue};

// ---------------------------------------------------------------------------
// Date component helpers (work on Date, DateTime, Zoned)
// ---------------------------------------------------------------------------

macro_rules! date_accessor {
    ($fn_name:ident, $prim_name:expr, $method:ident) => {
        pub extern "C" fn $fn_name(args: *const ElleValue, nargs: usize) -> ElleResult {
            let a = crate::api();
            let val = a.arg(args, nargs, 0);
            match as_jiff(val) {
                Some(JiffValue::Date(d)) => a.ok(a.int(d.$method() as i64)),
                Some(JiffValue::DateTime(dt)) => a.ok(a.int(dt.$method() as i64)),
                Some(JiffValue::Zoned(z)) => a.ok(a.int(z.$method() as i64)),
                Some(other) => a.err("type-error", &format!("{}: expected date, datetime, or zoned, got {}", $prim_name, other.type_name())),
                None => a.err("type-error", &format!("{}: expected date, datetime, or zoned, got {}", $prim_name, a.type_name(val))),
            }
        }
    };
}

date_accessor!(prim_date_year, "date/year", year);
date_accessor!(prim_date_month, "date/month", month);
date_accessor!(prim_date_day, "date/day", day);

pub extern "C" fn prim_date_weekday(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let val = a.arg(args, nargs, 0);
    let wd = match as_jiff(val) {
        Some(JiffValue::Date(d)) => d.weekday(),
        Some(JiffValue::DateTime(dt)) => dt.weekday(),
        Some(JiffValue::Zoned(z)) => z.weekday(),
        Some(other) => return a.err("type-error", &format!("date/weekday: expected date, datetime, or zoned, got {}", other.type_name())),
        None => return a.err("type-error", &format!("date/weekday: expected date, datetime, or zoned, got {}", a.type_name(val))),
    };
    let name = match wd {
        jiff::civil::Weekday::Monday => "monday",
        jiff::civil::Weekday::Tuesday => "tuesday",
        jiff::civil::Weekday::Wednesday => "wednesday",
        jiff::civil::Weekday::Thursday => "thursday",
        jiff::civil::Weekday::Friday => "friday",
        jiff::civil::Weekday::Saturday => "saturday",
        jiff::civil::Weekday::Sunday => "sunday",
    };
    a.ok(a.keyword(name))
}

pub extern "C" fn prim_date_weekday_number(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let val = a.arg(args, nargs, 0);
    let wd = match as_jiff(val) {
        Some(JiffValue::Date(d)) => d.weekday(),
        Some(JiffValue::DateTime(dt)) => dt.weekday(),
        Some(JiffValue::Zoned(z)) => z.weekday(),
        Some(other) => return a.err("type-error", &format!("date/weekday-number: expected date, datetime, or zoned, got {}", other.type_name())),
        None => return a.err("type-error", &format!("date/weekday-number: expected date, datetime, or zoned, got {}", a.type_name(val))),
    };
    a.ok(a.int(wd.to_monday_one_offset() as i64))
}

macro_rules! date_method_accessor {
    ($fn_name:ident, $prim_name:expr, $method:ident) => {
        pub extern "C" fn $fn_name(args: *const ElleValue, nargs: usize) -> ElleResult {
            let a = crate::api();
            let val = a.arg(args, nargs, 0);
            let n = match as_jiff(val) {
                Some(JiffValue::Date(d)) => d.$method(),
                Some(JiffValue::DateTime(dt)) => dt.date().$method(),
                Some(JiffValue::Zoned(z)) => z.date().$method(),
                Some(other) => return a.err("type-error", &format!("{}: expected date, datetime, or zoned, got {}", $prim_name, other.type_name())),
                None => return a.err("type-error", &format!("{}: expected date, datetime, or zoned, got {}", $prim_name, a.type_name(val))),
            };
            a.ok(a.int(n as i64))
        }
    };
}

date_method_accessor!(prim_date_day_of_year, "date/day-of-year", day_of_year);
date_method_accessor!(prim_date_days_in_month, "date/days-in-month", days_in_month);
date_method_accessor!(prim_date_days_in_year, "date/days-in-year", days_in_year);

pub extern "C" fn prim_date_leap_year(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let val = a.arg(args, nargs, 0);
    let b = match as_jiff(val) {
        Some(JiffValue::Date(d)) => d.in_leap_year(),
        Some(JiffValue::DateTime(dt)) => dt.date().in_leap_year(),
        Some(JiffValue::Zoned(z)) => z.date().in_leap_year(),
        Some(other) => return a.err("type-error", &format!("date/leap-year?: expected date, datetime, or zoned, got {}", other.type_name())),
        None => return a.err("type-error", &format!("date/leap-year?: expected date, datetime, or zoned, got {}", a.type_name(val))),
    };
    a.ok(a.boolean(b))
}

// ---------------------------------------------------------------------------
// Time component helpers (work on Time, DateTime, Zoned)
// ---------------------------------------------------------------------------

macro_rules! time_accessor {
    ($fn_name:ident, $prim_name:expr, $method:ident) => {
        pub extern "C" fn $fn_name(args: *const ElleValue, nargs: usize) -> ElleResult {
            let a = crate::api();
            let val = a.arg(args, nargs, 0);
            match as_jiff(val) {
                Some(JiffValue::Time(t)) => a.ok(a.int(t.$method() as i64)),
                Some(JiffValue::DateTime(dt)) => a.ok(a.int(dt.$method() as i64)),
                Some(JiffValue::Zoned(z)) => a.ok(a.int(z.$method() as i64)),
                Some(other) => a.err("type-error", &format!("{}: expected time, datetime, or zoned, got {}", $prim_name, other.type_name())),
                None => a.err("type-error", &format!("{}: expected time, datetime, or zoned, got {}", $prim_name, a.type_name(val))),
            }
        }
    };
}

time_accessor!(prim_time_hour, "time/hour", hour);
time_accessor!(prim_time_minute, "time/minute", minute);
time_accessor!(prim_time_second, "time/second", second);
time_accessor!(prim_time_millisecond, "time/millisecond", millisecond);
time_accessor!(prim_time_microsecond, "time/microsecond", microsecond);
time_accessor!(prim_time_nanosecond, "time/nanosecond", nanosecond);
time_accessor!(prim_time_subsec_nanosecond, "time/subsec-nanosecond", subsec_nanosecond);

// ---------------------------------------------------------------------------
// Zoned-specific accessors
// ---------------------------------------------------------------------------

pub extern "C" fn prim_zoned_tz_name(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let z = match require_variant!(a.arg(args, nargs, 0), Zoned, "zoned/tz-name", "zoned") {
        Ok(z) => z,
        Err(e) => return e,
    };
    a.ok(a.string(z.time_zone().iana_name().unwrap_or("unknown")))
}

pub extern "C" fn prim_zoned_utc_offset(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let z = match require_variant!(a.arg(args, nargs, 0), Zoned, "zoned/utc-offset", "zoned") {
        Ok(z) => z,
        Err(e) => return e,
    };
    a.ok(a.int(z.offset().seconds() as i64))
}

// ---------------------------------------------------------------------------
// SignedDuration accessors
// ---------------------------------------------------------------------------

pub extern "C" fn prim_sd_secs(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match require_variant!(a.arg(args, nargs, 0), SignedDuration, "signed-duration/secs", "signed-duration") {
        Ok(d) => d,
        Err(e) => return e,
    };
    a.ok(a.int(d.as_secs()))
}

pub extern "C" fn prim_sd_nanos(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match require_variant!(a.arg(args, nargs, 0), SignedDuration, "signed-duration/nanos", "signed-duration") {
        Ok(d) => d,
        Err(e) => return e,
    };
    a.ok(a.int(d.subsec_nanos() as i64))
}

pub extern "C" fn prim_sd_zero(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let d = match require_variant!(a.arg(args, nargs, 0), SignedDuration, "signed-duration/zero?", "signed-duration") {
        Ok(d) => d,
        Err(e) => return e,
    };
    a.ok(a.boolean(d.is_zero()))
}

// ---------------------------------------------------------------------------
// Span accessors
// ---------------------------------------------------------------------------

pub extern "C" fn prim_span_get(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(a.arg(args, nargs, 0), Span, "span/get", "span") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let unit = match crate::require_keyword(a.arg(args, nargs, 1), "span/get") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let n = match unit.as_str() {
        "years" => s.get_years() as i64,
        "months" => s.get_months() as i64,
        "weeks" => s.get_weeks() as i64,
        "days" => s.get_days() as i64,
        "hours" => s.get_hours() as i64,
        "minutes" => s.get_minutes() as i64,
        "seconds" => s.get_seconds() as i64,
        "milliseconds" => s.get_milliseconds() as i64,
        "microseconds" => s.get_microseconds() as i64,
        "nanoseconds" => s.get_nanoseconds() as i64,
        other => return a.err("jiff-error", &format!("span/get: unknown unit {:?}", other)),
    };
    a.ok(a.int(n))
}

pub extern "C" fn prim_span_zero(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(a.arg(args, nargs, 0), Span, "span/zero?", "span") {
        Ok(s) => s,
        Err(e) => return e,
    };
    a.ok(a.boolean(s.is_zero()))
}

pub extern "C" fn prim_span_to_struct(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    let s = match require_variant!(a.arg(args, nargs, 0), Span, "span->struct", "span") {
        Ok(s) => s,
        Err(e) => return e,
    };
    a.ok(a.build_struct(&[
        ("years", a.int(s.get_years() as i64)),
        ("months", a.int(s.get_months() as i64)),
        ("weeks", a.int(s.get_weeks() as i64)),
        ("days", a.int(s.get_days() as i64)),
        ("hours", a.int(s.get_hours() as i64)),
        ("minutes", a.int(s.get_minutes() as i64)),
        ("seconds", a.int(s.get_seconds() as i64)),
        ("milliseconds", a.int(s.get_milliseconds() as i64)),
        ("microseconds", a.int(s.get_microseconds() as i64)),
        ("nanoseconds", a.int(s.get_nanoseconds() as i64)),
    ]))
}
