//! `case_snooze` — park a case, or un-park it, over the triage capability.
//!
//! Gates on `mcp:case.workflow:call` (aliased `case.snooze → case.workflow`) — see
//! [`super::assign`] for why the three triage writes share one cap.
//!
//! Un-snooze is `until` at or before now, not a second verb. The reason requirement and the
//! un-snooze rule both live in the crate, so a second door cannot park a case without saying why.

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::Case;
use lb_mcp::authorize_tool;

use super::error::CaseSvcError;
use crate::boot::Node;

/// Snooze case `id` until `until` for `reason` (`until <= ts` un-snoozes), then run the SLA clock:
/// a snooze that ended a pause has moved the deadlines ([`lb_cases::snooze`]), and the clock is the
/// one path that writes them and re-arms the breach alarm.
pub async fn case_snooze(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    until: u64,
    reason: Option<&str>,
    ts: u64,
) -> Result<Case, CaseSvcError> {
    authorize_tool(principal, ws, "case.workflow").map_err(|_| CaseSvcError::Denied)?;
    super::entity_filter::ensure_visible(&node.store, principal, ws, id).await?;
    lb_cases::snooze(&node.store, ws, id, until, reason, principal.sub(), ts).await?;
    super::sla_clock::apply_sla(node, ws, id, ts).await?;
    lb_cases::get(&node.store, ws, id)
        .await?
        .ok_or_else(|| CaseSvcError::BadInput(format!("no such case: {id}")))
}
