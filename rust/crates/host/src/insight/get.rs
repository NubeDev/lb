//! `insight_get` — read one insight by id over the capability gate (insights umbrella scope).

use lb_auth::Principal;
use lb_insights::Insight;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Return the insight at `(ws, id)`, or `None` if absent in this workspace. Gated by
/// `mcp:insight.get:call` (workspace-first §7).
pub async fn insight_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Option<Insight>, InsightSvcError> {
    authorize_tool(principal, ws, "insight.get").map_err(|_| InsightSvcError::Denied)?;
    let insight = lb_insights::get(store, ws, id).await?;
    // Entity-scoped data: an insight outside the caller's entities reads as absent, not denied — the
    // same answer a wrong id gets, so its existence is not disclosed.
    match insight {
        Some(i) if !super::entity_filter::visible(store, principal, ws, &i).await? => Ok(None),
        other => Ok(other),
    }
}
