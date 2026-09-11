//! `case_workflow` — the transition verb over the capability gate (case-plane scope).
//!
//! Gated on `mcp:case.workflow:call`, the AUTHOR (member) tier — and deliberately NOT on a generic
//! `case.update`. One cap for "change any field" would hand the reactors' `case.open` grant the
//! power to close a customer's job, and the deny path would stop being expressible. The narrow verb
//! is what makes "a grouping reactor may create work but may never resolve it" a real statement.
//!
//! `actor` is forced to the principal's `sub` (host-stamped). The `resolved`-requires-`resolution`
//! invariant lives in the crate, not here — one implementation, so a second door cannot bypass it.

use lb_auth::Principal;
use lb_cases::{Case, Resolution, WaitingOn, Workflow};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Move case `id` to `next` in workspace `ws` as `principal`.
// The gate's arguments plus the transition's — a parameter struct would be a type that exists only
// to be destructured on the next line. `call_case_tool` is the only caller.
#[allow(clippy::too_many_arguments)]
pub async fn case_workflow(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    next: Workflow,
    resolution: Option<Resolution>,
    waiting_on: Option<WaitingOn>,
    ts: u64,
) -> Result<Case, CaseSvcError> {
    authorize_tool(principal, ws, "case.workflow").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::workflow(
        store,
        ws,
        id,
        next,
        resolution,
        waiting_on,
        principal.sub(),
        ts,
    )
    .await?)
}
