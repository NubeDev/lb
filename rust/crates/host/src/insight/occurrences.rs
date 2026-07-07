//! `insight_occurrences` — read the per-insight occurrence ring, keyset-paged newest-first
//! (insight-occurrences-scope.md). Own verb, own cap `mcp:insight.occurrences:call` — the
//! list/get read cap does NOT imply evidence access.

use lb_auth::Principal;
use lb_insights::{OccCursor, OccurrencePage};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Read the occurrence ring for insight `insight_id` in workspace `ws`, newest-first.
pub async fn insight_occurrences(
    store: &Store,
    principal: &Principal,
    ws: &str,
    insight_id: &str,
    cursor: Option<OccCursor>,
    limit: usize,
) -> Result<OccurrencePage, InsightSvcError> {
    authorize_tool(principal, ws, "insight.occurrences").map_err(|_| InsightSvcError::Denied)?;
    let page = lb_insights::occurrences(store, ws, insight_id, cursor, limit).await?;
    Ok(page)
}
