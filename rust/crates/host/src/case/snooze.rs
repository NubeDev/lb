//! `case_snooze` — park a case, or un-park it, over the triage capability.
//!
//! Gates on `mcp:case.workflow:call` (aliased `case.snooze → case.workflow`) — see
//! [`super::assign`] for why the three triage writes share one cap.
//!
//! Un-snooze is `until` at or before now, not a second verb. The reason requirement and the
//! un-snooze rule both live in the crate, so a second door cannot park a case without saying why.

use lb_auth::Principal;
use lb_cases::Case;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Snooze case `id` until `until` for `reason` as `principal` (`until <= ts` un-snoozes).
pub async fn case_snooze(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    until: u64,
    reason: Option<&str>,
    ts: u64,
) -> Result<Case, CaseSvcError> {
    authorize_tool(principal, ws, "case.workflow").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::snooze(store, ws, id, until, reason, principal.sub(), ts).await?)
}
