//! The raw store read/write for a [`Brand`] record — the (de)serialization seam between the typed
//! model and the generic `lb_store` `data`-envelope (mirrors `panel::store`). No authorization here
//! — the verbs gate first.

use lb_store::{read, scan, write, Store, StoreError};

use super::model::{Brand, TABLE};

/// The largest roster a single `brand.list` returns (one scan page).
pub const MAX_BRANDS: usize = lb_store::MAX_SCAN_LIMIT;

/// Read `brand:{id}` in `ws`. `None` if absent in this namespace (the hard wall).
pub async fn read_brand(store: &Store, ws: &str, id: &str) -> Result<Option<Brand>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(v) => {
            let b: Brand =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(b))
        }
        None => Ok(None),
    }
}

/// UPSERT `brand` at `brand:{id}` in `ws` (create+update; idempotent on the id).
pub async fn write_brand(store: &Store, ws: &str, b: &Brand) -> Result<(), StoreError> {
    let value = serde_json::to_value(b).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &b.id, &value).await
}

/// Scan up to [`MAX_BRANDS`] brands in `ws` (one page, id-ordered). The roster read — the caller
/// drops tombstones.
pub async fn scan_brands(store: &Store, ws: &str) -> Result<Vec<Brand>, StoreError> {
    let page = scan(store, ws, TABLE, MAX_BRANDS, None).await?;
    let mut out = Vec::with_capacity(page.rows.len());
    for row in page.rows {
        // Records written via `lb_store::write` carry a `{ data: ... }` envelope; unwrap it.
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        let b: Brand =
            serde_json::from_value(inner).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(b);
    }
    Ok(out)
}
