//! `set_deadlines` — write the SLA answer onto a case (case-plane scope, the sla-clock row).
//!
//! The **arithmetic** lives in [`crate::deadline`] and the **choice of contract** in
//! [`crate::policy_match`]; this file only persists the answer and records it in the history. Kept
//! separate for the reason [`crate::assign`] is separate: one responsibility per file, and a
//! deadline write that also computed the deadline would give the reactor two places to be wrong.
//!
//! # Idempotent by construction
//!
//! A deadline is a pure function of `(policy, opened_ts)` — neither of which moves once a case is
//! open unless the *policy* changes. So recomputing and calling this on every grouping pass is
//! free: when the answer is unchanged nothing is written and no event is appended. That is also
//! what makes "a snooze does not move the clock" structural rather than a rule somebody has to
//! remember — a snooze changes no input to the arithmetic, so a recompute after one returns the
//! identical instants.
//!
//! # Absent is honest
//!
//! `None` is a first-class answer, passed straight through. A workspace with no matching policy
//! gets a case with no deadlines and an event that says so. Inventing a default number of hours
//! would put a contract nobody agreed to in front of a client, which is worse than saying nothing.

use lb_store::Store;

use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// The deadlines a policy produced for one case. All three move together — a `policy_id` without
/// deadlines, or deadlines with no policy to trace them to, is not a defensible answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Deadlines {
    /// When the first response is due, or `None` when no policy governs this case.
    pub respond_by: Option<u64>,
    /// When resolution is due — the queue's sort key.
    pub due_at: Option<u64>,
}

/// Write `policy_id` + `deadlines` onto case `id`, appending an [`EventKind::Sla`] event naming the
/// clause that produced them. Returns the case when something actually changed, `None` when the
/// stored answer already matched (no write, no event).
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (sla-clock)
pub async fn set_deadlines(
    store: &Store,
    ws: &str,
    id: &str,
    policy_id: Option<&str>,
    deadlines: Deadlines,
    actor: &str,
    ts: u64,
) -> Result<Option<Case>, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    let next_policy = policy_id.map(str::to_string);
    if case.policy_id == next_policy
        && case.respond_by == deadlines.respond_by
        && case.due_at == deadlines.due_at
    {
        // Unchanged — nothing to write. With ONE exception, and it is the whole reason this branch
        // is not a bare `return`: a case that has never been measured and matches NO policy is
        // byte-for-byte identical to one this verb has already spoken about. Every field is `None`
        // either way. Absent is an honest answer, but a *silent* absent is not — a reader cannot
        // tell "no contract governs this work" from "the clock never ran", and those want opposite
        // responses (write a policy vs. fix the reactor). So the first pass over an unmatched case
        // still states it, and only the first.
        if next_policy.is_none() && !already_stated(store, ws, id).await? {
            state_it(store, ws, id, &case, None, deadlines, actor, ts).await?;
            return Ok(Some(case));
        }
        return Ok(None);
    }

    let from_policy = case.policy_id.clone();
    let from_respond = case.respond_by;
    let from_due = case.due_at;
    case.policy_id = next_policy.clone();
    case.respond_by = deadlines.respond_by;
    case.due_at = deadlines.due_at;
    save(store, ws, &mut case, ts).await?;

    let previous = Previous {
        policy_id: from_policy,
        respond_by: from_respond,
        due_at: from_due,
    };
    state_it(store, ws, id, &case, Some(previous), deadlines, actor, ts).await?;
    Ok(Some(case))
}

/// What the case said before this write — carried so the event can name both ends of every axis.
struct Previous {
    policy_id: Option<String>,
    respond_by: Option<u64>,
    due_at: Option<u64>,
}

/// Append the `sla` event. `previous` is `None` for the first statement about a case that matched
/// nothing (there was no prior answer to move away from).
#[allow(clippy::too_many_arguments)]
async fn state_it(
    store: &Store,
    ws: &str,
    id: &str,
    case: &Case,
    previous: Option<Previous>,
    deadlines: Deadlines,
    actor: &str,
    ts: u64,
) -> Result<(), CasesError> {
    let previous = previous.unwrap_or(Previous {
        policy_id: None,
        respond_by: None,
        due_at: None,
    });
    append_event(
        store,
        ws,
        id,
        EventKind::Sla,
        actor,
        serde_json::json!({
            // BOTH ends of every axis. A deadline a client can argue with has to say what it was
            // before, not just what it is now — a recompute that silently moved a due date and left
            // only the new value in the history is indistinguishable from a deadline nobody set.
            "policy_from": previous.policy_id,
            "policy_id": case.policy_id,
            "respond_by_from": previous.respond_by,
            "respond_by": deadlines.respond_by,
            "due_at_from": previous.due_at,
            "due_at": deadlines.due_at,
            // The explicit "no contract governs this case" statement. Present so the drawer can
            // render *why* a case has no deadline instead of leaving a blank column.
            "unmatched": case.policy_id.is_none(),
            "severity": case.severity,
        }),
        ts,
    )
    .await?;
    Ok(())
}

/// Whether this case's history already carries an `sla` statement. Read only on the unmatched path
/// (see the caller) — a case with a policy id is self-evidently already spoken about.
async fn already_stated(store: &Store, ws: &str, id: &str) -> Result<bool, CasesError> {
    Ok(crate::events::events_all(store, ws, id)
        .await?
        .iter()
        .any(|e| e.kind == EventKind::Sla))
}
