//! Cancel a running (or suspended) job — the durable **stop** the agent-run cancel hook lands on
//! (agent-run scope Part 0: "a run must be stoppable; ACP `session/cancel` and a UI stop button
//! both need it"). Sets the terminal, **non-restartable** [`JobStatus::Cancelled`]; the transcript
//! is kept for audit/replay (the run is observable after the fact, just not resumable).
//!
//! Idempotent: cancelling an already-cancelled job is a no-op. Cancelling a `Done`/`Failed` job is
//! refused as a caller error — a finished run cannot be retroactively cancelled (that would lie
//! about what happened). Raw verb — the agent service authorizes before calling this.

use lb_store::{Store, StoreError};

use super::load::load;
use super::model::JobStatus;
use super::update::update;

/// Set job `id` in workspace `ws` to [`JobStatus::Cancelled`]. Errors if absent here, or if the job
/// already reached a *successful/failed* terminal state (cancelling a finished run is a caller bug).
/// Cancelling a `Cancelled` job is a no-op.
pub async fn cancel(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    let mut job = load(store, ws, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("cancel: no job {id} in ws {ws}")))?;
    match job.status {
        JobStatus::Cancelled => Ok(()), // idempotent
        JobStatus::Done | JobStatus::Failed => Err(StoreError::Decode(format!(
            "cancel: job {id} already finished ({:?})",
            job.status
        ))),
        JobStatus::Running | JobStatus::Suspended => {
            job.status = JobStatus::Cancelled;
            update(store, ws, &job).await
        }
    }
}
