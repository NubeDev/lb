//! Write a JSON value to `<table>:<id>` within a workspace's namespace.
//!
//! The namespace is selected from `ws` before the upsert, so a write can only land in its own
//! workspace (README §7). The host JSON is bound as a `$data` param and wrapped under the
//! `data` field (see `record.rs`) so SurrealDB stores it cleanly. The caller is expected to
//! have passed `caps::check` first — this is the raw store verb, not the authorization point.
//!
//! Every write also **bumps a store-managed monotonic `rev`**: the new `rev` is the prior
//! record's `rev + 1` (or [`FIRST_REV`] on first write), computed server-side inside the same
//! statement so two concurrent writers can never read-modify-write the same rev. This is the
//! optimistic-concurrency token the undo journal's conditional restore tests against
//! (`docs/scope/undo/undo-scope.md`).

use serde_json::Value;

use crate::open::{Store, StoreError};
use crate::record::FIRST_REV;

/// Upsert `value` at `table:id` in workspace `ws`, bumping the record's monotonic `rev`.
pub async fn write(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
    value: &Value,
) -> Result<(), StoreError> {
    let db = store.use_ws(ws).await?;
    // `rev` is derived server-side from the record's own prior rev so it is monotonic without a
    // separate read: `(rev OR 0) + 1`. A brand-new record has no prior `rev`, so it lands at 1.
    db.query(
        "UPSERT type::thing($tb, $id) CONTENT { \
            data: $data, \
            rev: (type::thing($tb, $id).rev ?? ($first - 1)) + 1 \
         } RETURN NONE",
    )
    .bind(("tb", table.to_string()))
    .bind(("id", id.to_string()))
    .bind(("data", value.clone()))
    .bind(("first", FIRST_REV))
    .await?
    .check()?;
    Ok(())
}
