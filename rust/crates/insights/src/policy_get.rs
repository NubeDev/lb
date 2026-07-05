//! `policy_get` — read the workspace policy record (insight-notify-scope.md).
//!
//! Returns the compiled [`Policy::defaults`] when no record exists (the seed pattern). One row
//! per workspace; admin-gated at the host layer (`mcp:insight.policy.get:call`).
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use crate::policy::Policy;
use lb_store::Store;

/// Read the ws policy record, or the compiled defaults if absent. The host gates on the admin cap.
// SCOPE: docs/scope/insights/insight-notify-scope.md §"Settings surface"
pub async fn policy_get(_store: &Store, _ws: &str) -> Result<Policy, InsightsError> {
    // `read(store, ws, TABLE, ws)` and decode; if None ⇒ return `defaults()`.
    todo!("insights: policy get (defaults-on-absent) — SCOPE: notify-scope.md §Settings surface")
}
