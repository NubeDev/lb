//! `policy_set` — write the workspace policy record (insights-notify-scope.md).
//!
//! Admin-only (`mcp:insight.policy.set:call` — the host gates; member deny is opaque). Stores
//! OVERRIDES only — the record is written whole; absent fields use the compiled defaults' serde
//! defaults (the seed pattern). The ring cap is validated against the hard bounds `[0, 1000]`.
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use crate::policy::{validate_ring_cap, Policy, TABLE};
use lb_store::{write, Store};

/// Write `policy` as the ws's policy record. Validates the ring cap bounds; rejects out-of-range
/// as `BadInput`. Idempotent (one row per ws, keyed by the ws id).
// SCOPE: docs/scope/insights/insight-notify-scope.md §"Settings surface"
pub async fn policy_set(store: &Store, ws: &str, policy: &Policy) -> Result<(), InsightsError> {
    validate_ring_cap(policy.ring_cap).map_err(InsightsError::BadInput)?;
    let value = serde_json::to_value(policy)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, TABLE, ws, &value).await?;
    Ok(())
}
