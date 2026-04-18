//! Type predicates

use crate::{as_jiff, JiffValue};
use elle_plugin::{ElleResult, ElleValue};

pub extern "C" fn prim_date_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(matches!(as_jiff(unsafe { a.arg(args, nargs, 0) }), Some(JiffValue::Date(_)))))
}
pub extern "C" fn prim_time_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(matches!(as_jiff(unsafe { a.arg(args, nargs, 0) }), Some(JiffValue::Time(_)))))
}
pub extern "C" fn prim_datetime_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(matches!(as_jiff(unsafe { a.arg(args, nargs, 0) }), Some(JiffValue::DateTime(_)))))
}
pub extern "C" fn prim_timestamp_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(matches!(as_jiff(unsafe { a.arg(args, nargs, 0) }), Some(JiffValue::Timestamp(_)))))
}
pub extern "C" fn prim_zoned_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(matches!(as_jiff(unsafe { a.arg(args, nargs, 0) }), Some(JiffValue::Zoned(_)))))
}
pub extern "C" fn prim_span_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(matches!(as_jiff(unsafe { a.arg(args, nargs, 0) }), Some(JiffValue::Span(_)))))
}
pub extern "C" fn prim_signed_duration_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(matches!(as_jiff(unsafe { a.arg(args, nargs, 0) }), Some(JiffValue::SignedDuration(_)))))
}
pub extern "C" fn prim_temporal_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = crate::api();
    a.ok(a.boolean(as_jiff(unsafe { a.arg(args, nargs, 0) }).is_some()))
}
