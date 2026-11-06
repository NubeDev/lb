//! The **global schedule** record and its store surface: `schedule.save` / `get` / `list` /
//! `delete` / `evaluate`, plus the resolution seam the `schedule` flow node reads through.
//!
//! A schedule is a first-class, workspace-scoped record in the rubix-ce document shape: one record is
//! referenced by any number of flow nodes and dashboard widgets and edited in one place. State only
//! (rule 3): evaluation is `lb-schedules` (pure, clock-injected), firing is the reactor. One verb per
//! file (FILE-LAYOUT).

mod authorize;
mod delete;
mod evaluate;
mod read;
mod record;
mod refusal;
mod save;

pub use delete::schedule_delete;
pub use evaluate::schedule_evaluate;
pub use read::{read_schedule_internal, schedule_get, schedule_list};
pub use record::{ScheduleEvaluation, ScheduleListing, ScheduleRecord, UnreadableSchedule};
pub use refusal::{SCHEDULE_EXISTS, SCHEDULE_LEGACY_FIELDS, SCHEDULE_OLD_SHAPE, SCHEDULE_STALE};
pub use save::{schedule_save, SavedSchedule, ScheduleSave};

/// The SurrealDB table holding workspace-scoped global schedule records.
pub const SCHEDULE_TABLE: &str = "schedule";
