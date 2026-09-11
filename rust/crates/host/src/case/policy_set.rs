//! `policy.sla.set` — upsert one SLA service policy (case-plane scope).
//!
//! **Admin.** Deadlines are the contract the whole queue is ordered by, so the power to move them
//! is the power to reorder every case in the workspace — that belongs with `party.upsert` in the
//! ADMIN tier, not with the author verbs that triage individual cases.
//!
//! Upserts by `id`: a second write with the same id replaces the row, so the settings surface edits
//! in place and a pack re-seeds idempotently. A workspace-default policy is simply one whose
//! `match` is empty.

use lb_auth::Principal;
use lb_cases::ServicePolicy;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Write `policy` into the workspace's `service_policy` table, replacing any row with the same id.
///
/// The gate runs **before** validation, so a caller without the cap learns nothing about whether
/// their payload would have been accepted — the deny is opaque either way.
pub async fn case_policy_sla_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    policy: &ServicePolicy,
) -> Result<(), CaseSvcError> {
    authorize_tool(principal, ws, "policy.sla.set").map_err(|_| CaseSvcError::Denied)?;
    lb_cases::policy_set(store, ws, policy).await?;
    Ok(())
}
