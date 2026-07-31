use chrono::{DateTime, Datelike, Month, Utc};
use chrono_tz::Tz;

use crate::error::ScheduleError;
use crate::schedule::{ExceptionEntry, Schedule};
use crate::types::ScheduleStatus;

impl Schedule {
    /// Add a date/time-based exception.
    ///
    /// `priority` of `0` defaults to `self.priority + 1` (beats the base schedule).
    pub fn add_exception(
        &mut self,
        start: DateTime<Utc>,
        stop: DateTime<Utc>,
        exception_type: impl Into<String>,
        priority: i32,
    ) {
        let priority = if priority == 0 {
            self.priority + 1
        } else {
            priority
        };

        self.exceptions.push(ExceptionEntry {
            start,
            stop,
            priority,
            exception_type: exception_type.into(),
        });

        // Keep sorted by start time for consistent iteration
        self.exceptions.sort_by_key(|e| e.start);
    }

    /// Add an exception parsed from `"YYYY-MM-DD HH:MM:SS"` strings.
    pub fn add_exception_from_strings(
        &mut self,
        start: &str,
        stop: &str,
        exception_type: impl Into<String>,
        priority: i32,
    ) -> Result<(), ScheduleError> {
        let parse = |s: &str| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc())
                .map_err(|e| ScheduleError::InvalidDatetime {
                    value: s.to_string(),
                    reason: e.to_string(),
                })
        };

        let mut start_dt = parse(start)?;
        let mut stop_dt = parse(stop)?;

        // Shift into the schedule's timezone if needed
        if let Some(tz) = self.tz {
            start_dt = shift_to_tz(start_dt, tz);
            stop_dt = shift_to_tz(stop_dt, tz);
        }

        self.add_exception(start_dt, stop_dt, exception_type, priority);
        Ok(())
    }

    /// Add a yearly recurring exception (e.g. a public holiday).
    ///
    /// The exception is added for the current year and the next year so that
    /// schedules survive a year boundary without needing re-population.
    pub fn add_yearly_exception(
        &mut self,
        month: Month,
        day: u32,
        start_time: &str,
        stop_time: &str,
        exception_type: impl Into<String>,
    ) -> Result<(), ScheduleError> {
        let (sh, sm) = Self::parse_hhmm(start_time)?;
        let (eh, em) = Self::parse_hhmm(stop_time)?;
        let now = self.now();
        let exception_type = exception_type.into();

        for year_offset in 0..=1_i32 {
            let year = now.year() + year_offset;
            let start = build_utc(year, month.number_from_month(), day, sh, sm, self.tz)
                .ok_or_else(|| ScheduleError::InvalidDatetime {
                    value: format!("{year}-{:02}-{day:02}", month.number_from_month()),
                    reason: "date out of range".to_string(),
                })?;
            let mut stop = build_utc(year, month.number_from_month(), day, eh, em, self.tz)
                .ok_or_else(|| ScheduleError::InvalidDatetime {
                    value: format!("{year}-{:02}-{day:02}", month.number_from_month()),
                    reason: "date out of range".to_string(),
                })?;

            if stop <= start {
                stop = stop + chrono::Duration::hours(24);
            }

            self.add_exception(start, stop, exception_type.clone(), self.priority + 5);
        }

        Ok(())
    }

    /// Evaluate all exceptions and return their status.
    pub fn check_exceptions(&self) -> Vec<ScheduleStatus> {
        let now = self.now();
        self.exceptions
            .iter()
            .map(|ex| evaluate_exception(ex, now))
            .collect()
    }

    /// Returns `true` when at least one exception is currently active.
    pub fn exception_active(&self) -> bool {
        let now = self.now();
        self.exceptions
            .iter()
            .any(|ex| now > ex.start && now < ex.stop)
    }

    /// Returns the highest-priority active exception, or `None`.
    pub fn active_exception(&self) -> Option<ScheduleStatus> {
        let now = self.now();
        self.exceptions
            .iter()
            .filter(|ex| now > ex.start && now < ex.stop)
            .map(|ex| evaluate_exception(ex, now))
            .max_by_key(|s| s.priority)
    }

    /// Remove exceptions whose stop time is more than `older_than` in the past.
    pub fn clean_old_exceptions(&mut self, older_than: chrono::Duration) {
        let cutoff = self.now() - older_than;
        self.exceptions.retain(|ex| ex.stop > cutoff);
    }
}

fn evaluate_exception(ex: &ExceptionEntry, now: DateTime<Utc>) -> ScheduleStatus {
    let is_active = now > ex.start && now < ex.stop;

    let (next_start, next_stop) = if now > ex.stop {
        // Past exception — no future occurrence
        (None, None)
    } else {
        (Some(ex.start), Some(ex.stop))
    };

    ScheduleStatus {
        is_active,
        start_date: ex.start,
        end_date: ex.stop,
        next_start,
        next_stop,
        source: format!("exception-{}", ex.exception_type),
        priority: ex.priority,
    }
}

/// Interpret a UTC `DateTime` as if it were local time in `tz`.
///
/// Used when parsing naive date strings that were intended to be in the
/// schedule's configured timezone.
fn shift_to_tz(dt: DateTime<Utc>, tz: Tz) -> DateTime<Utc> {
    use chrono::TimeZone;
    let naive = dt.naive_utc();
    tz.from_local_datetime(&naive)
        .single()
        .unwrap_or_else(|| {
            tz.from_local_datetime(&naive)
                .earliest()
                .expect("tz conversion failed")
        })
        .with_timezone(&Utc)
}

fn build_utc(
    year: i32,
    month: u32,
    day: u32,
    h: u32,
    m: u32,
    tz: Option<Tz>,
) -> Option<DateTime<Utc>> {
    use chrono::TimeZone;
    if let Some(tz) = tz {
        tz.with_ymd_and_hms(year, month, day, h, m, 0)
            .single()
            .map(|dt| dt.with_timezone(&Utc))
    } else {
        Utc.with_ymd_and_hms(year, month, day, h, m, 0).single()
    }
}
