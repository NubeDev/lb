//! `policy_delete` — erase a service-policy row (case-plane scope §Wave 5).
//!
//! **Unconditional, unlike [`crate::party_delete`]**, and the asymmetry is the point rather than an
//! oversight. A case carries the deadlines it was given — `respond_by` and `due_at` are STAMPED on
//! the case by the sla-clock reactor, not re-derived from the policy on every read — so deleting a
//! policy cannot change any existing case's clock or orphan a reference the way deleting a party
//! orphans its requests. `Case.policy_id` keeps the id as provenance, and an id that no longer
//! resolves still answers "which policy was applied" better than nothing would.
//!
//! What deleting DOES change is what governs the NEXT case, which is the same thing disabling does.
//! So [`crate::ServicePolicy::active`] exists beside this for the case where the row is worth
//! keeping as evidence — a lapsed seasonal contract is the argument for what was promised when a
//! breach happened, and erasing it makes that unanswerable.
//!
//! Idempotent on a row that is not there.

use lb_store::{delete, read, Store};

use crate::error::CasesError;
use crate::policy::POLICY_TABLE;

/// Erase the policy at `(ws, id)`. Returns `Ok(false)` when there was no such row.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Wave 5"
pub async fn policy_delete(store: &Store, ws: &str, id: &str) -> Result<bool, CasesError> {
    if read(store, ws, POLICY_TABLE, id).await?.is_none() {
        return Ok(false);
    }
    delete(store, ws, POLICY_TABLE, id).await?;
    Ok(true)
}
