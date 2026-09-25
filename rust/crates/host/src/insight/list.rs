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

use super::case_lens::{resolve_case_lens, CaseLens};
use super::error::InsightSvcError;

/// What an `insight.list` call carries beside its wire filter. `Default` is a plain list.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListOptions<'a> {
    /// The node's configured tag keys the search box matches beside the title
    /// (`Node::insight_search_tags`). Set on the filter from server state, as `entity` is: the wire
    /// can never carry it.
    pub search_tags: &'a [String],
    /// The caller's case lens (`case` beside the filter on the MCP call), resolved against the case
    /// plane before the list runs.
    pub case: Option<&'a CaseLens>,
}
use super::resolve_filter::resolve_filter;

/// List insights in workspace `ws` matching `query`, newest-first, keyset-paged. See
/// [`ListQuery`] for the filter axes.
///
/// `opts` carries what the transport adds beside the wire filter; see [`ListOptions`].
pub async fn insight_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    query: ListQuery,
    opts: &ListOptions<'_>,
) -> Result<ListPage, InsightSvcError> {
    authorize_tool(principal, ws, "insight.list").map_err(|_| InsightSvcError::Denied)?;
    // Both halves resolve in ONE place — see `resolve_filter`.
    let (tag_allow, assignee) = resolve_filter(store, principal, ws, &query.filter).await?;
    // Entity-scoped data: a restricted caller's list (and its counts) is limited to their entities'
    // insights, set here from server state — the wire can never carry it (`entity` is serde(skip)).
    let mut query = query;
    query.filter.search_tags = opts.search_tags.to_vec();
    if let Some(lens) = opts.case {
        query.filter.case = Some(resolve_case_lens(store, principal, ws, lens).await?);
    }
    if let Some((tag, ids)) = super::entity_filter::entity_limit(store, principal, ws).await? {
        query.filter.entity = Some((tag, ids.into_iter().collect()));
    }
    let page = lb_insights::list(store, ws, query, tag_allow.as_ref(), assignee.as_ref()).await?;
    Ok(page)
}
