//! `next_after` — the one new mechanical piece (reminders scope "Cron semantics & next after T").
//! Compute the next instant strictly after `after` (logical `ts` seconds) that the 5-field cron
//! `schedule` matches, on the **injected logical clock** (testing §3 — never wall-clock).
//!
//! `croner`'s `Cron::find_next_occurrence(&DateTime, inclusive=false)` does exactly this: it takes
//! a *supplied* `DateTime`, so the caller controls time. We convert the logical `u64` seconds to a
//! `DateTime<Utc>` for croner and back. Easy to get subtly wrong by hand (multi-value fields,
//! month/day rollover); a vetted crate does it, per the scope's hard-problem note.
//!
//! `inclusive=false` so "next after T" is strictly future — a fire at exactly `T` is not re-found
//! at `T`, which is what makes the reactor's advance (recompute from `now`) idempotent.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use croner::Cron;

use crate::error::ReminderError;

/// The next instant strictly after `after_secs` (logical seconds since the unix epoch) that
/// `schedule` matches, as logical seconds. `Err` if the cron string is not a valid 5-field Vixie
/// cron expression.
pub fn next_after(schedule: &str, after_secs: u64) -> Result<u64, ReminderError> {
    let cron = Cron::from_str(schedule).map_err(|e| ReminderError::BadCron(e.to_string()))?;
    let after = secs_to_dt(after_secs);
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

fn secs_to_dt(secs: u64) -> DateTime<Utc> {
    // Logical seconds since the unix epoch → a UTC instant. `from_timestamp` returns `None` only
    // for out-of-range i64; clamp an absurd value to the epoch rather than panic.
    DateTime::<Utc>::from_timestamp(secs as i64, 0).unwrap_or_else(DateTime::<Utc>::default)
}

fn dt_to_secs(dt: DateTime<Utc>) -> u64 {
    dt.timestamp().max(0) as u64
}
