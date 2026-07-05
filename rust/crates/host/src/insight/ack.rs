//! `insight_ack` — `open → acked` over the capability gate (insights umbrella scope). The
//! `acked_by` is **forced** to the principal's `sub` — never caller-supplied (a caller can't
//! forge another reviewer's ack). Idempotent on an already-acked insight; refused on `resolved`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Ack insight `id` in workspace `ws` as `principal` at logical ts `ts`. `acked_by` is forced
/// to `principal.sub()`.
pub async fn insight_ack(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    ts: u64,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.ack").map_err(|_| InsightSvcError::Denied)?;
    lb_insights::ack(store, ws, id, principal.sub(), ts).await?;
    Ok(())
}
