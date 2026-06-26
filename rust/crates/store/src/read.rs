//! Read a JSON value from `<table>:<id>` within a workspace's namespace.
//!
//! Because the namespace is selected from `ws`, a read for workspace A returns `None` for a
//! record that lives in workspace B's namespace — even with the same `table:id`. That is the
//! workspace-isolation guarantee at the store layer (README §7). The stored `data` field is
//! unwrapped back to the host's `serde_json::Value` (see `record.rs`).

use serde_json::Value;

use crate::open::{Store, StoreError};
use crate::record::Record;

/// Fetch `table:id` from workspace `ws`. `None` if absent in *this* namespace.
pub async fn read(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
) -> Result<Option<Value>, StoreError> {
    let db = store.use_ws(ws).await?;
    let mut response = db
        .query("SELECT data FROM ONLY type::thing($tb, $id)")
        .bind(("tb", table.to_string()))
        .bind(("id", id.to_string()))
        .await?
        .check()?;
    let record: Option<Record> = response
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(record.map(|r| r.data))
}
