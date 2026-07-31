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
mod error;
mod evaluator;
mod exceptions;
mod schedule;
mod types;
mod weekly;

pub use bulk::{create_bulk_override, create_date_range_override, create_scheduled_override};
pub use error::ScheduleError;
pub use evaluator::ScheduleEvaluator;
pub use schedule::{ExceptionEntry, Schedule};
pub use types::{CombinedStats, Entry, ScheduleState, ScheduleStatus, TimeRange, Weekday};
