//! The business-hours arithmetic an SLA deadline is made of
//! (`docs/scope/insights/case-plane-scope.md` — "Prioritisation is a deadline, not a score").
//!
//! **Pure.** No wall clock, no store, no I/O: `from_ms` is injected by the caller and everything
//! else is a function of the calendar. That is the whole reason the interesting cases — a Friday
//! afternoon, a public holiday, a daylight-saving transition — are unit-testable at all.
//!
//! # Semantics
//!
//! Start at `from_ms`. If that instant is outside business hours, advance to the next opening.
//! Then consume `hours` of business time, skipping closed days and holidays, spilling across days
//! as needed. The window is half-open `[open, close)`: an instant exactly at opening is inside,
//! an instant exactly at closing is not and rolls to the next open day.
//!
//! # Termination
//!
//! A calendar with no open hours — or one whose every open day for the next decade is a holiday —
//! would otherwise loop for ever. The scan is bounded at [`MAX_DAYS_SCANNED`] and returns
//! [`NEVER`] (`u64::MAX`) instead.
//!
//! **Why a sentinel and not an error.** Two reasons. First, the fail-safe direction: a deadline of
//! "never" means the case never breaches, which is quiet and inspectable; the alternative failure
//! (returning `from_ms`, or zero) breaches every case in the workspace instantly and buries the
//! real work in false alarms. Second, [`Calendar::validate`] rejects the unusable calendar at the
//! `policy.sla.set` door, so a stored calendar cannot reach here in that state — the sentinel is
//! the belt to that braces, not the primary defence. Callers that want the diagnosis rather than
//! the sentinel call [`Calendar::validate`] themselves; `NEVER` is a public constant precisely so
//! a reactor can test for it.

use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::calendar::{Calendar, DayHours};
use crate::policy::ServicePolicy;

/// The deadline of a case whose calendar never opens: "never due". See the module note on why this
/// is a sentinel rather than an error.
pub const NEVER: u64 = u64::MAX;

/// How many local days the stepper will scan before giving up and returning [`NEVER`]. Ten years —
/// far past any real SLA, and small enough that the give-up is instant.
pub const MAX_DAYS_SCANNED: u32 = 3_660;

const MS_PER_DAY: i64 = 24 * 60 * 60 * 1_000;
const MS_PER_MINUTE: i64 = 60_000;
const MS_PER_HOUR: i64 = 3_600_000;

/// Add `hours` of **business time** to `from_ms` (epoch milliseconds, UTC), returning the resulting
/// epoch-ms instant. [`Calendar::Always`] is plain wall arithmetic. Returns [`NEVER`] if the
/// calendar never opens (see the module note).
pub fn add_business_hours(cal: &Calendar, from_ms: u64, hours: u32) -> u64 {
    match cal {
        Calendar::Always => from_ms.saturating_add(hours as u64 * MS_PER_HOUR as u64),
        Calendar::Business {
            hours: week,
            holidays,
            ..
        } => {
            let Ok(Some(tz)) = cal.tz() else {
                // An unresolvable zone is a malformed stored row; `validate` refuses these at the
                // write door. Fail to "never due" rather than to an arbitrary instant.
                return NEVER;
            };
            step(tz, week, holidays, from_ms, hours)
        }
    }
}

/// When the first response is due: `opened_ms` plus `respond_h` business hours.
pub fn respond_by(policy: &ServicePolicy, opened_ms: u64) -> u64 {
    add_business_hours(&policy.calendar, opened_ms, policy.respond_h)
}

/// When resolution is due: `opened_ms` plus `resolve_h` business hours. This is the queue's sort
/// key — the deadline that stands in for a priority score.
pub fn due_at(policy: &ServicePolicy, opened_ms: u64) -> u64 {
    add_business_hours(&policy.calendar, opened_ms, policy.resolve_h)
}

/// Walk local days from `from_ms`, consuming open time until `hours` is spent.
fn step(tz: Tz, week: &[DayHours; 7], holidays: &[String], from_ms: u64, hours: u32) -> u64 {
    let Some(from) = Utc.timestamp_millis_opt(from_ms as i64).single() else {
        return NEVER;
    };
    let local = from.with_timezone(&tz);
    let mut date = local.date_naive();
    // Exact time-of-day in ms, so a `from` with seconds on it is not silently rounded down.
    let start_of_day = date.and_time(NaiveTime::MIN);
    let mut remaining = hours as i64 * MS_PER_HOUR;
    let mut first_day = true;

    for _ in 0..MAX_DAYS_SCANNED {
        // 0 = Monday, by chrono's definition — the same end of the convention `calendar.rs`
        // documents, so the two cannot drift apart.
        let dh = week[date.weekday().num_days_from_monday() as usize];
        let open = dh.open_min as i64 * MS_PER_MINUTE;
        let close = dh.close_min as i64 * MS_PER_MINUTE;

        if dh.is_open() && !is_holiday(holidays, date) {
            let cursor = if first_day {
                // Only the starting day begins mid-window; every later day begins at opening.
                (local.naive_local() - start_of_day)
                    .num_milliseconds()
                    .max(open)
            } else {
                open
            };
            if cursor < close {
                let available = close - cursor;
                if available >= remaining {
                    return to_epoch_ms(tz, date, cursor + remaining);
                }
                remaining -= available;
            }
        }

        first_day = false;
        let Some(next) = date.succ_opt() else {
            return NEVER;
        };
        date = next;
    }
    NEVER
}

/// Whether `date` (a local civil date) is in the holiday list. Holidays are `YYYY-MM-DD` in the
/// calendar's own local time, so this compares local date to local date — no conversion.
fn is_holiday(holidays: &[String], date: NaiveDate) -> bool {
    if holidays.is_empty() {
        return false;
    }
    let iso = date.format("%Y-%m-%d").to_string();
    holidays.iter().any(|h| h == &iso)
}

/// Local civil `date` plus `ms_past_midnight` → epoch ms.
///
/// Two daylight-saving hazards live here and both are handled explicitly:
/// * **Ambiguous** (clocks went back; the local time happens twice) — take the **earlier** instant,
///   the same choice `lb-schedules` makes. Earlier is the conservative one for a deadline.
/// * **Nonexistent** (clocks went forward; the local time never happens) — step forward a minute at
///   a time to the first instant that does exist, i.e. the moment the clock jumps to.
fn to_epoch_ms(tz: Tz, date: NaiveDate, ms_past_midnight: i64) -> u64 {
    // A window closing at 1440 (midnight) lands on the next date at 00:00.
    let (date, ms) = if ms_past_midnight >= MS_PER_DAY {
        match date.succ_opt() {
            Some(d) => (d, ms_past_midnight - MS_PER_DAY),
            None => return NEVER,
        }
    } else {
        (date, ms_past_midnight)
    };
    let Some(time) = NaiveTime::from_num_seconds_from_midnight_opt(
        (ms / 1000) as u32,
        (ms % 1000) as u32 * 1_000_000,
    ) else {
        return NEVER;
    };
    let mut naive: NaiveDateTime = date.and_time(time);
    for _ in 0..=180 {
        match tz.from_local_datetime(&naive) {
            LocalResult::Single(dt) => return dt.timestamp_millis().max(0) as u64,
            LocalResult::Ambiguous(earlier, _) => {
                return earlier.timestamp_millis().max(0) as u64;
            }
            LocalResult::None => naive += Duration::minutes(1),
        }
    }
    NEVER
}
