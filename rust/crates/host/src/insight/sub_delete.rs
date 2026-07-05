//! `insight_sub_delete` — delete a subscription (subscriptions scope). Owner-or-admin only.
//! Idempotent.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Delete the sub at `(ws, id)`. Idempotent.
pub async fn insight_sub_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.sub.delete").map_err(|_| InsightSvcError::Denied)?;
    lb_insights::sub_delete(store, ws, id).await?;
    Ok(())
}
