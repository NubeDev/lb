//! Scan a workspace's **rejected** resolutions — the sibling of [`approved`](crate::approved) the
//! approval-release reactor reads to discard a gated effect (rules-approvals scope).
//!
//! Same shape as `approved`: a durable equality scan (the source of truth a restarting reactor
//! re-reads), workspace-namespaced so a ws-B scan can physically only return ws-B resolutions (the
//! hard wall §7). Raw verb — the host reactor is the caps chokepoint.

use lb_store::{list as store_list, Store, StoreError};

use crate::resolution::{Resolution, RESOLUTION_TABLE};

/// Every `Rejected` resolution in workspace `ws`, oldest→newest by `ts`. The `decision` field is the
/// kebab-case `"rejected"` discriminant the `Resolution` serializes to. Empty if none — never another
/// workspace's.
pub async fn rejected(store: &Store, ws: &str) -> Result<Vec<Resolution>, StoreError> {
    let mut resolutions = Vec::new();
    for v in store_list(store, ws, RESOLUTION_TABLE, "decision", "rejected").await? {
        resolutions.push(serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?);
    }
    resolutions.sort_by_key(|r: &Resolution| r.ts);
    Ok(resolutions)
}
