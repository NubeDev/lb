//! Persist (upsert) a reminder into the workspace's namespace. Idempotent on `id` (a re-`create` or
//! an `update` upserts the same `reminder:{id}` row). Raw store verb — the host `reminder` service
//! runs `caps::check` before this (capability-first §3.5).

use lb_store::{write, Store, StoreError};

use crate::model::Reminder;

/// Upsert `reminder` into workspace `ws`. Idempotent on `reminder.id`.
pub async fn save(store: &Store, ws: &str, reminder: &Reminder) -> Result<(), StoreError> {
    let value = serde_json::to_value(reminder).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, super::TABLE, &reminder.id, &value).await
}
