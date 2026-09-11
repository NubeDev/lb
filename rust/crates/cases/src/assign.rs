//! `assign` — set, re-assign, or clear a case's owner (case-plane scope).
//!
//! **The case owns the assignee; the insight's `assigned_to` is an echo of it** (resolved decision
//! 5). `insight.assign` delegates here rather than reading through, so there is exactly one writer
//! for the fact "who is doing this work" and no consumer has to know which record is live.
//!
//! One verb for all three gestures: `Some(subject)` assigns/re-assigns, `None` un-assigns.
//! Idempotent — assigning the current owner writes nothing and appends no event, so a double-click
//! or a retried bulk call neither dirties the history nor pages a queue twice.
//!
//! `assigned_to` is a **subject, not a user id** (`user:priya` or `team:mechanical`). Membership
//! validation is the SERVICE layer's job (it needs the membership/team planes this crate is
//! deliberately agnostic of); this verb writes what it is told, after establishing the case exists.

use lb_store::Store;

use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// What an [`assign`] call actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignOutcome {
    /// The stored value after the call — echoed so the caller reports what actually landed.
    pub assigned_to: Option<String>,
    /// `false` when the case ALREADY had this owner. Nothing was written and no event was
    /// appended; the host reads this to decide whether to notify.
    pub changed: bool,
}

/// Assign case `id` in workspace `ws` to `assignee` (`None` clears), as `actor` at ts `ts`.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.assign)
pub async fn assign(
    store: &Store,
    ws: &str,
    id: &str,
    assignee: Option<&str>,
    actor: &str,
    ts: u64,
) -> Result<AssignOutcome, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    let next = assignee.map(str::to_string);
    if case.assigned_to == next {
        return Ok(AssignOutcome {
            assigned_to: next,
            changed: false,
        });
    }
    let previous = case.assigned_to.clone();
    case.assigned_to = next.clone();
    save(store, ws, &mut case, ts).await?;
    append_event(
        store,
        ws,
        id,
        EventKind::Assigned,
        actor,
        serde_json::json!({ "from": previous, "to": next }),
        ts,
    )
    .await?;
    Ok(AssignOutcome {
        assigned_to: next,
        changed: true,
    })
}
