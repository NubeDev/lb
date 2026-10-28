//! `business_ms_between` — how much business time lies between two instants on a calendar.
//!
//! The inverse of [`crate::add_business_ms`]: an SLA pause ([`crate::sla_pause`]) is measured with
//! this and handed back to the deadline arithmetic, so a pause over a weekend adds nothing and a
//! pause across a working afternoon adds that afternoon. Pure, like the deadline stepper: no clock,
//! no store, the calendar's own local days.
//!
//! One responsibility: the business time in a span.

use chrono::{Datelike, TimeZone, Utc};

use crate::calendar::Calendar;
use crate::deadline::{is_holiday, to_epoch_ms, MAX_DAYS_SCANNED};

const MS_PER_MINUTE: i64 = 60_000;

/// The business ms in `[from_ms, to_ms)` on `cal`. Zero for an empty or backwards span, and for a
/// calendar whose zone cannot be resolved (`validate` refuses those at the write door; zero is the
/// fail-safe here, since it never moves a deadline). A span longer than [`MAX_DAYS_SCANNED`] days
/// counts only that many.
pub fn business_ms_between(cal: &Calendar, from_ms: u64, to_ms: u64) -> u64 {
    if to_ms <= from_ms {
        return 0;
    }
    let Calendar::Business {
        hours: week,
        holidays,
        ..
    } = cal
    else {
        return to_ms - from_ms;
    };
    let Ok(Some(tz)) = cal.tz() else {
        return 0;
    };
    let Some(from) = Utc.timestamp_millis_opt(from_ms as i64).single() else {
        return 0;
    };
    let mut date = from.with_timezone(&tz).date_naive();
    let mut total = 0;
    for _ in 0..MAX_DAYS_SCANNED {
        let dh = week[date.weekday().num_days_from_monday() as usize];
        if dh.is_open() && !is_holiday(holidays, date) {
            let open = to_epoch_ms(tz, date, dh.open_min as i64 * MS_PER_MINUTE);
            let close = to_epoch_ms(tz, date, dh.close_min as i64 * MS_PER_MINUTE);
            if open >= to_ms {
                break;
            }
            let start = open.max(from_ms);
            let end = close.min(to_ms);
            total += end.saturating_sub(start);
        }
        let Some(next) = date.succ_opt() else {
            break;
        };
        date = next;
    }
    total
}
