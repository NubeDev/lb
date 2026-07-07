//! `insight_sub_mute` — toggle a subscription's `muted` flag (subscriptions scope). Owner only.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Set the `muted` flag on sub `(ws, id)`.
pub async fn insight_sub_mute(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    muted: bool,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.sub.mute").map_err(|_| InsightSvcError::Denied)?;
    lb_insights::sub_mute(store, ws, id, muted).await?;
    Ok(())
}
