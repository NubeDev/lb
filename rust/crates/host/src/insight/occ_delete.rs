//! `insight_occurrence_delete` — delete one row from an insight's occurrence ring, over the
//! capability gate (insight-occurrences-scope.md). The per-transaction destructive verb: drop a
//! single spurious evidence row without touching the parent (its lifetime `count` is unchanged).
//!
//! Gated on its OWN cap `mcp:insight.occurrence.delete:call` — deleting evidence is a stronger
//! effect than reading it (`mcp:insight.occurrences:call`), so the read cap does NOT imply it.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Delete the occurrence with sequence `oseq` from insight `insight_id`'s ring in workspace `ws`
/// as `principal`. Idempotent. The parent's `count`/`first_ts`/`last_ts` are unchanged.
pub async fn insight_occurrence_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    insight_id: &str,
    oseq: u64,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.occurrence.delete")
        .map_err(|_| InsightSvcError::Denied)?;
    lb_insights::delete_occurrence(store, ws, insight_id, oseq).await?;
    Ok(())
}
