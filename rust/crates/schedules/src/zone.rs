//! The zone a schedule's wall times live in, and the two conversions it needs: an instant to wall
//! time (to evaluate) and wall time to an instant (to report `next_transition`).

use chrono::{DateTime, Duration, FixedOffset, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::error::ScheduleError;

/// UTC when `use_utc` is set or no `timezone` is given, else an IANA zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zone {
    Utc,
    Named(Tz),
}

impl Zone {
    pub fn new(use_utc: bool, timezone: &str) -> Result<Self, ScheduleError> {
        if use_utc || timezone.is_empty() {
            return Ok(Zone::Utc);
        }
        timezone
            .parse::<Tz>()
            .map(Zone::Named)
            .map_err(|_| ScheduleError::UnknownTimezone(timezone.to_string()))
    }

    /// The wall time at `now`.
    pub fn local(&self, now: DateTime<Utc>) -> NaiveDateTime {
        match self {
            Zone::Utc => now.naive_utc(),
            Zone::Named(tz) => now.with_timezone(tz).naive_local(),
        }
    }

    /// The instant a wall time names. A wall time that happens twice (clocks go back) takes its first
    /// occurrence; one that never happens (clocks go forward) moves to the first minute that exists.
    pub fn instant(&self, local: NaiveDateTime) -> DateTime<FixedOffset> {
        match self {
            Zone::Utc => Utc.from_utc_datetime(&local).fixed_offset(),
            Zone::Named(tz) => (0..=180)
                .find_map(|m| {
                    tz.from_local_datetime(&(local + Duration::minutes(m)))
                        .earliest()
                })
                .map(|t| t.fixed_offset())
                .unwrap_or_else(|| Utc.from_utc_datetime(&local).fixed_offset()),
        }
    }
}
