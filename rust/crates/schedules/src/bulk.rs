use chrono::{DateTime, Utc};

use crate::schedule::{ExceptionEntry, Schedule};

/// Create a temporary exception starting now and lasting `duration`.
///
/// `priority` of `0` defaults to `100` — high enough to beat most schedules.
pub fn create_bulk_override(duration: chrono::Duration, priority: i32) -> ExceptionEntry {
    let priority = if priority == 0 { 100 } else { priority };
    let now = Utc::now();
    ExceptionEntry {
        start: now,
        stop: now + duration,
        priority,
        exception_type: "bulk-override".to_string(),
    }
}

/// Create an override that starts at a specific instant.
pub fn create_scheduled_override(
    start: DateTime<Utc>,
    duration: chrono::Duration,
    priority: i32,
) -> ExceptionEntry {
    let priority = if priority == 0 { 100 } else { priority };
    ExceptionEntry {
        start,
        stop: start + duration,
        priority,
        exception_type: "scheduled-override".to_string(),
    }
}

/// Create an override spanning an explicit date range.
pub fn create_date_range_override(
    start: DateTime<Utc>,
    stop: DateTime<Utc>,
    priority: i32,
) -> ExceptionEntry {
    let priority = if priority == 0 { 100 } else { priority };
    ExceptionEntry {
        start,
        stop,
        priority,
        exception_type: "date-range-override".to_string(),
    }
}

impl Schedule {
    /// Apply a pre-built `ExceptionEntry` directly.
    pub fn apply_bulk_exception(&mut self, ex: ExceptionEntry) {
        self.add_exception(ex.start, ex.stop, ex.exception_type.clone(), ex.priority);
    }

    /// Add an active exception that forces the schedule OFF for `duration`.
    ///
    /// Note: whether "off" is meaningful depends on higher-level semantics;
    /// this simply inserts a high-priority exception marked "force-off".
    pub fn turn_off_for_duration(&mut self, duration: chrono::Duration) {
        let mut ex = create_bulk_override(duration, 100);
        ex.exception_type = "force-off".to_string();
        self.apply_bulk_exception(ex);
    }

    /// Add an active exception that forces the schedule ON for `duration`.
    pub fn turn_on_for_duration(&mut self, duration: chrono::Duration) {
        let mut ex = create_bulk_override(duration, 100);
        ex.exception_type = "force-on".to_string();
        self.apply_bulk_exception(ex);
    }

    /// Schedule a maintenance window starting at `start` lasting `duration`.
    pub fn schedule_downtime(&mut self, start: DateTime<Utc>, duration: chrono::Duration) {
        let mut ex = create_scheduled_override(start, duration, 90);
        ex.exception_type = "maintenance".to_string();
        self.apply_bulk_exception(ex);
    }

    /// Add an all-day holiday exception for the given date.
    pub fn add_holiday(&mut self, name: &str, date: DateTime<Utc>) {
        let start = date.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let stop = start + chrono::Duration::hours(24);
        self.add_exception(start, stop, format!("holiday-{name}"), 80);
    }
}
