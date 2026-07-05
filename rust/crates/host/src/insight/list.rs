//! `insight_list` — the faceted, keyset-paged read over the capability gate (insights umbrella
//! scope). The workspace wall is structural (the store scan is ws-scoped, §7); the verb's gate
//! is `mcp:insight.list:call`.

use lb_auth::Principal;
use lb_insights::{ListPage, ListQuery};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// List insights in workspace `ws` matching `query`, newest-first, keyset-paged. See
/// [`ListQuery`] for the filter axes.
pub async fn insight_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    query: ListQuery,
) -> Result<ListPage, InsightSvcError> {
    authorize_tool(principal, ws, "insight.list").map_err(|_| InsightSvcError::Denied)?;
    let page = lb_insights::list(store, ws, query).await?;
    Ok(page)
}
