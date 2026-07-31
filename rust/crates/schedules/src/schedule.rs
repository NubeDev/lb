use std::collections::HashMap;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::error::ScheduleError;
use crate::types::{TimeRange, Weekday};

/// A date/time-based exception that overrides the weekly schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceptionEntry {
    pub start: DateTime<Utc>,
    pub stop: DateTime<Utc>,
    /// Higher value wins when exceptions overlap.
    pub priority: i32,
    /// Semantic label: "override", "temporary", "holiday", "maintenance", etc.
    pub exception_type: String,
}

/// A complete scheduling configuration.
///
/// Build one with `Schedule::new`, populate it via `add_weekly_time_range` and
/// `add_exception`, then evaluate it with `check_combined` or pass it to a
/// `ScheduleEvaluator` for priority-based multi-schedule resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// Unique name used in status strings.
    pub name: String,
    /// Priority used when comparing against other schedules in an evaluator.
    /// Conventions: 1 = local, 10 = master, 20 = emergency.
    pub priority: i32,
    /// When true all times are interpreted as UTC; `timezone` is ignored.
    pub use_utc: bool,
    /// IANA timezone string (e.g. "America/New_York"). Ignored when `use_utc` is true.
    pub timezone: String,
    /// Weekly time ranges keyed by weekday.
    pub weekly: HashMap<Weekday, Vec<TimeRange>>,
    /// Date-based exceptions, kept sorted by start time.
    pub exceptions: Vec<ExceptionEntry>,
    #[serde(skip)]
    pub(crate) tz: Option<Tz>,
}

impl Schedule {
    /// Create a new schedule.
    ///
    /// `timezone` must be a valid IANA name (e.g. `"America/Chicago"`) unless
    /// `use_utc` is `true`, in which case it is ignored.
    pub fn new(
        name: impl Into<String>,
        use_utc: bool,
        timezone: impl Into<String>,
        priority: i32,
    ) -> Result<Self, ScheduleError> {
        let timezone = timezone.into();
        let tz = if !use_utc && !timezone.is_empty() {
            let parsed: Tz = timezone
                .parse()
                .map_err(|_| ScheduleError::UnknownTimezone(timezone.clone()))?;
            Some(parsed)
        } else {
            None
        };

        Ok(Self {
            name: name.into(),
            priority,
            use_utc,
            timezone,
            weekly: HashMap::new(),
            exceptions: Vec::new(),
            tz,
        })
    }

    /// Returns the current time adjusted for this schedule's timezone setting.
    pub(crate) fn now(&self) -> DateTime<Utc> {
        // All internal logic works in UTC; timezone context is applied during
        // weekday/time-of-day calculations via `chrono-tz`.
        Utc::now()
    }

    /// Parse an HH:MM string into (hour, minute).
    pub(crate) fn parse_hhmm(s: &str) -> Result<(u32, u32), ScheduleError> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(ScheduleError::InvalidTimeFormat(s.to_string()));
        }
        let h = parts[0]
            .parse::<u32>()
            .map_err(|_| ScheduleError::InvalidTimeFormat(s.to_string()))?;
        let m = parts[1]
            .parse::<u32>()
            .map_err(|_| ScheduleError::InvalidTimeFormat(s.to_string()))?;
        if h > 23 || m > 59 {
            return Err(ScheduleError::InvalidTimeFormat(s.to_string()));
        }
        Ok((h, m))
    }
}
