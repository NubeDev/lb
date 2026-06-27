//! SPIKE-GATED ADD-ON — **per-dimension counts** ("how many entities per `region`?"), answered with
//! a single `GROUP BY` over the `tagged` edges (tags scope).
//!
//! **Engine finding (recorded):** the store spike marked `DEFINE TABLE … AS SELECT … GROUP` as
//! parse/define-AVAILABLE, but a **materialized** AS-SELECT view does **not populate** on SurrealKV
//! for our edge writes — neither incrementally on edge UPSERT nor on backfill-at-define (the view
//! reads empty while the equivalent ad-hoc `GROUP BY` is correct). See
//! debugging/tags/materialized-view-does-not-populate.md. So per the spike's degrade rule and this
//! scope's open question ("`tag_counts` live view vs periodically rebuilt — measure"), counts are
//! **computed per-query** here. `define_counts_view` remains the idempotent setup hook (ensures the
//! edge table exists + records intent); when a future engine populates the view, `count_by_key` can
//! switch to reading it with no caller change.
//!
//! **Per-dimension only.** This gives counts for ONE dimension; arbitrary multi-tag INTERSECTION
//! counts ("eu-west AND telemetry") are combinatorial and computed via the `find` traversal, NEVER
//! from a per-dimension rollup. Do not oversell the "no scan" claim. Host-internal setup — the MCP
//! surface stays add/remove/of/find and nothing else.

use lb_store::{Store, StoreError};
use serde::Deserialize;

use crate::edge::TAGGED_TABLE;

/// A per-dimension count row: how many edges carry tag `key`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KeyCount {
    pub key: String,
    pub n: i64,
}

/// Ensure the `tagged` edge table exists in `ws` (idempotent setup hook). Host-internal; not an MCP
/// verb. Kept as the seam where a materialized view would be (re)defined once the engine populates
/// one — today the counts are computed per-query by [`count_by_key`].
pub async fn define_counts_view(store: &Store, ws: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!("DEFINE TABLE IF NOT EXISTS {TAGGED_TABLE}"),
            vec![],
        )
        .await?;
    Ok(())
}

/// The per-dimension counts in `ws`: one row per tag `key` with how many edges carry it. Computed
/// with a single `GROUP BY` (the edge's denormalized `tkey` aliased back to `key`).
pub async fn count_by_key(store: &Store, ws: &str) -> Result<Vec<KeyCount>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() AS n, tkey AS key FROM {TAGGED_TABLE} GROUP BY key"),
            vec![],
        )
        .await?;
    let rows: Vec<KeyCount> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows)
}
