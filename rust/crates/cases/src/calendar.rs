//! `Calendar` — the business-hours definition an SLA deadline is measured in
//! (`docs/scope/insights/case-plane-scope.md`, the `service_policy` row).
//!
//! **One calendar type, no per-site exceptions in v1.** A site that keeps different hours gets its
//! own `service_policy` row with `match.site` set — the resolution ladder in
//! [`crate::policy_match`] already expresses "this site is different", so a second exception
//! mechanism inside the calendar would be a redundant way to say the same thing.
//!
//! # Monday is index 0
//!
//! `hours[0]` is **Monday**, `hours[6]` is **Sunday** — the ISO week, and the same convention
//! `host/src/timerange/civil.rs::weekday` already uses in this tree. Stated this loudly because an
//! off-by-one here does not crash, it silently bills a Sunday as a Monday: every deadline in the
//! workspace lands on the wrong day and the arithmetic still looks plausible. The stepping code
//! derives the index from `chrono`'s `num_days_from_monday()`, which is 0 = Monday by definition,
//! so the two ends of the convention cannot drift apart.
//!
//! # Timezones are IANA names, and DST is handled
//!
//! `tz` is an IANA zone name (`"Australia/Brisbane"`, `"Australia/Sydney"`), resolved through
//! `chrono-tz` — already a first-class workspace dependency here, consumed by `lb-schedules`,
//! `lb-prefs` and `lb-host`. A fixed UTC-offset field was rejected: it is wrong by an hour for
//! roughly half the year in every DST zone, and "the deadline moved an hour" is precisely the class
//! of bug an SLA product cannot ship. Since the tzdata is already linked into the binary, the
//! honest option costs nothing.
//!
//! Resolution is a pure function of a static table — no clock, no I/O — so the arithmetic built on
//! top of it stays pure and testable.

use serde::{Deserialize, Serialize};

/// Minutes in a full day — the exclusive upper bound for a [`DayHours`] boundary.
pub const MINUTES_PER_DAY: u32 = 24 * 60;

/// One weekday's opening window, in **minutes past local midnight**.
///
/// The window is half-open, `[open_min, close_min)`: an instant exactly at `open_min` is inside
/// business hours, an instant exactly at `close_min` is not. `open_min == close_min` (and, after
/// validation rejects it, any `close_min < open_min`) means **closed all day**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayHours {
    /// Opening time, minutes past local midnight. `0..=1440`.
    #[serde(default)]
    pub open_min: u32,
    /// Closing time, minutes past local midnight. `0..=1440`; `1440` is midnight at the day's end.
    #[serde(default)]
    pub close_min: u32,
}

impl DayHours {
    /// A closed day — the zero value, and the one `Default` gives you.
    pub const CLOSED: DayHours = DayHours {
        open_min: 0,
        close_min: 0,
    };

    /// A day open from `open_min` to `close_min`.
    pub const fn new(open_min: u32, close_min: u32) -> Self {
        DayHours {
            open_min,
            close_min,
        }
    }

    /// Whether this day has any open time at all.
    pub const fn is_open(&self) -> bool {
        self.close_min > self.open_min
    }
}

impl Default for DayHours {
    fn default() -> Self {
        DayHours::CLOSED
    }
}

/// The calendar an SLA clock runs against.
///
/// Serialised **internally tagged** on `kind` — `{"kind":"always"}` /
/// `{"kind":"business","tz":"Australia/Brisbane",...}` — so a settings form can switch the variant
/// by editing one field, and a stored row stays readable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Calendar {
    /// 24x7 — every hour is a business hour. The default, and plain wall arithmetic.
    #[default]
    Always,
    /// Business hours in a named timezone, with a weekly pattern and a holiday list.
    Business {
        /// IANA timezone name, e.g. `"Australia/Brisbane"`. Resolved through `chrono-tz`.
        tz: String,
        /// The weekly pattern. **Index 0 is Monday**, index 6 is Sunday.
        #[serde(default = "closed_week")]
        hours: [DayHours; 7],
        /// `YYYY-MM-DD` dates that are closed regardless of the weekly pattern. These are dates in
        /// **this calendar's own local time**, not UTC — a public holiday is a local-calendar fact.
        #[serde(default)]
        holidays: Vec<String>,
    },
}

fn closed_week() -> [DayHours; 7] {
    [DayHours::CLOSED; 7]
}

/// Why a calendar cannot be used. Returned by [`Calendar::validate`], which the `policy.sla.set`
/// write path calls — so an unusable calendar is rejected at the door and never reaches the clock.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CalendarError {
    /// The `tz` string is not an IANA zone name `chrono-tz` knows.
    #[error("unknown timezone: {0}")]
    UnknownTimezone(String),
    /// A `DayHours` boundary is past midnight, or closes before it opens.
    #[error("day {day} has invalid hours: open_min={open_min}, close_min={close_min}")]
    BadDayHours {
        /// Weekday index, 0 = Monday.
        day: usize,
        open_min: u32,
        close_min: u32,
    },
    /// A holiday entry is not a `YYYY-MM-DD` date.
    #[error("holiday {0:?} is not a YYYY-MM-DD date")]
    BadHoliday(String),
    /// Every day of the week is closed, so no amount of business time ever elapses.
    #[error("calendar has no open hours in the whole week")]
    NeverOpen,
}

impl Calendar {
    /// Resolve the timezone. `Always` has none (it never converts to local time).
    pub fn tz(&self) -> Result<Option<chrono_tz::Tz>, CalendarError> {
        match self {
            Calendar::Always => Ok(None),
            Calendar::Business { tz, .. } => tz
                .parse::<chrono_tz::Tz>()
                .map(Some)
                .map_err(|_| CalendarError::UnknownTimezone(tz.clone())),
        }
    }

    /// Reject a calendar the clock could not use. Called by the `policy.sla.set` write path so the
    /// arithmetic downstream is total: a stored calendar always resolves.
    ///
    /// `NeverOpen` is a hard reject rather than a shrug because the alternative — storing it and
    /// letting every deadline come back "never" — is a silent workspace-wide SLA outage, and the
    /// only signal would be that nothing ever breaches.
    pub fn validate(&self) -> Result<(), CalendarError> {
        let Calendar::Business {
            hours, holidays, ..
        } = self
        else {
            return Ok(());
        };
        self.tz()?;
        for (day, dh) in hours.iter().enumerate() {
            if dh.open_min > MINUTES_PER_DAY
                || dh.close_min > MINUTES_PER_DAY
                || dh.close_min < dh.open_min
            {
                return Err(CalendarError::BadDayHours {
                    day,
                    open_min: dh.open_min,
                    close_min: dh.close_min,
                });
            }
        }
        for h in holidays {
            if chrono::NaiveDate::parse_from_str(h, "%Y-%m-%d").is_err() {
                return Err(CalendarError::BadHoliday(h.clone()));
            }
        }
        if !hours.iter().any(DayHours::is_open) {
            return Err(CalendarError::NeverOpen);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monday is index 0 — pinned here as well as in the stepping tests, because this is the
    /// convention every other assertion in the crate silently assumes.
    #[test]
    fn monday_is_index_zero() {
        use chrono::Datelike;
        // 2026-10-05 is a Monday.
        let d = chrono::NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
        assert_eq!(d.weekday().num_days_from_monday(), 0);
        assert_eq!(d.weekday(), chrono::Weekday::Mon);
    }

    /// A day whose boundaries are equal is closed, and `is_open` is the single place that says so.
    #[test]
    fn equal_boundaries_are_a_closed_day() {
        assert!(!DayHours::new(540, 540).is_open());
        assert!(!DayHours::CLOSED.is_open());
        assert!(DayHours::new(540, 1020).is_open());
    }

    /// The write path refuses every calendar the clock could not use, including the all-closed week
    /// that would otherwise make every deadline "never".
    #[test]
    fn validate_rejects_the_unusable() {
        let biz = |tz: &str, hours: [DayHours; 7], holidays: Vec<String>| Calendar::Business {
            tz: tz.into(),
            hours,
            holidays,
        };
        let mut week = [DayHours::CLOSED; 7];
        week[0] = DayHours::new(540, 1020);

        assert!(biz("Australia/Brisbane", week, vec![]).validate().is_ok());
        assert_eq!(
            biz("Mars/Olympus", week, vec![]).validate(),
            Err(CalendarError::UnknownTimezone("Mars/Olympus".into()))
        );
        assert_eq!(
            biz("Australia/Brisbane", [DayHours::CLOSED; 7], vec![]).validate(),
            Err(CalendarError::NeverOpen)
        );
        assert_eq!(
            biz("Australia/Brisbane", week, vec!["25 Dec".into()]).validate(),
            Err(CalendarError::BadHoliday("25 Dec".into()))
        );
        let mut backwards = week;
        backwards[1] = DayHours::new(1020, 540);
        assert!(matches!(
            biz("Australia/Brisbane", backwards, vec![]).validate(),
            Err(CalendarError::BadDayHours { day: 1, .. })
        ));
        // `Always` is always valid and has no timezone.
        assert!(Calendar::Always.validate().is_ok());
        assert_eq!(Calendar::Always.tz(), Ok(None));
    }

    /// The wire shape is internally tagged, and a `business` row written without a weekly pattern
    /// or holidays still decodes (every optional field carries a serde default).
    #[test]
    fn wire_shape_is_tagged_and_partial_rows_decode() {
        let json = serde_json::to_string(&Calendar::Always).unwrap();
        assert_eq!(json, r#"{"kind":"always"}"#);

        let partial: Calendar =
            serde_json::from_str(r#"{"kind":"business","tz":"Australia/Brisbane"}"#).unwrap();
        assert_eq!(
            partial,
            Calendar::Business {
                tz: "Australia/Brisbane".into(),
                hours: [DayHours::CLOSED; 7],
                holidays: vec![],
            }
        );
    }
}
