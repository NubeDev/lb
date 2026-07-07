//! `insight_sub_get` — read one subscription over the capability gate (subscriptions scope).
//! Owner-or-admin only (the host checks ownership — deferred to the verb body; the cap gate ran).

use lb_auth::Principal;
use lb_insights::Subscription;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Return the sub at `(ws, id)`, or `None` if absent in this workspace.
pub async fn insight_sub_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Option<Subscription>, InsightSvcError> {
    authorize_tool(principal, ws, "insight.sub.get").map_err(|_| InsightSvcError::Denied)?;
    let sub = lb_insights::sub_get(store, ws, id).await?;
    Ok(sub)
}
