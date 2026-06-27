//! The raw store read/write/scan for a [`RenderTemplate`] record — the (de)serialization seam between
//! the typed model and the generic `lb_store` `data`-envelope (FILE-LAYOUT; mirrors the dashboard
//! store seam). No authorization here — the verbs gate first.

use lb_store::{read, scan, write, Store, StoreError};

use super::model::{RenderTemplate, TABLE};

/// The largest roster a single `template.list` returns (one scan page).
pub const MAX_TEMPLATES: usize = lb_store::MAX_SCAN_LIMIT;

/// Read `render_template:{id}` in `ws`. `None` if absent in this namespace (the hard wall) — a
/// tombstoned record still deserializes (callers treat `deleted` as absent).
pub async fn read_template(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<RenderTemplate>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(v) => {
            let t: RenderTemplate =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(t))
        }
        None => Ok(None),
    }
}

/// UPSERT `template` at `render_template:{id}` in `ws` (create+update; idempotent on the id).
pub async fn write_template(store: &Store, ws: &str, t: &RenderTemplate) -> Result<(), StoreError> {
    let value = serde_json::to_value(t).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &t.id, &value).await
}

/// Scan up to [`MAX_TEMPLATES`] templates in `ws` (one page, id-ordered). The roster read — the
/// caller drops tombstones.
pub async fn scan_templates(store: &Store, ws: &str) -> Result<Vec<RenderTemplate>, StoreError> {
    let page = scan(store, ws, TABLE, MAX_TEMPLATES, None).await?;
    let mut out = Vec::with_capacity(page.rows.len());
    for row in page.rows {
        // `lb_store::write` wraps the value in a `{ data: ... }` envelope; `scan` returns the whole
        // record, so unwrap the envelope (same idiom the dashboard store seam uses).
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        let t: RenderTemplate =
            serde_json::from_value(inner).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(t);
    }
    Ok(out)
}
