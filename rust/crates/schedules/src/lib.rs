//! Timezone-aware scheduling library.
//!
//! # Quick start
//!
//! ```rust
//! use lb_schedules::{Schedule, ScheduleEvaluator};
//! use chrono::Weekday;
//!
//! let mut local = Schedule::new("hvac", false, "America/New_York", 1).unwrap();
//! local.add_weekly_time_range(Weekday::Mon, "08:00", "17:00").unwrap();
//!
//! let mut eval = ScheduleEvaluator::new();
//! eval.set_local_schedule(local);
//!
//! let state = eval.get_state();
//! println!("active={}, source={}", state.is_active, state.active_source);
//! ```

mod bulk;
mod doc;
mod error;
mod evaluate;
mod evaluator;
mod exceptions;
mod key_order;
mod next_change;
mod parse;
mod schedule;
mod types;
mod validate;
mod weekly;
mod windows;
mod zone;

pub use bulk::{create_bulk_override, create_date_range_override, create_scheduled_override};
pub use doc::{DateWindow, DatedEntry, DocConfig, DocSchedules, ScheduleDoc, WeeklyEntry};
pub use error::ScheduleError;
pub use evaluate::{evaluate_local, Running, ScheduleNow};
pub use evaluator::ScheduleEvaluator;
pub use next_change::next_change;
pub use parse::{minutes_of, parse_local, weekday_from_name};
pub use schedule::{ExceptionEntry, Schedule};
pub use types::{CombinedStats, Entry, ScheduleState, ScheduleStatus, TimeRange, Weekday};
pub use validate::{validate, PayloadLimits};
pub use windows::occurrences;
pub use zone::Zone;
