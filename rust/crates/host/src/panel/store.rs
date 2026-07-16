//! The raw store read/write for a [`Panel`] record — the (de)serialization seam between the typed
//! model and the generic `lb_store` `data`-envelope. Kept in one file so the table name + the envelope
//! shape have a single owner (FILE-LAYOUT), mirroring `dashboard::store`. No authorization here — the
//! verbs gate first.

use lb_store::{read, scan_all, write, Store, StoreError};

use super::model::{Panel, TABLE};

/// Read `panel:{id}` in `ws`. `None` if absent in this namespace (the hard wall). A tombstoned record
/// still deserializes (callers treat `deleted` as absent).
pub async fn read_panel(store: &Store, ws: &str, id: &str) -> Result<Option<Panel>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(v) => {
            let p: Panel =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(p))
        }
        None => Ok(None),
    }
}

/// UPSERT `panel` at `panel:{id}` in `ws` (create+update; idempotent on the id).
pub async fn write_panel(store: &Store, ws: &str, p: &Panel) -> Result<(), StoreError> {
    let value = serde_json::to_value(p).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &p.id, &value).await
}

/// Scan every panel in `ws` (id-ordered, drained past the one-page cap). The roster read — the caller
/// then filters by gate-3 visibility and drops tombstones.
pub async fn scan_panels(store: &Store, ws: &str) -> Result<Vec<Panel>, StoreError> {
    let rows = scan_all(store, ws, TABLE).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Records written via `lb_store::write` carry a `{ data: ... }` envelope; `scan` returns the
        // whole record, so unwrap the envelope to get the panel (mirrors `dashboard::scan_dashboards`).
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        let p: Panel =
            serde_json::from_value(inner).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(p);
    }
    Ok(out)
}
