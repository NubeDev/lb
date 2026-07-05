//! `insight_policy_set` — write the workspace policy record (insights-notify-scope.md).
//! Admin-only (`mcp:insight.policy.set:call`); member deny is opaque.

use lb_auth::Principal;
use lb_insights::Policy;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Write `policy` as the ws's policy record. Validates the ring cap bounds.
pub async fn insight_policy_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    policy: &Policy,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.policy.set").map_err(|_| InsightSvcError::Denied)?;
    lb_insights::policy_set(store, ws, policy).await?;
    Ok(())
}
