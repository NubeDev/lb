//! Mark a job **suspended** on a durable human decision (agent-run scope Part 2). The loop calls
//! this after it has recorded the [`SuspensionOpened`](crate::TranscriptEvent) event and written
//! the `agent_decision` record — so the durable pause is complete *before* the status flips and the
//! turn ends. `Suspended` is terminal for the current turn (the connection need not be held) but
//! **restartable**: when the decision settles, the reactor resumes from the cursor.
//!
//! Idempotent: suspending an already-suspended job is a no-op. Raw verb — the agent service
//! authorizes before calling this.

use lb_store::{Store, StoreError};

use super::load::load;
use super::model::JobStatus;
use super::update::update;

/// Set job `id` in workspace `ws` to [`JobStatus::Suspended`]. Errors if absent here, or if the job
/// already reached a non-resumable terminal state (suspending a finished/cancelled run is a bug).
pub async fn suspend(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    let mut job = load(store, ws, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("suspend: no job {id} in ws {ws}")))?;
    match job.status {
        JobStatus::Suspended => Ok(()), // idempotent
        JobStatus::Running => {
            job.status = JobStatus::Suspended;
            update(store, ws, &job).await
        }
        terminal => Err(StoreError::Decode(format!(
            "suspend: job {id} is not running ({terminal:?})"
        ))),
    }
}

/// Move a suspended job back to [`JobStatus::Running`] — the reactor calls this when the decision
/// settles and the loop is about to resume. Idempotent on an already-running job; refused on a
/// non-resumable terminal state.
pub async fn unsuspend(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    let mut job = load(store, ws, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("unsuspend: no job {id} in ws {ws}")))?;
    match job.status {
        JobStatus::Running => Ok(()),
        JobStatus::Suspended => {
            job.status = JobStatus::Running;
            update(store, ws, &job).await
        }
        terminal => Err(StoreError::Decode(format!(
            "unsuspend: job {id} is not suspended ({terminal:?})"
        ))),
    }
}
