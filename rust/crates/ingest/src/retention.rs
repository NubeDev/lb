//! The per-series-prefix retention policy — the record the GC pass executes (series-retention
//! scope). A policy says: keep raw samples for `raw_for_ms`, downsample what falls off into the
//! listed rollup `tiers` (each kept for its own horizon), then evict. Workspace-scoped like every
//! series-plane record (the hard wall); administered only through the capability-gated
//! `series.retention.*` verbs in the host.

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

/// The retention-policy table; one row per series-name prefix (id = prefix).
pub const RETENTION_TABLE: &str = "series_retention";

/// One rollup tier: bucket width and how long the tier's rows are kept (`0` = keep forever).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tier {
    pub width_ms: u64,
    pub keep_for_ms: u64,
}

/// A retention policy for every series whose name starts with `prefix`. `raw_for_ms == 0` disables
/// eviction (the explicit "keep raw forever" default a series has when no policy matches).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    pub prefix: String,
    pub raw_for_ms: u64,
    #[serde(default)]
    pub tiers: Vec<Tier>,
}

/// Upsert the policy at its prefix (one policy per prefix; a re-set overwrites).
pub async fn set_policy(store: &Store, ws: &str, policy: &Policy) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!("UPSERT type::thing('{RETENTION_TABLE}', $prefix) CONTENT $row"),
            vec![
                ("prefix".into(), Value::String(policy.prefix.clone())),
                ("row".into(), json!(policy)),
            ],
        )
        .await?;
    Ok(())
}

/// All policies in `ws`, ordered by prefix.
pub async fn list_policies(store: &Store, ws: &str) -> Result<Vec<Policy>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT prefix, raw_for_ms, tiers FROM {RETENTION_TABLE} ORDER BY prefix ASC"),
            vec![],
        )
        .await?;
    resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))
}

/// Delete the policy at `prefix` (series covered by it revert to keep-forever).
pub async fn delete_policy(store: &Store, ws: &str, prefix: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!("DELETE type::thing('{RETENTION_TABLE}', $prefix)"),
            vec![("prefix".into(), Value::String(prefix.to_string()))],
        )
        .await?;
    Ok(())
}
