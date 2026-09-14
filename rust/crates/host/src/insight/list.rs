//! `insight_list` — the faceted, keyset-paged read over the capability gate (insights umbrella
//! scope). The workspace wall is structural (the store scan is ws-scoped, §7); the verb's gate
//! is `mcp:insight.list:call`.
//!
//! Tag facets ride the tag graph: when `query.filter.tags` is non-empty the host resolves the
//! matching entity ids via `lb_tags::find` (the raw graph read — the `insight.list` cap already
//! authorized this workspace read, and a tag facet is a filter on already-authorized insights, not
//! a new privilege) and hands the id allowlist to the crate's tag-agnostic `list`.

use lb_auth::Principal;
use lb_insights::{ListPage, ListQuery};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;
use super::resolve_filter::resolve_filter;

/// List insights in workspace `ws` matching `query`, newest-first, keyset-paged. See
/// [`ListQuery`] for the filter axes.
pub async fn insight_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    query: ListQuery,
) -> Result<ListPage, InsightSvcError> {
    authorize_tool(principal, ws, "insight.list").map_err(|_| InsightSvcError::Denied)?;
    // Both halves resolve in ONE place — see `resolve_filter`.
    let (tag_allow, assignee) = resolve_filter(store, principal, ws, &query.filter).await?;
    let page = lb_insights::list(store, ws, query, tag_allow.as_ref(), assignee.as_ref()).await?;
    Ok(page)
}
