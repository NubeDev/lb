//! Persist a mutated job back to its `job:{id}` row — the write half that `append_step` and
//! `complete` share. Idempotent on `job.id` (an upsert), workspace-namespaced (README §7). Kept
//! `pub(crate)`: callers mutate a loaded [`Job`] then hand it here; the public verbs are the
//! intent-named ones (`append_step`, `complete`), not a bare "write whatever".

use lb_store::{write_locked as write, Store, StoreError};

use super::model::Job;
use super::TABLE;

/// Upsert the (mutated) `job` back into workspace `ws`. The whole record is rewritten — at S5
/// scale (one running session) a read-modify-write is simpler and clearer than a field patch.
pub(crate) async fn update(store: &Store, ws: &str, job: &Job) -> Result<(), StoreError> {
    let value = serde_json::to_value(job).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &job.id, &value).await
}
