//! List the JSON values of every record in `<table>` whose `data.<field>` equals a value,
//! within a workspace's namespace — the query behind a channel/inbox view.
//!
//! Like `read`, the namespace is selected from `ws` first, so a list for workspace A can
//! physically only see namespace A's rows — the workspace wall holds for queries, not just
//! point reads (README §7). Results are the unwrapped host `data` values (see `record.rs`).
//!
//! This is a pure *filter*: it does NOT order. Ordering is the caller's concern, because the
//! generic store has no business knowing where the order key lives inside the opaque `data`
//! (and SurrealDB rejects `ORDER BY data.ts` when only `data` is projected — see
//! debugging/store/order-by-needs-selected-idiom.md). The inbox sorts by `ts` itself.

use serde_json::Value;

use crate::open::{Store, StoreError};
use crate::record::Record;

/// Return the `data` value of every `table` row in workspace `ws` whose `data.<field>`
/// equals `value`, ordered by `data.ts` ascending. Empty if none — never another
/// workspace's rows.
pub async fn list(
    store: &Store,
    ws: &str,
    table: &str,
    field: &str,
    value: &str,
) -> Result<Vec<Value>, StoreError> {
    // `field` is interpolated into the query text, so it must be a bare column identifier —
    // never caller input. Guard the class shut: only `[a-z0-9_]` is accepted. `value` is
    // always bound as a param, so caller input never reaches the query text.
    if field.is_empty()
        || !field
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(StoreError::Decode(format!(
            "invalid field identifier: {field:?}"
        )));
    }
    let db = store.use_ws(ws).await?;
    let query = format!("SELECT data FROM type::table($tb) WHERE data.{field} = $value");
    let mut response = db
        .query(query)
        .bind(("tb", table.to_string()))
        .bind(("value", value.to_string()))
        .await?
        .check()?;
    let records: Vec<Record> = response
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(records.into_iter().map(|r| r.data).collect())
}
