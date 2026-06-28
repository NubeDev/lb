//! Append (idempotently) a typed [`TranscriptEvent`] to a job's transcript and advance the cursor —
//! the verb that makes resume both **safe** and **faithful** (jobs scope "idempotent resume",
//! agent-run scope Part 0).
//!
//! The transcript is **append-addressed by step index**: appending at `index` upserts the
//! `steps[index]` slot. So re-running a persisted step on resume (the edge disconnected after the
//! event landed but before the loop advanced) overwrites the same slot with the same event — a
//! no-op, never a duplicate. The cursor advances to `max(cursor, index + 1)` so it only ever moves
//! *past* events that durably landed; replaying an old event does not rewind it.
//!
//! Raw verb — the agent service authorizes (caps + the derived principal) before calling this.

use lb_store::{Store, StoreError};

use super::load::load;
use super::model::Step;
use super::transcript::TranscriptEvent;
use super::update::update;

/// Record `event` at `index` on job `id` in workspace `ws`, idempotently, and advance the cursor
/// past it. Errors if the job does not exist in this workspace (an event for a missing or
/// cross-workspace job is a bug, not a silent create).
pub async fn append_event(
    store: &Store,
    ws: &str,
    id: &str,
    index: u32,
    event: TranscriptEvent,
) -> Result<(), StoreError> {
    let mut job = load(store, ws, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("append_event: no job {id} in ws {ws}")))?;

    let step = Step { index, event };
    // Upsert the slot: replace an existing event at this index (idempotent re-apply), else push.
    match job.steps.iter_mut().find(|s| s.index == index) {
        Some(existing) => *existing = step,
        None => job.steps.push(step),
    }
    // Keep the transcript dense + ordered (a replay may land out of order in principle).
    job.steps.sort_by_key(|s| s.index);
    // Cursor only ever moves forward, past events that landed — replaying an old event never rewinds.
    job.cursor = job.cursor.max(index + 1);

    update(store, ws, &job).await
}
