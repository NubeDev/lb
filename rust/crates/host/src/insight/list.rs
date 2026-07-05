//! `insight_list` — the faceted, keyset-paged read over the capability gate (insights umbrella
//! scope). The workspace wall is structural (the store scan is ws-scoped, §7); the verb's gate
//! is `mcp:insight.list:call`.
//!
//! Tag facets ride the tag graph: when `query.filter.tags` is non-empty the host resolves the
//! matching entity ids via `lb_tags::find` (the raw graph read — the `insight.list` cap already
//! authorized this workspace read, and a tag facet is a filter on already-authorized insights, not
//! a new privilege) and hands the id allowlist to the crate's tag-agnostic `list`.

use std::collections::HashSet;

use lb_auth::Principal;
use lb_insights::{ListPage, ListQuery};
use lb_mcp::authorize_tool;
use lb_store::Store;
use lb_tags::Facet;

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

    // Resolve the tag facet to an insight-id allowlist (only when the query constrains on tags).
    let tag_allow: Option<HashSet<String>> = if query.filter.tags.is_empty() {
        None
    } else {
        let facets: Vec<Facet> = query
            .filter
            .tags
            .iter()
            .map(|(k, v)| Facet::exact(k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        let entities = lb_tags::find(store, ws, &facets)
            .await
            .map_err(|e| InsightSvcError::Store(e.to_string()))?;
        // Entities are `insight:<id>` refs; the crate list matches on bare ids.
        let ids = entities
            .into_iter()
            .map(|e| e.strip_prefix("insight:").map(str::to_string).unwrap_or(e))
            .collect();
        Some(ids)
    };

    let page = lb_insights::list(store, ws, query, tag_allow.as_ref()).await?;
    Ok(page)
}
