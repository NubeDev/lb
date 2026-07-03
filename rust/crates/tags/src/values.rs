//! `facet_values(key)` — the **distinct values** present for one tag key in a workspace (e.g. every
//! `site`: `plant-1`, `plant-2`, …). A single `GROUP BY` over the denormalized `tval` on the `tagged`
//! edges (tags scope) — the enumeration the reusable-pages **template-group** fans a template page out
//! over (one page instance per value). Distinct from [`crate::find`] (which returns the *entities*
//! matching a facet) and from [`crate::counts::count_by_key`] (per-*key* counts, not per-*value*).
//!
//! Namespace-scoped (the hard wall). Raw read — the host wrapper runs `caps::check` (the `tags.find`
//! cap) first, so the lens holds: a caller who cannot `tags.find` cannot enumerate values.

use lb_store::{Store, StoreError};
use serde::Deserialize;
use serde_json::Value;

use crate::edge::TAGGED_TABLE;

/// The distinct values present for tag `key` in `ws`, one entry per value (deduped by `GROUP BY`).
/// Empty when the key is unused. Values are returned verbatim (a string, number, or bool tag value).
pub async fn facet_values(store: &Store, ws: &str, key: &str) -> Result<Vec<Value>, StoreError> {
    if key.is_empty() {
        return Ok(Vec::new());
    }
    let mut resp = store
        .query_ws(
            ws,
            // tkey/tval are the edge's denormalized tag key/value (a RELATION drops literal key/value
            // fields — debugging/tags/relation-drops-key-value-fields.md). GROUP BY tval dedups.
            &format!("SELECT tval AS value FROM {TAGGED_TABLE} WHERE tkey = $k GROUP BY value"),
            vec![("k".into(), Value::String(key.to_string()))],
        )
        .await?;
    let rows: Vec<ValueRow> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.into_iter().map(|r| r.value).collect())
}

#[derive(Deserialize)]
struct ValueRow {
    value: Value,
}
