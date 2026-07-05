//! `insight_raise` — the producer WRITE, capability-gated (insights umbrella scope).
//!
//! The MCP door for "everything else" (the umbrella's third producer door): an agent under its
//! derived principal, an extension via host-callback, a human via the page/CLI. The rule door
//! (the rhai handle) and the flow door (the `insight` sink node) reach this same verb through
//! the same gate (`caller ∩ grant`), exactly like `inbox.record`/`channel.post` (rules-messaging).
//!
//! `producer` is **forced** to the principal's `sub` (host-set, never caller-supplied) — a caller
//! cannot forge another producer's identity (the ingest pattern). The dedup/re-open decision +
//! occurrence append live in `lb_insights::raise`; this layer is authorization + producer-forcing
//! only (one verb per file, FILE-LAYOUT §3).

use lb_auth::Principal;
use lb_insights::{raise, RaiseInput};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Raise an insight in workspace `ws` as `principal`. `producer` is forced to `principal.sub()`;
/// the caller's `input.producer` is ignored (never caller-spoofable). The dedup/re-open decision
/// is `lb_insights::raise`'s job.
pub async fn insight_raise(
    store: &Store,
    principal: &Principal,
    ws: &str,
    mut input: RaiseInput,
) -> Result<lb_insights::RaiseOutcome, InsightSvcError> {
    authorize_tool(principal, ws, "insight.raise").map_err(|_| InsightSvcError::Denied)?;
    input.producer = principal.sub().to_string();
    let outcome = raise(store, ws, input).await?;
    Ok(outcome)
}
