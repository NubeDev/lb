//! `insight_sub_list` — list subscriptions over the capability gate (subscriptions scope).
//! Member-default: own subs. Admin lens: all ws subs (the host threads `all`).

use lb_auth::Principal;
use lb_insights::Subscription;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// List subs in workspace `ws`. If `all` is true, returns every ws sub (admin lens — the host
/// checks the admin cap before setting `all`); else only `principal`'s subs.
pub async fn insight_sub_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    all: bool,
) -> Result<Vec<Subscription>, InsightSvcError> {
    authorize_tool(principal, ws, "insight.sub.list").map_err(|_| InsightSvcError::Denied)?;
    let subs = lb_insights::sub_list(store, ws, principal.sub(), all).await?;
    Ok(subs)
}
