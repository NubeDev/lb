//! `insight_policy_get` — read the workspace policy record (insights-notify-scope.md).
//! Admin-gated at the host layer (`mcp:insight.policy.get:call`).

use lb_auth::Principal;
use lb_insights::Policy;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Read the ws policy, or the compiled defaults if absent.
pub async fn insight_policy_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Policy, InsightSvcError> {
    authorize_tool(principal, ws, "insight.policy.get").map_err(|_| InsightSvcError::Denied)?;
    let policy = lb_insights::policy_get(store, ws).await?;
    Ok(policy)
}
