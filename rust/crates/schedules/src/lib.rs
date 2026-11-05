//! Timezone-aware schedules in the rubix-ce document shape (`weekly` / `events` / `exception` maps,
//! the same key names rubix-ce and the box use), evaluated by the box's rules.
//!
//! Everything here is pure and clock-injected: [`evaluate_local`] and [`next_change`] take a local
//! wall time, and [`Zone`] converts between instants and wall times (DST included). [`validate`]
//! holds the save-time rules.

mod doc;
mod error;
mod evaluate;
mod key_order;
mod next_change;
mod parse;
mod validate;
mod windows;
mod zone;

pub use doc::{DateWindow, DatedEntry, DocConfig, DocSchedules, ScheduleDoc, WeeklyEntry};
pub use error::ScheduleError;
pub use evaluate::{evaluate_local, Running, ScheduleNow};
pub use next_change::next_change;
pub use parse::{minutes_of, parse_local, weekday_from_name};
pub use validate::{validate, PayloadLimits};
pub use windows::occurrences;
pub use zone::Zone;
