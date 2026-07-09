//! `insight_delete` — hard-delete an insight + cascade its occurrence ring, over the capability
//! gate (insights umbrella scope). A destructive verb: unlike `resolve` (which closes but keeps
//! the record), this erases the parent AND every occurrence row under it. Idempotent.
//!
//! Gated on its OWN cap `mcp:insight.delete:call` — the read/act caps (`get`/`list`/`ack`/
//! `resolve`) do NOT imply destroying shared content + evidence (an authoring reach, member-level).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Delete insight `id` in workspace `ws` as `principal`, cascading its occurrence ring. Idempotent
/// on an already-gone insight.
pub async fn insight_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.delete").map_err(|_| InsightSvcError::Denied)?;
    lb_insights::delete(store, ws, id).await?;
    Ok(())
}
