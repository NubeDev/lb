//! The per-workspace series registry — one `series_meta` row per distinct series name. Two jobs
//! (series schema slice):
//!   - the **cardinality cap**: the count of rows here is "how many distinct series this workspace
//!     has", checked before a commit admits a NEW series name (the ingest scope's highest-risk
//!     item — unbounded series names are unbounded index + tag growth);
//!   - the **label→tag flag**: `labels_applied` records that a series' wire labels were converted
//!     to tag edges, so the conversion runs once per series, not once per sample.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::schema::SERIES_META_TABLE;

/// Default cap on distinct series names per workspace.
pub const DEFAULT_SERIES_CAP: usize = 10_000;

/// Count of registered (distinct) series names in `ws`.
pub async fn series_count(store: &Store, ws: &str) -> Result<usize, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {SERIES_META_TABLE} GROUP ALL"),
            vec![],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}

/// Is `series` already registered in `ws`?
pub async fn is_registered(store: &Store, ws: &str, series: &str) -> Result<bool, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT series FROM type::thing('{SERIES_META_TABLE}', $series)"),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(!rows.is_empty())
}

/// Register `series` (idempotent; preserves an existing `labels_applied`).
pub async fn register(store: &Store, ws: &str, series: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "UPSERT type::thing('{SERIES_META_TABLE}', $series) SET series = $series, \
                 labels_applied = labels_applied OR false"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    Ok(())
}

/// Has this series' labels already been converted to tag edges?
pub async fn labels_applied(store: &Store, ws: &str, series: &str) -> Result<bool, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT labels_applied FROM type::thing('{SERIES_META_TABLE}', $series)"),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take("labels_applied")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.first().and_then(|v| v.as_bool()).unwrap_or(false))
}

/// Mark the series' labels as converted (the once-per-series latch).
pub async fn mark_labels_applied(store: &Store, ws: &str, series: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "UPDATE type::thing('{SERIES_META_TABLE}', $series) SET labels_applied = true"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    Ok(())
}

/// The registered series names in `ws` starting with `prefix` (empty = all), ascending.
pub async fn series_names(
    store: &Store,
    ws: &str,
    prefix: &str,
) -> Result<Vec<String>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT series FROM {SERIES_META_TABLE} \
                 WHERE string::starts_with(series, $prefix) ORDER BY series ASC"
            ),
            vec![("prefix".into(), Value::String(prefix.to_string()))],
        )
        .await?;
    let names: Vec<String> = resp
        .take("series")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(names)
}
