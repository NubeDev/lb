//! `policy_get` — read the workspace policy record (insight-notify-scope.md).
//!
//! Returns the compiled [`Policy::defaults`] when no record exists (the seed pattern). One row
//! per workspace; admin-gated at the host layer (`mcp:insight.policy.get:call`).
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use crate::policy::{defaults, Policy, TABLE};
use lb_store::{read, Store};

/// Read the ws policy record, or the compiled defaults if absent. The host gates on the admin cap.
// SCOPE: docs/scope/insights/insight-notify-scope.md §"Settings surface"
pub async fn policy_get(store: &Store, ws: &str) -> Result<Policy, InsightsError> {
    let Some(value) = read(store, ws, TABLE, ws).await? else {
        // Absent record ⇒ compiled defaults (the seed pattern — record stores overrides only).
        return Ok(defaults());
    };
    // A record with only some fields set fills the rest from serde defaults (the whole-fold
    // pattern) — so an old/partial record still resolves to a complete Policy.
    let policy: Policy = serde_json::from_value(value)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    Ok(policy)
}
