//! Write a JSON value to `<table>:<id>` within a workspace's namespace.
//!
//! The namespace is selected from `ws` before the upsert, so a write can only land in its own
//! workspace (README §7). The host JSON is bound as a `$data` param and wrapped under the
//! `data` field (see `record.rs`) so SurrealDB stores it cleanly. The caller is expected to
//! have passed `caps::check` first — this is the raw store verb, not the authorization point.

use serde_json::Value;

use crate::open::{Store, StoreError};

/// Upsert `value` at `table:id` in workspace `ws`.
pub async fn write(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
    value: &Value,
) -> Result<(), StoreError> {
    let db = store.use_ws(ws).await?;
    db.query("UPSERT type::thing($tb, $id) CONTENT { data: $data }")
        .bind(("tb", table.to_string()))
        .bind(("id", id.to_string()))
        .bind(("data", value.clone()))
        .await?
        .check()?;
    Ok(())
}
