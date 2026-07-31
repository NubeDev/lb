use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A concrete start/stop instant pair, used for evaluated results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub start: DateTime<Utc>,
    pub stop: DateTime<Utc>,
}

/// A time-of-day range stored as HH:MM strings.
///
/// `spans_midnight` is set automatically when `stop` is earlier in the day than `start`
/// (e.g. "22:00" → "04:00").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start time in HH:MM format.
    pub start: String,
    /// Stop time in HH:MM format.
    pub stop: String,
    /// True when the window crosses the 00:00 boundary.
    pub spans_midnight: bool,
}

/// Evaluated status for a single schedule entry (weekly or exception).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleStatus {
    pub is_active: bool,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub next_start: Option<DateTime<Utc>>,
    pub next_stop: Option<DateTime<Utc>>,
    /// Originating source: "weekly", "weekly-midnight-span", "exception-<type>", etc.
    pub source: String,
    pub priority: i32,
}

/// Complete evaluated state returned by `ScheduleEvaluator::get_state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleState {
    pub is_active: bool,
    /// Human-readable source of the winning entry, e.g. "hvac-weekly".
    pub active_source: String,
    pub next_transition: Option<DateTime<Utc>>,
    /// Name of the schedule that is currently winning.
    pub current_schedule: Option<String>,
    pub active_priority: i32,
}

/// Combined weekly + exception status for a single `Schedule`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedStats {
    pub weekly_active: bool,
    pub exception_active: bool,
    pub weekly: Vec<ScheduleStatus>,
    pub exception: Vec<ScheduleStatus>,
    pub priority: i32,
}

/// Weekday enum, mirroring chrono::Weekday but usable in maps.
pub use chrono::Weekday;
