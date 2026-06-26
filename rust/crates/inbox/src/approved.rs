//! Scan a workspace's **approved** resolutions — the durable backstop a resolution reactor reads
//! (coding-workflow scope). The read side of "which approvals have landed `Approved`?", the inbox
//! sibling of the outbox's `pending`/`due` scan.
//!
//! Why a scan and not a LIVE query: S6/S7 drive the workflow with a durable scan + an explicit start
//! (the relay is the same shape), not a LIVE-query reactor — the scan is the source of truth, so a
//! reactor that restarts simply re-reads the same set and never misses an approval (the LIVE push is
//! the latency optimization layered on later, like the relay's). The namespace is selected from `ws`,
//! so a ws-B scan can physically only return ws-B resolutions (the hard wall, §7).
//!
//! Raw verb — no authorization here (the host's workflow service is the caps chokepoint), exactly
//! like `resolve`/`resolution` and `list`.

use lb_store::{list as store_list, Store, StoreError};

use crate::resolution::{Resolution, RESOLUTION_TABLE};

/// Every `Approved` resolution in workspace `ws`, oldest→newest by `ts`. Empty if none — never
/// another workspace's. The `decision` field is the kebab-case `"approved"` discriminant the
/// `Resolution` serializes to, so the generic store equality filter selects exactly these.
pub async fn approved(store: &Store, ws: &str) -> Result<Vec<Resolution>, StoreError> {
    let mut resolutions = Vec::new();
    for v in store_list(store, ws, RESOLUTION_TABLE, "decision", "approved").await? {
        resolutions.push(serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?);
    }
    resolutions.sort_by_key(|r: &Resolution| r.ts);
    Ok(resolutions)
}
