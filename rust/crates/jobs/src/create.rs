//! Create (persist) a new job record into the workspace namespace — the start of a durable
//! session (jobs scope, agent scope). Idempotent on the job `id`: re-`create` upserts the same
//! `job:{id}` row, so a retried "start the session" never forks two sessions. The namespace is
//! selected from `ws` by `lb_store`, so a job can only land in its own workspace (README §7).
//!
//! Raw verb — authorization is the host's job (the agent service is the caps chokepoint).

use lb_store::{write_locked as write, Store, StoreError};

use super::model::Job;
use super::TABLE;

/// Upsert `job` into workspace `ws`'s job table. Idempotent on `job.id`.
pub async fn create(store: &Store, ws: &str, job: &Job) -> Result<(), StoreError> {
    let value = serde_json::to_value(job).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &job.id, &value).await
}
