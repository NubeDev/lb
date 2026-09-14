//! `case_assign` — set / re-assign / un-assign a case's owner, over the triage capability.
//!
//! Gates on `mcp:case.workflow:call` (aliased `case.assign → case.workflow` in `tool_gate.rs`):
//! assigning is one of the three triage WRITES, and splitting them into three caps would create two
//! more grants that exist in no bundle without expressing a distinction anybody wants — nobody
//! grants "may comment but may not assign".
//!
//! **The case is the authority for who owns the work; the insight's `assigned_to` is an echo**
//! (resolved decision 5). This verb writes the case and then echoes onto every member, so there is
//! one writer for the fact and no consumer has to know which record is live.
//!
//! The assignee is necessarily caller-supplied (you assign to someone else), so it is **validated,
//! not trusted** — through the same `validate_assignee` the insight triage plane uses, which holds
//! the opacity contract (an unknown subject, a non-member and a member of ANOTHER workspace all
//! produce the identical refusal).

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::AssignOutcome;
use lb_mcp::authorize_tool;

use super::error::CaseSvcError;
use crate::boot::Node;
use crate::insight::validate_assignee;

/// Assign case `id` to `assignee` (`None` un-assigns) in `ws` as `principal`, echoing the owner
/// onto every insight the case cites.
pub async fn case_assign(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    assignee: Option<&str>,
    ts: u64,
) -> Result<AssignOutcome, CaseSvcError> {
    authorize_tool(principal, ws, "case.workflow").map_err(|_| CaseSvcError::Denied)?;
    if let Some(a) = assignee {
        validate_assignee(&node.store, ws, a)
            .await
            .map_err(|e| CaseSvcError::BadInput(e.to_string()))?;
    }
    let outcome = lb_cases::assign(&node.store, ws, id, assignee, principal.sub(), ts).await?;
    if outcome.changed {
        echo_owner_to_members(node, ws, id, assignee).await;
    }
    Ok(outcome)
}

/// Push the case's owner onto every insight it cites. Best-effort per member: the case already
/// holds the durable answer, so a failed echo is a stale roster column the next assign repairs —
/// not a reason to fail a write that landed.
pub async fn echo_owner_to_members(
    node: &Arc<Node>,
    ws: &str,
    case_id: &str,
    assignee: Option<&str>,
) {
    // `members_all`, not the paged `members`: the echo wants the membership, and the paged read
    // resolves titles for a drawer that is not on screen here.
    let members = match lb_cases::members_all(&node.store, ws, case_id).await {
        Ok(members) => members,
        Err(e) => {
            tracing::warn!(ws, case_id, error = %e, "case owner echo skipped: members unreadable");
            return;
        }
    };
    for member in members {
        if let Err(e) = lb_insights::assign(&node.store, ws, &member.insight_id, assignee).await {
            tracing::warn!(
                ws, case_id, insight_id = %member.insight_id, error = %e,
                "case owner echo not written onto the insight"
            );
        }
    }
}
