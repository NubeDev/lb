//! Append (idempotently) a step result to a job's transcript and advance the cursor — the verb
//! that makes resume safe (jobs scope "idempotent resume", agent scope offline/sync).
//!
//! The transcript is **append-addressed by step index**: appending step `i` upserts the `steps[i]`
//! slot. So re-running a persisted step on resume (the edge disconnected after the step landed but
//! before the loop advanced) overwrites the same slot with the same result — a no-op, never a
//! duplicate. The cursor advances to `max(cursor, index + 1)` so it only ever moves *past* steps
//! that durably landed; replaying an old step does not rewind it.
//!
//! Raw verb — the agent service authorizes (caps + the derived principal) before calling this.

use lb_store::{Store, StoreError};

use super::load::load;
use super::model::Step;
use super::update::update;

/// Record step `index`'s `result` on job `id` in workspace `ws`, idempotently, and advance the
/// cursor past it. Errors if the job does not exist in this workspace (a step for a missing or
/// cross-workspace job is a bug, not a silent create).
pub async fn append_step(
    store: &Store,
    ws: &str,
    id: &str,
    index: u32,
    result: impl Into<String>,
) -> Result<(), StoreError> {
    let mut job = load(store, ws, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("append_step: no job {id} in ws {ws}")))?;

    let step = Step {
        index,
        result: result.into(),
    };
    // Upsert the slot: replace an existing step at this index (idempotent re-apply), else push.
    match job.steps.iter_mut().find(|s| s.index == index) {
        Some(existing) => *existing = step,
        None => job.steps.push(step),
    }
    // Cursor only ever moves forward, past steps that landed — replaying an old step never rewinds.
    job.cursor = job.cursor.max(index + 1);

    update(store, ws, &job).await
}
