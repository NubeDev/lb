//! The raw store read/write for a [`Report`] record — the (de)serialization seam between the typed
//! model and the generic `lb_store` `data`-envelope (mirrors `panel::store`). No authorization here
//! — the verbs gate first.

use lb_store::{read, scan_all, write, Store, StoreError};

use super::model::{Report, TABLE};

/// Read `report:{id}` in `ws`. `None` if absent in this namespace (the hard wall).
pub async fn read_report(store: &Store, ws: &str, id: &str) -> Result<Option<Report>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(v) => {
            let r: Report =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(r))
        }
        None => Ok(None),
    }
}

/// UPSERT `report` at `report:{id}` in `ws` (create+update; idempotent on the id).
pub async fn write_report(store: &Store, ws: &str, r: &Report) -> Result<(), StoreError> {
    let value = serde_json::to_value(r).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &r.id, &value).await
}

/// Scan every report in `ws` (id-ordered, drained past the one-page cap). The roster read — the
/// caller filters by gate-3 visibility and drops tombstones.
pub async fn scan_reports(store: &Store, ws: &str) -> Result<Vec<Report>, StoreError> {
    let rows = scan_all(store, ws, TABLE).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Records written via `lb_store::write` carry a `{ data: ... }` envelope; unwrap it.
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        let r: Report =
            serde_json::from_value(inner).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(r);
    }
    Ok(out)
}
