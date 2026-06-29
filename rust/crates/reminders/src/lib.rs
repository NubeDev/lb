//! `lb-reminders` — the store side of the **reminders** slice (reminders scope). A reminder is a
//! durable, workspace-scoped schedule that fires one action when it comes due. This crate owns the
//! record + the raw store verbs + the one new mechanical piece (cron "next after T" on the injected
//! logical clock via `croner`). It holds **no authorization** and no host seams (bus/jobs/outbox) —
//! exactly like `lb_inbox`/`lb_outbox`/`lb_jobs`, these are the raw verbs the host `reminder`
//! service runs *after* `caps::check` (capability-first §3.5).
//!
//! Verbs, one per file (FILE-LAYOUT §3):
//! - [`save`] — upsert a reminder (create/update share it; idempotent on `id`).
//! - [`load`] — read by id (`None` cross-workspace — isolation).
//! - [`list`] / [`due`] — the `list` read + the reactor's due-now scan.
//! - [`next_after`] — the cron "next after T" math on the injected clock.

mod error;
mod load;
mod model;
mod next_after;
mod save;
mod scan;

pub use error::ReminderError;
pub use load::load;
pub use model::{Action, Reminder, ReminderStatus};
pub use next_after::{is_valid, next_after};
pub use save::save;
pub use scan::{due, list};

/// The reminder table within a workspace namespace. One place owns the name so every verb agrees.
pub(crate) const TABLE: &str = "reminder";
