//! `insight_comments` — read an insight's comment thread, over the **`insight.get` capability**
//! (insight-triage-scope.md).
//!
//! **No new read cap, deliberately.** The thread is part of the finding's detail, and a reader who
//! may `get` the insight may read its notes — splitting a cap here would create a record whose
//! drawer half-renders. This is the deliberate contrast with `insight.occurrences`, which DOES have
//! its own cap: evidence can be more sensitive than the headline, whereas a triage note is written
//! by and for the same operators who read the finding.
//!
//! Composed into the `insight.get` response by the MCP bridge rather than returned as its own verb —
//! the drawer wants the record and its thread in one round-trip, and a separate `insight.comments`
//! verb would be a second cap-checked call for one screen.

use lb_auth::Principal;
use lb_insights::Comment;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Every comment on insight `id` in workspace `ws`, newest-first. Empty when there is no thread.
pub async fn insight_comments(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Vec<Comment>, InsightSvcError> {
    authorize_tool(principal, ws, "insight.get").map_err(|_| InsightSvcError::Denied)?;
    let thread = lb_insights::comments(store, ws, id).await?;
    Ok(thread)
}
