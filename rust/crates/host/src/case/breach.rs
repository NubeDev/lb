//! `case.breach` — record that a case blew its deadline, and escalate (case-plane scope).
//!
//! This is the verb the breach alarm fires ([`super::breach_reminder`]). It is a real MCP verb
//! rather than an internal function for one reason: the reminder plane's only way to run work is
//! `Action::McpTool`, which re-enters `call_tool` and re-checks `mcp:{tool}:call` under the stored
//! principal at fire time. Going through the door is what makes the alarm subject to the same caps
//! wall as everything else — an alarm that could write a case without a capability would be a hole
//! in the wall shaped exactly like a reactor.
//!
//! # The capability is in no role bundle, and that is deliberate
//!
//! `mcp:case.breach:call` gates on its **own name** (no `tool_gate.rs` alias needed) and is granted
//! to exactly one subject — the SLA clock's own — by [`super::breach_reminder`]. A person does not
//! declare a breach; a deadline does. Nobody gets a button for this, which is also why a missing
//! grant shows up as a positive-test failure rather than as a quietly un-breached queue.
//!
//! # Idempotent, and set-once underneath
//!
//! Everything durable here is [`lb_cases::mark_breached`], which refuses to touch a case that is
//! already breached. So a re-fired alarm, a manual retry and a reconcile pass all converge: the
//! FIRST pass writes the breach and escalates, every later one returns `breached: false` and sends
//! nothing. The escalation is inside the `Some(_)` arm precisely so a duplicate firing cannot page
//! the assignee twice.

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::Case;
use lb_mcp::authorize_tool;

use super::error::CaseSvcError;
use super::sla_clock::SLA_ACTOR;
use crate::boot::Node;

/// Declare case `id` breached as of logical time `ts`, if it actually is.
///
/// Returns the updated case, or `None` when nothing changed — already breached, closed, no
/// deadline, or the deadline has not arrived. Each of those is a legitimate no-op, so a caller may
/// run this as often as it likes.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (sla-clock, at breach)
pub async fn case_breach(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    ts: u64,
) -> Result<Option<Case>, CaseSvcError> {
    authorize_tool(principal, ws, "case.breach").map_err(|_| CaseSvcError::Denied)?;

    // `at` and `ts` are the same instant here: the alarm rings when the deadline arrives, so "has
    // it arrived" is asked against the same clock the event is stamped with. The crate writes the
    // DEADLINE as `breached_ts`, not this, so a late tick cannot understate how late the case is.
    let Some(case) = lb_cases::mark_breached(&node.store, ws, id, ts, SLA_ACTOR, ts).await? else {
        return Ok(None);
    };
    super::breach_notify::escalate_to_assignee(node, ws, &case, ts).await;
    Ok(Some(case))
}
