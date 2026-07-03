//! Create (persist) a new job record into the workspace namespace — the start of a durable
//! session (jobs scope, agent scope). Idempotent on the job `id`: re-`create` upserts the same
//! `job:{id}` row, so a retried "start the session" never forks two sessions. The namespace is
//! selected from `ws` by `lb_store`, so a job can only land in its own workspace (README §7).
//!
//! Raw verb — authorization is the host's job (the agent service is the caps chokepoint).

use lb_store::{write_locked as write, Store, StoreError};

use super::model::Job;
use super::schema::define_job_index;
use super::TABLE;

/// Upsert `job` into workspace `ws`'s job table. Idempotent on `job.id`.
///
/// Ensures the `(kind, status)` drain index exists first (first-touch schema, per the prefs/tags
/// convention — there is no global boot schema pass), so the reactor's [`pending`](crate::pending)
/// query is an index lookup, not a full scan, from the very first job a workspace ever creates.
pub async fn create(store: &Store, ws: &str, job: &Job) -> Result<(), StoreError> {
    define_job_index(store, ws).await?;
    let value = serde_json::to_value(job).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &job.id, &value).await
}
