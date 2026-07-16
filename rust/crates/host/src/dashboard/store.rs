//! The raw store read/write for a [`Dashboard`] record — the (de)serialization seam between the
//! typed model and the generic `lb_store` `data`-envelope. Kept in one file so the table name + the
//! envelope shape have a single owner (FILE-LAYOUT). No authorization here — the verbs gate first.

use lb_store::{read, scan_all, write, Store, StoreError};

use super::model::{Dashboard, TABLE};

/// Read `dashboard:{id}` in `ws`. `None` if absent in this namespace (the hard wall) — a tombstoned
/// record still deserializes (callers treat `deleted` as absent).
pub async fn read_dashboard(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<Dashboard>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(v) => {
            let d: Dashboard =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(d))
        }
        None => Ok(None),
    }
}

/// UPSERT `dashboard` at `dashboard:{id}` in `ws` (create+update; idempotent on the id).
pub async fn write_dashboard(store: &Store, ws: &str, d: &Dashboard) -> Result<(), StoreError> {
    let value = serde_json::to_value(d).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &d.id, &value).await
}

/// Scan every dashboard in `ws` (id-ordered, drained past the one-page cap). The roster read — the
/// caller then filters by gate-3 visibility and drops tombstones.
pub async fn scan_dashboards(store: &Store, ws: &str) -> Result<Vec<Dashboard>, StoreError> {
    let rows = scan_all(store, ws, TABLE).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Records written via `lb_store::write` carry a `{ data: ... }` envelope (the same one `read`
        // unwraps); `scan` returns the whole record, so unwrap the envelope to get the dashboard.
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        let d: Dashboard =
            serde_json::from_value(inner).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(d);
    }
    Ok(out)
}
