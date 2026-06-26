//! SPIKE-GATED ADD-ON — materialized **per-dimension count** view `tag_counts` (tags scope; the
//! store spike marked `DEFINE TABLE … AS SELECT … GROUP` AVAILABLE). Answers "how many entities per
//! `region`?" with no scan.
//!
//! **Per-dimension only.** A materialized `GROUP BY key` view gives counts for ONE dimension cheaply;
//! arbitrary multi-tag INTERSECTION counts ("eu-west AND telemetry") are combinatorial and computed
//! per-query (the `find` traversal), NEVER from this view. Do not oversell the "no scan" claim.
//!
//! `define_counts_view` is HOST-INTERNAL setup (run once per workspace at boot), not a caller verb —
//! the MCP surface is add/remove/of/find and nothing else.

use lb_store::{Store, StoreError};
use serde::Deserialize;

use crate::edge::TAGGED_TABLE;

/// A per-dimension count row: how many edges carry tag `key`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KeyCount {
    pub key: String,
    pub n: i64,
}

/// Define the materialized `tag_counts` view in `ws` (idempotent — re-defining is a no-op update).
/// Host-internal; not exposed as an MCP verb.
pub async fn define_counts_view(store: &Store, ws: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "DEFINE TABLE IF NOT EXISTS tag_counts AS \
                 SELECT count() AS n, key FROM {TAGGED_TABLE} GROUP BY key"
            ),
            vec![],
        )
        .await?;
    Ok(())
}

/// Read the per-dimension counts from the materialized view in `ws`.
pub async fn count_by_key(store: &Store, ws: &str) -> Result<Vec<KeyCount>, StoreError> {
    let mut resp = store
        .query_ws(ws, "SELECT key, n FROM tag_counts", vec![])
        .await?;
    let rows: Vec<KeyCount> = resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows)
}
