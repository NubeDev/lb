//! `insight_resolve` — `* → resolved` over the capability gate (insights umbrella scope). The
//! `resolved_by` is **forced** to the principal's `sub` — never caller-supplied. Idempotent on
//! an already-resolved insight.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Resolve insight `id` in workspace `ws` as `principal` at logical ts `ts`, with an optional
/// `note`. `resolved_by` is forced to `principal.sub()`.
pub async fn insight_resolve(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    note: Option<&str>,
    ts: u64,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.resolve").map_err(|_| InsightSvcError::Denied)?;
    lb_insights::resolve(store, ws, id, principal.sub(), note, ts).await?;
    Ok(())
}
