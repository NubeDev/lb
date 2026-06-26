//! Mark a job finished — set its terminal [`JobStatus`] (`Done` or `Failed`). The loop calls this
//! once the model returns no more tool calls (or the ceiling is hit), or on an unrecoverable error
//! (jobs scope, agent scope). Idempotent: completing a `Done` job again sets the same status.
//!
//! Raw verb — the agent service authorizes before calling this.

use lb_store::{Store, StoreError};

use super::load::load;
use super::model::JobStatus;
use super::update::update;

/// Set job `id`'s terminal `status` in workspace `ws`. Errors if the job is absent here (a
/// completion for a missing or cross-workspace job is a bug). `status` must be terminal
/// (`Done`/`Failed`); passing `Running` is a caller error but harmless (it just stays running).
pub async fn complete(
    store: &Store,
    ws: &str,
    id: &str,
    status: JobStatus,
) -> Result<(), StoreError> {
    let mut job = load(store, ws, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("complete: no job {id} in ws {ws}")))?;
    job.status = status;
    update(store, ws, &job).await
}
