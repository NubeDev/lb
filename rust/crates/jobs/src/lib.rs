//! SurrealDB-native durable jobs — the resumable remote-workflow session (README §6.9, jobs
//! scope). S5 builds the **resumable-session subset** the central agent needs: a job is a record,
//! its transcript is append-addressed, and resume is idempotent.
//!
//! This crate is the *store side*: the [`Job`] record + the raw `lb_store` verbs that persist and
//! resume it, all workspace-namespaced (the hard wall, §7). It holds **no authorization** — exactly
//! like `lb_inbox`/`lb_assets`, these are the raw verbs the host's agent service runs *after*
//! `caps::check` under the derived (intersected) principal (capability-first, §3.5). No separate
//! datastore (§3.2): jobs persist in the one embedded SurrealDB on every node.
//!
//! The atomic-claim queue (multi-worker contention, `run_at` scheduling, backoff, cron) is
//! deferred past S5 (jobs scope) — the single hub-hosted agent session has no contending workers.
//!
//! Verbs, one per file (FILE-LAYOUT §3):
//! - [`create`] — start a session (idempotent on `id`).
//! - [`load`] — read it back (the resume read; `None` cross-workspace — isolation).
//! - [`append_step`] — record step `i`'s result + advance the cursor (idempotent resume).
//! - [`complete`] — set the terminal status.

mod append_step;
mod complete;
mod create;
mod load;
mod model;
mod update;

pub use append_step::append_step;
pub use complete::complete;
pub use create::create;
pub use load::load;
pub use model::{Job, JobStatus, Step};

/// The job table within a workspace namespace. One place owns the name so every verb agrees.
pub(crate) const TABLE: &str = "job";
