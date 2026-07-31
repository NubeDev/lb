use thiserror::Error;

/// Errors produced by the schedules crate.
#[derive(Debug, Error)]
pub enum ScheduleError {
    /// The timezone string could not be parsed into a known IANA timezone.
    #[error("unknown timezone: {0}")]
    UnknownTimezone(String),

    /// A time-of-day string was not in valid HH:MM format.
    #[error("invalid time format '{0}': expected HH:MM")]
    InvalidTimeFormat(String),

    /// A datetime string was not in the expected layout.
    #[error("invalid datetime string '{value}': {reason}")]
    InvalidDatetime { value: String, reason: String },
}
