//! `emit_effect` — the workflow's **transactional must-deliver write**: append a step to the coding
//! job AND enqueue the external effect in ONE transaction (the transactional-outbox pattern, outbox
//! scope, vision §3 step 8). This is how every external effect (PR, comment, notify, sync) leaves
//! the job — never a direct call, never raw pub/sub.
//!
//! The domain change here is the job's transcript advancing (a durable record of "the job decided to
//! do X"); the effect is the must-deliver intent. `lb_outbox::enqueue` writes both in one
//! `BEGIN…COMMIT`, so the job can never record an effect it failed to schedule, nor schedule one it
//! failed to record. The relay then delivers at-least-once with retry; the receiver dedups on the
//! effect's `idempotency_key` (which the caller supplies, stable per domain change).
//!
//! Raw-ish verb at the host layer — the caller (`start_job`) has already passed the workflow gate.

use lb_jobs::{load, Job, Step};
use lb_outbox::Effect;
use lb_store::{Store, StoreError};

/// Append step `index`'s `note` to job `job_id` AND enqueue `effect`, atomically, in workspace `ws`.
/// Idempotent: re-emitting upserts the same job (slot `index`) and the same `effect.id`. Errors if
/// the job is absent here (an effect for a missing/cross-ws job is a bug).
pub async fn emit_effect(
    store: &Store,
    ws: &str,
    job_id: &str,
    index: u32,
    note: &str,
    effect: &Effect,
) -> Result<(), StoreError> {
    let mut job: Job = load(store, ws, job_id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("emit_effect: no job {job_id} in ws {ws}")))?;

    // Advance the transcript (the domain change) — append-addressed + idempotent, exactly like
    // `lb_jobs::append_step`, but the write is deferred into the transaction below.
    let step = Step {
        index,
        result: note.to_string(),
    };
    match job.steps.iter_mut().find(|s| s.index == index) {
        Some(existing) => *existing = step,
        None => job.steps.push(step),
    }
    job.cursor = job.cursor.max(index + 1);

    let job_value = serde_json::to_value(&job).map_err(|e| StoreError::Decode(e.to_string()))?;
    // The job step AND the effect commit together — or neither does (outbox scope).
    lb_outbox::enqueue(store, ws, "job", job_id, &job_value, effect).await
}
