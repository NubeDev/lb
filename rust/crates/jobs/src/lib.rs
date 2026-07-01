//! SurrealDB-native durable jobs — the resumable remote-workflow session (README §6.9, jobs
//! scope). S5 built the **resumable-session subset** the central agent needs; agent-run scope
//! **Part 0** made the transcript *typed* (a [`TranscriptEvent`] per step, not an opaque `String`)
//! so a suspended run can be rehydrated faithfully, and added the **cancel** + **suspend** verbs.
//!
//! This crate is the *store side*: the [`Job`] record + the raw `lb_store` verbs that persist and
//! resume it, all workspace-namespaced (the hard wall, §7). It holds **no authorization** — exactly
//! like `lb_inbox`/`lb_assets`, these are the raw verbs the host's agent service runs *after*
//! `caps::check` under the derived (intersected) principal (capability-first, §3.5). No separate
//! datastore (§3.2): jobs persist in the one embedded SurrealDB on every node. It also stays in the
//! lowest layer — no deps on protocols/gateway/wasm — so the [`TranscriptEvent`] vocabulary the
//! `RunEvent` projection (Part 1) reads can live here without an import cycle.
//!
//! The atomic-claim queue (multi-worker contention, `run_at` scheduling, backoff, cron) is
//! deferred past S5 (jobs scope) — the single hub-hosted agent session has no contending workers.
//!
//! Verbs, one per file (FILE-LAYOUT §3):
//! - [`create`] — start a session (idempotent on `id`).
//! - [`load`] — read it back (the resume read; `None` cross-workspace — isolation).
//! - [`pending`] — list still-running jobs of a `kind` (the background reactor's drain scan).
//! - [`append_event`] — record the typed event at step `i` + advance the cursor (idempotent resume).
//! - [`complete`] — set a terminal status (`Done`/`Failed`).
//! - [`cancel`] — the durable stop (`Cancelled`, non-restartable; Part 0 cancel hook).
//! - [`suspend`] / [`unsuspend`] — the durable pause/wake on a human decision (Part 2).

mod append_event;
mod cancel;
mod complete;
mod create;
mod load;
mod model;
mod pending;
mod suspend;
mod transcript;
mod update;

pub use append_event::append_event;
pub use cancel::cancel;
pub use complete::complete;
pub use create::create;
pub use load::load;
pub use model::{Job, JobStatus, Step};
pub use pending::pending;
pub use suspend::{suspend, unsuspend};
pub use transcript::{SuspensionDecision, TranscriptEvent, TRANSCRIPT_SCHEMA_VERSION};

/// The job table within a workspace namespace. One place owns the name so every verb agrees.
pub(crate) const TABLE: &str = "job";
