//! `next_after` — the one new mechanical piece (reminders scope "Cron semantics & next after T").
//! Compute the next instant strictly after `after` (logical `ts` seconds) that the 5-field cron
//! `schedule` matches, on the **injected logical clock** (testing §3 — never wall-clock).
//!
//! `croner`'s `Cron::find_next_occurrence(&DateTime<Tz>, inclusive=false)` does exactly this: it
//! takes a *supplied* `DateTime`, so the caller controls time, and it is generic over the chrono
//! zone, so the cron's hour/day fields are read in whatever zone the `DateTime` carries. We convert
//! the logical `u64` seconds to a `DateTime` in the schedule's zone for croner and back to seconds.
//! Easy to get subtly wrong by hand (multi-value fields, month/day rollover, DST gaps and folds); a
//! vetted crate does it, per the scope's hard-problem note.
//!
//! `inclusive=false` so "next after T" is strictly future — a fire at exactly `T` is not re-found
//! at `T`, which is what makes the reactor's advance (recompute from `now`) idempotent.

use std::str::FromStr;

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use croner::Cron;

use crate::error::ReminderError;

/// [`next_after_in`] in UTC — the form every schedule used before a zone could be stated, and the
/// one the flow cron and the rule preview still run on.
pub fn next_after(schedule: &str, after_secs: u64) -> Result<u64, ReminderError> {
    next_after_in(schedule, after_secs, "")
}

/// The next instant strictly after `after_secs` (logical seconds since the unix epoch) that
/// `schedule` matches when its clock fields are read in the IANA zone `tz` (empty ⇒ UTC), as
/// logical seconds. `Err(BadCron)` if the cron string is not a valid 5-field Vixie cron
/// expression; `Err(BadInput)` if `tz` is not a zone name.
///
/// "0 9 * * *" in `Australia/Sydney` is 09:00 Sydney wall-clock every day — 22:00 or 23:00 UTC
/// depending on DST — and the returned instant is that, as a plain UTC-second count.
pub fn next_after_in(schedule: &str, after_secs: u64, tz: &str) -> Result<u64, ReminderError> {
    let cron = Cron::from_str(schedule).map_err(|e| ReminderError::BadCron(e.to_string()))?;
    let zone = parse_tz(tz)?;
    let after = secs_to_dt(after_secs).with_timezone(&zone);
    // `inclusive = false`: the slot strictly after the anchor. A reminder created at `now` thus
    // first fires at the next future slot; a re-scan at the same `now` finds nothing new.
    let next = cron
        .find_next_occurrence(&after, false)
        .map_err(|e| ReminderError::BadCron(e.to_string()))?;
    Ok(dt_to_secs(next))
}

/// Whether `schedule` is a valid 5-field cron expression (the create-time best-effort check).
pub fn is_valid(schedule: &str) -> bool {
    Cron::from_str(schedule).is_ok()
}

/// Whether `tz` names a zone this build knows (empty counts — it means UTC).
pub fn is_valid_tz(tz: &str) -> bool {
    parse_tz(tz).is_ok()
}

/// The zone by IANA name. Empty (and `"UTC"`) is UTC; an unknown name is a refusal naming the bad
/// value, never a panic — the create verb runs this so a typo is caught with a human watching.
fn parse_tz(tz: &str) -> Result<Tz, ReminderError> {
    let tz = tz.trim();
    if tz.is_empty() {
        return Ok(Tz::UTC);
    }
    tz.parse::<Tz>()
        .map_err(|_| ReminderError::BadInput(format!("unknown timezone: {tz}")))
}

fn secs_to_dt(secs: u64) -> DateTime<Utc> {
    // Logical seconds since the unix epoch → a UTC instant. `from_timestamp` returns `None` only
    // for out-of-range i64; clamp an absurd value to the epoch rather than panic.
    DateTime::<Utc>::from_timestamp(secs as i64, 0).unwrap_or_default()
}

fn dt_to_secs<Z: TimeZone>(dt: DateTime<Z>) -> u64 {
    dt.timestamp().max(0) as u64
}
