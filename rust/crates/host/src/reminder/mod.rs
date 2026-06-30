//! The **reminder** host service — the capability chokepoint over `lb_reminders` (reminders scope).
//! A reminder is a durable, workspace-scoped schedule that fires one action when it comes due. The
//! CRUD verbs are bounded, always-fast single-record writes (synchronous, NOT jobs); the FIRING is
//! the job — the reactor enqueues one `kind="reminder-fire"` lb-jobs job per due instant.
//!
//! One verb per file (FILE-LAYOUT §3):
//!   - `create` / `update` / `delete` / `get` / `list` — the gated CRUD verbs (the scope's MCP
//!     surface; live-feed + batch are explicit non-goals).
//!   - `fire` — dispatch one firing's action under the stored principal (the job's body).
//!   - `react` — the durable scan that finds due reminders, enqueues a job, fires, advances.
//!   - `tool` — the `reminder.*` MCP bridge.
//!
//! Holds no durable state of its own (stateless extensions, §3.4): every fact lives in the
//! `reminder:{id}` record + the firing's lb-jobs job + the action's effect (inbox/outbox).

mod authorize;
mod create;
mod delete;
mod fire;
mod get;
mod react;
mod tool;
mod update;

pub use create::reminder_create;
pub use delete::reminder_delete;
pub use fire::{fire_job_id, fire_reminder, FIRE_KIND};
pub use get::{reminder_get, reminder_list};
pub use react::{react_to_reminders, ReactorPass};
pub use tool::call_reminder_tool;
pub use update::{reminder_update, ReminderPatch};

pub use lb_reminders::{Action, Reminder, ReminderError, ReminderStatus};
