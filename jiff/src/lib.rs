//! Elle jiff plugin — date/time support via the `jiff` crate.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR, SIG_OK};

pub mod access;
pub mod arith;
pub mod calendar;
pub mod construct;
pub mod convert;
pub mod format;
pub mod parse;
pub mod predicate;

elle_plugin::define_plugin!("", &PRIMITIVES);

// ---------------------------------------------------------------------------
// JiffValue — the single External data type
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum JiffValue {
    Timestamp(jiff::Timestamp),
    Date(jiff::civil::Date),
    Time(jiff::civil::Time),
    DateTime(jiff::civil::DateTime),
    Zoned(Box<jiff::Zoned>),
    Span(jiff::Span),
    SignedDuration(jiff::SignedDuration),
}

impl JiffValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Timestamp(_) => "timestamp",
            Self::Date(_) => "date",
            Self::Time(_) => "time",
            Self::DateTime(_) => "datetime",
            Self::Zoned(_) => "zoned",
            Self::Span(_) => "span",
            Self::SignedDuration(_) => "signed-duration",
        }
    }
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

pub fn jiff_val(v: JiffValue) -> ElleValue {
    let a = api();
    let name = v.type_name();
    a.external(name, v)
}

pub fn as_jiff(v: ElleValue) -> Option<&'static JiffValue> {
    let a = api();
    // Try each type name variant
    a.get_external::<JiffValue>(v, "timestamp")
        .or_else(|| a.get_external::<JiffValue>(v, "date"))
        .or_else(|| a.get_external::<JiffValue>(v, "time"))
        .or_else(|| a.get_external::<JiffValue>(v, "datetime"))
        .or_else(|| a.get_external::<JiffValue>(v, "zoned"))
        .or_else(|| a.get_external::<JiffValue>(v, "span"))
        .or_else(|| a.get_external::<JiffValue>(v, "signed-duration"))
}

pub fn require_jiff(v: ElleValue, fn_name: &str) -> Result<&'static JiffValue, ElleResult> {
    let a = api();
    as_jiff(v).ok_or_else(|| {
        a.err("type-error", &format!("{}: expected temporal value, got {}", fn_name, a.type_name(v)))
    })
}

macro_rules! require_variant {
    ($val:expr, $variant:ident, $fn_name:expr, $expected:expr) => {
        match crate::as_jiff($val) {
            Some(crate::JiffValue::$variant(inner)) => Ok(inner),
            Some(other) => Err(crate::api().err(
                "type-error",
                &format!("{}: expected {}, got {}", $fn_name, $expected, other.type_name()),
            )),
            None => Err(crate::api().err(
                "type-error",
                &format!("{}: expected {}, got {}", $fn_name, $expected, crate::api().type_name($val)),
            )),
        }
    };
}
pub(crate) use require_variant;

pub fn require_string(v: ElleValue, fn_name: &str) -> Result<String, ElleResult> {
    let a = api();
    a.get_string(v)
        .map(|s| s.to_string())
        .ok_or_else(|| a.err("type-error", &format!("{}: expected string, got {}", fn_name, a.type_name(v))))
}

pub fn require_int(v: ElleValue, fn_name: &str) -> Result<i64, ElleResult> {
    let a = api();
    a.get_int(v)
        .ok_or_else(|| a.err("type-error", &format!("{}: expected int, got {}", fn_name, a.type_name(v))))
}

pub fn require_float(v: ElleValue, fn_name: &str) -> Result<f64, ElleResult> {
    let a = api();
    a.get_float(v)
        .ok_or_else(|| a.err("type-error", &format!("{}: expected float, got {}", fn_name, a.type_name(v))))
}

pub fn require_keyword(v: ElleValue, fn_name: &str) -> Result<String, ElleResult> {
    let a = api();
    a.get_keyword_name(v)
        .map(|s| s.to_string())
        .ok_or_else(|| a.err("type-error", &format!("{}: expected keyword, got {}", fn_name, a.type_name(v))))
}

pub fn jiff_err(fn_name: &str, e: impl std::fmt::Display) -> ElleResult {
    let a = api();
    a.err("jiff-error", &format!("{}: {}", fn_name, e))
}

pub fn struct_get_kw(v: ElleValue, key: &str) -> Option<ElleValue> {
    let a = api();
    let field = a.get_struct_field(v, key);
    if a.check_nil(field) { None } else { Some(field) }
}

pub fn struct_get_int(v: ElleValue, key: &str) -> Option<i64> {
    let a = api();
    struct_get_kw(v, key).and_then(|v| a.get_int(v))
}

// ---------------------------------------------------------------------------
// Collected PRIMITIVES from all sub-modules
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    // construct
    EllePrimDef::exact("now", construct::prim_now, SIG_OK, 0, "Current wall-clock time with system timezone.", "jiff", "(now)"),
    EllePrimDef::range("timestamp", construct::prim_timestamp, SIG_ERROR, 0, 2, "Current UTC instant (no args), or construct from epoch seconds and optional nanoseconds.", "jiff", "(timestamp)"),
    EllePrimDef::exact("date", construct::prim_date, SIG_ERROR, 3, "Construct a calendar date from year, month, day.", "jiff", "(date 2024 6 19)"),
    EllePrimDef::range("time", construct::prim_time, SIG_ERROR, 3, 4, "Construct a wall-clock time from hour, minute, second, and optional nanoseconds.", "jiff", "(time 15 22 45)"),
    EllePrimDef::exact("datetime", construct::prim_datetime, SIG_ERROR, 6, "Construct a datetime from year, month, day, hour, minute, second.", "jiff", "(datetime 2024 6 19 15 22 45)"),
    EllePrimDef::exact("zoned", construct::prim_zoned, SIG_ERROR, 2, "Attach a timezone to a datetime, timestamp, or date.", "jiff", r#"(zoned (datetime 2024 6 19 15 22 45) "America/New_York")"#),
    EllePrimDef::exact("span", construct::prim_span, SIG_ERROR, 1, "Construct a span from a struct of units.", "jiff", "(span {:hours 1 :minutes 30})"),
    EllePrimDef::range("signed-duration", construct::prim_signed_duration, SIG_ERROR, 1, 2, "Construct an exact signed duration from seconds and optional nanoseconds.", "jiff", "(signed-duration 3600)"),
    // predicate
    EllePrimDef::exact("date?", predicate::prim_date_p, SIG_OK, 1, "True if value is a date.", "jiff", "(date? (date 2024 6 19))"),
    EllePrimDef::exact("time?", predicate::prim_time_p, SIG_OK, 1, "True if value is a time.", "jiff", "(time? (time 15 22 45))"),
    EllePrimDef::exact("datetime?", predicate::prim_datetime_p, SIG_OK, 1, "True if value is a datetime.", "jiff", "(datetime? (datetime 2024 6 19 15 22 45))"),
    EllePrimDef::exact("timestamp?", predicate::prim_timestamp_p, SIG_OK, 1, "True if value is a timestamp.", "jiff", "(timestamp? (timestamp))"),
    EllePrimDef::exact("zoned?", predicate::prim_zoned_p, SIG_OK, 1, "True if value is a zoned datetime.", "jiff", r#"(zoned? (now))"#),
    EllePrimDef::exact("span?", predicate::prim_span_p, SIG_OK, 1, "True if value is a span.", "jiff", "(span? (span {:hours 1}))"),
    EllePrimDef::exact("signed-duration?", predicate::prim_signed_duration_p, SIG_OK, 1, "True if value is a signed-duration.", "jiff", "(signed-duration? (signed-duration 3600))"),
    EllePrimDef::exact("temporal?", predicate::prim_temporal_p, SIG_OK, 1, "True if value is any jiff temporal type.", "jiff", "(temporal? (now))"),
    // access
    EllePrimDef::exact("date/year", access::prim_date_year, SIG_ERROR, 1, "Year component. Works on date, datetime, zoned.", "jiff", "(date/year (date 2024 6 19))"),
    EllePrimDef::exact("date/month", access::prim_date_month, SIG_ERROR, 1, "Month component (1-12).", "jiff", "(date/month (date 2024 6 19))"),
    EllePrimDef::exact("date/day", access::prim_date_day, SIG_ERROR, 1, "Day component (1-31).", "jiff", "(date/day (date 2024 6 19))"),
    EllePrimDef::exact("date/weekday", access::prim_date_weekday, SIG_ERROR, 1, "Day of week as keyword.", "jiff", "(date/weekday (date 2024 6 19))"),
    EllePrimDef::exact("date/weekday-number", access::prim_date_weekday_number, SIG_ERROR, 1, "ISO weekday number (1=Monday .. 7=Sunday).", "jiff", "(date/weekday-number (date 2024 6 19))"),
    EllePrimDef::exact("date/day-of-year", access::prim_date_day_of_year, SIG_ERROR, 1, "Day of year (1-366).", "jiff", "(date/day-of-year (date 2024 6 19))"),
    EllePrimDef::exact("date/days-in-month", access::prim_date_days_in_month, SIG_ERROR, 1, "Number of days in the month.", "jiff", "(date/days-in-month (date 2024 2 1))"),
    EllePrimDef::exact("date/days-in-year", access::prim_date_days_in_year, SIG_ERROR, 1, "Number of days in the year.", "jiff", "(date/days-in-year (date 2024 1 1))"),
    EllePrimDef::exact("date/leap-year?", access::prim_date_leap_year, SIG_ERROR, 1, "True if the year is a leap year.", "jiff", "(date/leap-year? (date 2024 1 1))"),
    EllePrimDef::exact("time/hour", access::prim_time_hour, SIG_ERROR, 1, "Hour component (0-23).", "jiff", "(time/hour (time 15 22 45))"),
    EllePrimDef::exact("time/minute", access::prim_time_minute, SIG_ERROR, 1, "Minute component (0-59).", "jiff", "(time/minute (time 15 22 45))"),
    EllePrimDef::exact("time/second", access::prim_time_second, SIG_ERROR, 1, "Second component (0-59).", "jiff", "(time/second (time 15 22 45))"),
    EllePrimDef::exact("time/millisecond", access::prim_time_millisecond, SIG_ERROR, 1, "Millisecond component (0-999).", "jiff", "(time/millisecond (time 15 22 45 123456789))"),
    EllePrimDef::exact("time/microsecond", access::prim_time_microsecond, SIG_ERROR, 1, "Microsecond component (0-999999).", "jiff", "(time/microsecond (time 15 22 45 123456789))"),
    EllePrimDef::exact("time/nanosecond", access::prim_time_nanosecond, SIG_ERROR, 1, "Nanosecond component (0-999999999).", "jiff", "(time/nanosecond (time 15 22 45 123456789))"),
    EllePrimDef::exact("time/subsec-nanosecond", access::prim_time_subsec_nanosecond, SIG_ERROR, 1, "Sub-second nanoseconds.", "jiff", "(time/subsec-nanosecond (time 15 22 45 123456789))"),
    EllePrimDef::exact("zoned/tz-name", access::prim_zoned_tz_name, SIG_ERROR, 1, "IANA timezone name of a zoned datetime.", "jiff", "(zoned/tz-name (now))"),
    EllePrimDef::exact("zoned/utc-offset", access::prim_zoned_utc_offset, SIG_ERROR, 1, "UTC offset in seconds.", "jiff", "(zoned/utc-offset (now))"),
    EllePrimDef::exact("signed-duration/secs", access::prim_sd_secs, SIG_ERROR, 1, "Whole seconds of a signed-duration.", "jiff", "(signed-duration/secs (signed-duration 3661 500000000))"),
    EllePrimDef::exact("signed-duration/nanos", access::prim_sd_nanos, SIG_ERROR, 1, "Sub-second nanoseconds.", "jiff", "(signed-duration/nanos (signed-duration 3661 500000000))"),
    EllePrimDef::exact("signed-duration/zero?", access::prim_sd_zero, SIG_ERROR, 1, "True if the signed-duration is zero.", "jiff", "(signed-duration/zero? (signed-duration 0))"),
    EllePrimDef::exact("span/get", access::prim_span_get, SIG_ERROR, 2, "Get a unit field from a span.", "jiff", "(span/get (span {:hours 1 :minutes 30}) :hours)"),
    EllePrimDef::exact("span/zero?", access::prim_span_zero, SIG_ERROR, 1, "True if all span fields are zero.", "jiff", "(span/zero? (span {:hours 0}))"),
    EllePrimDef::exact("span->struct", access::prim_span_to_struct, SIG_ERROR, 1, "Convert a span to a struct with all 10 unit fields.", "jiff", "(span->struct (span {:hours 1 :minutes 30}))"),
    // parse
    EllePrimDef::exact("date/parse", parse::prim_date_parse, SIG_ERROR, 1, "Parse an ISO 8601 date string.", "jiff", r#"(date/parse "2024-06-19")"#),
    EllePrimDef::exact("time/parse", parse::prim_time_parse, SIG_ERROR, 1, "Parse an ISO 8601 time string.", "jiff", r#"(time/parse "15:22:45")"#),
    EllePrimDef::exact("datetime/parse", parse::prim_datetime_parse, SIG_ERROR, 1, "Parse an ISO 8601 datetime string.", "jiff", r#"(datetime/parse "2024-06-19T15:22:45")"#),
    EllePrimDef::exact("timestamp/parse", parse::prim_timestamp_parse, SIG_ERROR, 1, "Parse an ISO 8601 timestamp string.", "jiff", r#"(timestamp/parse "2024-06-19T19:22:45Z")"#),
    EllePrimDef::exact("zoned/parse", parse::prim_zoned_parse, SIG_ERROR, 1, "Parse an ISO 8601 zoned datetime string.", "jiff", r#"(zoned/parse "2024-06-19T15:22:45-04:00[America/New_York]")"#),
    EllePrimDef::exact("span/parse", parse::prim_span_parse, SIG_ERROR, 1, "Parse an ISO 8601 duration string.", "jiff", r#"(span/parse "P1Y2M3DT4H5M6S")"#),
    EllePrimDef::exact("signed-duration/parse", parse::prim_signed_duration_parse, SIG_ERROR, 1, "Parse an ISO 8601 duration string as an exact signed duration.", "jiff", r#"(signed-duration/parse "PT3600S")"#),
    EllePrimDef::exact("temporal/parse-with", parse::prim_temporal_parse_with, SIG_ERROR, 2, "Parse a string using a strftime-style format.", "jiff", r#"(temporal/parse-with "%Y-%m-%d" "2024-06-19")"#),
    // format
    EllePrimDef::exact("temporal/string", format::prim_temporal_string, SIG_ERROR, 1, "Convert any temporal value to its ISO 8601 string representation.", "jiff", "(temporal/string (date 2024 6 19))"),
    EllePrimDef::exact("temporal/format", format::prim_temporal_format, SIG_ERROR, 2, "Format a temporal value using a strftime-style format string.", "jiff", r#"(temporal/format "%B %d, %Y" (date 2024 6 19))"#),
    EllePrimDef::exact("timestamp/->epoch", format::prim_ts_epoch, SIG_ERROR, 1, "Timestamp as float seconds since Unix epoch.", "jiff", "(timestamp/->epoch (timestamp))"),
    EllePrimDef::exact("timestamp/->epoch-millis", format::prim_ts_epoch_millis, SIG_ERROR, 1, "Timestamp as integer milliseconds since Unix epoch.", "jiff", "(timestamp/->epoch-millis (timestamp))"),
    EllePrimDef::exact("timestamp/->epoch-micros", format::prim_ts_epoch_micros, SIG_ERROR, 1, "Timestamp as integer microseconds since Unix epoch.", "jiff", "(timestamp/->epoch-micros (timestamp))"),
    EllePrimDef::exact("timestamp/->epoch-nanos", format::prim_ts_epoch_nanos, SIG_ERROR, 1, "Timestamp as integer nanoseconds since Unix epoch.", "jiff", "(timestamp/->epoch-nanos (timestamp))"),
    EllePrimDef::exact("timestamp/from-epoch-seconds", format::prim_ts_from_epoch_seconds, SIG_ERROR, 1, "Construct a timestamp from seconds since Unix epoch.", "jiff", "(timestamp/from-epoch-seconds 1718826165)"),
    EllePrimDef::exact("timestamp/from-epoch-millis", format::prim_ts_from_epoch_millis, SIG_ERROR, 1, "Construct a timestamp from milliseconds since Unix epoch.", "jiff", "(timestamp/from-epoch-millis 1718826165000)"),
    EllePrimDef::exact("timestamp/from-epoch-micros", format::prim_ts_from_epoch_micros, SIG_ERROR, 1, "Construct a timestamp from microseconds since Unix epoch.", "jiff", "(timestamp/from-epoch-micros 1718826165000000)"),
    EllePrimDef::exact("timestamp/from-epoch-nanos", format::prim_ts_from_epoch_nanos, SIG_ERROR, 1, "Construct a timestamp from nanoseconds since Unix epoch.", "jiff", "(timestamp/from-epoch-nanos 1718826165000000000)"),
    // arith
    EllePrimDef::exact("date/add", arith::prim_date_add, SIG_ERROR, 2, "Add a span or signed-duration to a date.", "jiff", "(date/add (date 2024 6 19) (span {:days 30}))"),
    EllePrimDef::exact("date/sub", arith::prim_date_sub, SIG_ERROR, 2, "Subtract a span or signed-duration from a date.", "jiff", "(date/sub (date 2024 6 19) (span {:days 30}))"),
    EllePrimDef::exact("time/add", arith::prim_time_add, SIG_ERROR, 2, "Add a span or signed-duration to a time.", "jiff", "(time/add (time 15 22 45) (span {:hours 2}))"),
    EllePrimDef::exact("time/sub", arith::prim_time_sub, SIG_ERROR, 2, "Subtract a span or signed-duration from a time.", "jiff", "(time/sub (time 15 22 45) (span {:hours 2}))"),
    EllePrimDef::exact("datetime/add", arith::prim_datetime_add, SIG_ERROR, 2, "Add a span or signed-duration to a datetime.", "jiff", "(datetime/add (datetime 2024 6 19 15 22 45) (span {:hours 2}))"),
    EllePrimDef::exact("datetime/sub", arith::prim_datetime_sub, SIG_ERROR, 2, "Subtract a span or signed-duration from a datetime.", "jiff", "(datetime/sub (datetime 2024 6 19 15 22 45) (span {:hours 2}))"),
    EllePrimDef::exact("timestamp/add", arith::prim_timestamp_add, SIG_ERROR, 2, "Add a span or signed-duration to a timestamp.", "jiff", "(timestamp/add (timestamp) (span {:hours 2}))"),
    EllePrimDef::exact("timestamp/sub", arith::prim_timestamp_sub, SIG_ERROR, 2, "Subtract a span or signed-duration from a timestamp.", "jiff", "(timestamp/sub (timestamp) (span {:hours 2}))"),
    EllePrimDef::exact("zoned/add", arith::prim_zoned_add, SIG_ERROR, 2, "Add a span or signed-duration to a zoned datetime.", "jiff", r#"(zoned/add (now) (span {:hours 2}))"#),
    EllePrimDef::exact("zoned/sub", arith::prim_zoned_sub, SIG_ERROR, 2, "Subtract a span or signed-duration from a zoned datetime.", "jiff", r#"(zoned/sub (now) (span {:hours 2}))"#),
    EllePrimDef::range("timestamp/since", arith::prim_timestamp_since, SIG_ERROR, 2, 3, "Signed duration from b to a.", "jiff", "(timestamp/since ts1 ts2)"),
    EllePrimDef::range("timestamp/until", arith::prim_timestamp_until, SIG_ERROR, 2, 3, "Signed duration from a to b.", "jiff", "(timestamp/until ts1 ts2)"),
    EllePrimDef::range("zoned/until", arith::prim_zoned_until, SIG_ERROR, 2, 3, "Span from a to b.", "jiff", "(zoned/until z1 z2)"),
    EllePrimDef::exact("span/add", arith::prim_span_add, SIG_ERROR, 2, "Add two spans.", "jiff", "(span/add (span {:hours 1}) (span {:minutes 30}))"),
    EllePrimDef::exact("span/mul", arith::prim_span_mul, SIG_ERROR, 2, "Multiply a span by an integer.", "jiff", "(span/mul (span {:hours 1}) 3)"),
    EllePrimDef::exact("span/negate", arith::prim_span_negate, SIG_ERROR, 1, "Negate a span.", "jiff", "(span/negate (span {:hours 1}))"),
    EllePrimDef::exact("span/abs", arith::prim_span_abs, SIG_ERROR, 1, "Absolute value of a span.", "jiff", "(span/abs (span/negate (span {:hours 1})))"),
    EllePrimDef::exact("span-total", arith::prim_span_total, SIG_ERROR, 2, "Total value of a span in the given unit as a float.", "jiff", "(span-total (span {:hours 1 :minutes 30}) :minutes)"),
    EllePrimDef::exact("signed-duration/add", arith::prim_sd_add, SIG_ERROR, 2, "Add two signed-durations.", "jiff", "(signed-duration/add (signed-duration 3600) (signed-duration 1800))"),
    EllePrimDef::exact("signed-duration/negate", arith::prim_sd_negate, SIG_ERROR, 1, "Negate a signed-duration.", "jiff", "(signed-duration/negate (signed-duration 3600))"),
    EllePrimDef::exact("signed-duration/abs", arith::prim_sd_abs, SIG_ERROR, 1, "Absolute value of a signed-duration.", "jiff", "(signed-duration/abs (signed-duration -3600))"),
    EllePrimDef::exact("temporal/compare", arith::prim_temporal_compare, SIG_ERROR, 2, "Compare two temporal values. Returns -1, 0, or 1.", "jiff", "(temporal/compare (date 2024 1 1) (date 2024 6 19))"),
    EllePrimDef::exact("temporal/before?", arith::prim_temporal_before, SIG_ERROR, 2, "True if a is before b.", "jiff", "(temporal/before? (date 2024 1 1) (date 2024 6 19))"),
    EllePrimDef::exact("temporal/after?", arith::prim_temporal_after, SIG_ERROR, 2, "True if a is after b.", "jiff", "(temporal/after? (date 2024 6 19) (date 2024 1 1))"),
    EllePrimDef::exact("temporal/equal?", arith::prim_temporal_equal, SIG_ERROR, 2, "True if a and b represent the same instant/value.", "jiff", "(temporal/equal? (date 2024 6 19) (date 2024 6 19))"),
    // calendar
    EllePrimDef::exact("date/start-of-month", calendar::prim_date_start_of_month, SIG_ERROR, 1, "First day of the month.", "jiff", "(date/start-of-month (date 2024 6 19))"),
    EllePrimDef::exact("date/end-of-month", calendar::prim_date_end_of_month, SIG_ERROR, 1, "Last day of the month.", "jiff", "(date/end-of-month (date 2024 6 19))"),
    EllePrimDef::exact("date/start-of-year", calendar::prim_date_start_of_year, SIG_ERROR, 1, "January 1 of the same year.", "jiff", "(date/start-of-year (date 2024 6 19))"),
    EllePrimDef::exact("date/end-of-year", calendar::prim_date_end_of_year, SIG_ERROR, 1, "December 31 of the same year.", "jiff", "(date/end-of-year (date 2024 6 19))"),
    EllePrimDef::exact("date/next-weekday", calendar::prim_date_next_weekday, SIG_ERROR, 2, "Next occurrence of a weekday.", "jiff", "(date/next-weekday (date 2024 6 19) :monday)"),
    EllePrimDef::exact("date/prev-weekday", calendar::prim_date_prev_weekday, SIG_ERROR, 2, "Previous occurrence of a weekday.", "jiff", "(date/prev-weekday (date 2024 6 19) :monday)"),
    EllePrimDef::exact("tz-list", calendar::prim_tz_list, SIG_OK, 0, "List all available IANA timezone names.", "jiff", "(tz-list)"),
    EllePrimDef::exact("tz-valid?", calendar::prim_tz_valid, SIG_OK, 1, "True if the string is a valid IANA timezone name.", "jiff", r#"(tz-valid? "America/New_York")"#),
    EllePrimDef::exact("tz-system", calendar::prim_tz_system, SIG_ERROR, 0, "Return the system's IANA timezone name.", "jiff", "(tz-system)"),
    EllePrimDef::exact("tz-fixed", calendar::prim_tz_fixed, SIG_ERROR, 1, "Create a fixed-offset timezone string from seconds offset.", "jiff", "(tz-fixed -18000)"),
    EllePrimDef::exact("temporal/series", calendar::prim_temporal_series, SIG_ERROR, 3, "Generate an array of temporal values.", "jiff", "(temporal/series (date 2024 1 1) (span {:months 1}) 12)"),
    // convert
    EllePrimDef::exact("date/->date", convert::prim_to_date, SIG_ERROR, 1, "Extract or convert to a date.", "jiff", "(date/->date (now))"),
    EllePrimDef::exact("time/->time", convert::prim_to_time, SIG_ERROR, 1, "Extract or convert to a time.", "jiff", "(time/->time (now))"),
    EllePrimDef::range("datetime/->datetime", convert::prim_to_datetime, SIG_ERROR, 1, 2, "Extract datetime from zoned/date, or combine a date and time.", "jiff", "(datetime/->datetime (now))"),
    EllePrimDef::exact("timestamp/->timestamp", convert::prim_to_timestamp, SIG_ERROR, 1, "Extract timestamp from zoned, or identity on timestamp.", "jiff", "(timestamp/->timestamp (now))"),
    EllePrimDef::exact("zoned/in-tz", convert::prim_zoned_in_tz, SIG_ERROR, 2, "Convert a zoned datetime to a different timezone.", "jiff", r#"(zoned/in-tz (now) "America/Los_Angeles")"#),
    EllePrimDef::exact("span/->signed-duration", convert::prim_span_to_sd, SIG_ERROR, 1, "Convert a span to a signed-duration.", "jiff", "(span/->signed-duration (span {:hours 1 :minutes 30}))"),
    EllePrimDef::exact("temporal/round", convert::prim_temporal_round, SIG_ERROR, 2, "Round a temporal value.", "jiff", "(temporal/round (time 15 22 45) {:unit :hour})"),
];
