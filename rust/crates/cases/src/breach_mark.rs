//! `mark_breached` — record that a case blew its resolution deadline (case-plane scope).
//!
//! # Set once, never un-set
//!
//! A breach is **history**, not a status light. Once `breached_ts` is written this verb refuses to
//! touch it again — a second pass, a re-fired reminder, a reconcile sweep and a manual retry all
//! return `Ok(None)` and change nothing. The temptation to clear it when the case is finally
//! resolved is exactly the bug: "we missed the SLA and then fixed it" and "we never missed it" are
//! different facts, and only one of them is true.
//!
//! # `breach_waiting_on` is the whole point
//!
//! The breach captures the case's `waiting_on` **at that instant**. That is what turns "this case
//! is late" into an attributable fact — the difference between a report that says a contractor went
//! quiet for three weeks and one that says we were late. It is captured here, at the moment of the
//! breach, because five minutes later somebody moves the case and the answer is gone for ever.
//!
//! # The clock runs only while the case is Open
//!
//! The SLA is the time to a first response ([`crate::sla_follow`]): once the case moves out of Open
//! — actioned, assigned, emailed, on hold — no breach is recorded here, and if it comes back to Open
//! its deadline has moved forward by the time it spent out, so a breach armed for the old deadline
//! finds the case not yet due.

use lb_store::Store;

use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// Mark case `id` breached at `at` (the deadline instant that was missed), attributing it to the
/// `waiting_on` the case holds right now. `Ok(None)` when the case is already breached, is closed,
/// is paused, has no `due_at`, or has not reached it yet — every one of those is a legitimate no-op, so the
/// caller may run this as often as it likes.
///
/// `at` is the DEADLINE, not the moment the reactor noticed. A reactor that ticks late must not
/// make a case look less late than it was.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (sla-clock, "pause on hold")
pub async fn mark_breached(
    store: &Store,
    ws: &str,
    id: &str,
    at: u64,
    actor: &str,
    ts: u64,
) -> Result<Option<Case>, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    // Set once. This is the guard the whole file exists for — see the module doc.
    if case.breached_ts.is_some() {
        return Ok(None);
    }
    // A case that was resolved before its deadline did not breach. Note the ORDER: this is checked
    // after the set-once guard, so a case that breached and was LATER resolved keeps its breach.
    if case.closed {
        return Ok(None);
    }
    // A stopped clock does not breach: out of Open, or a pause still being booked. The deadline
    // moves if the case comes back to Open.
    if case.sla_paused_at.is_some() || !crate::sla_follow::is_open(&case, ts) {
        return Ok(None);
    }
    let Some(due_at) = case.due_at else {
        return Ok(None);
    };
    if at < due_at {
        return Ok(None);
    }

    case.breached_ts = Some(due_at);
    case.breach_waiting_on = case.waiting_on;
    let waiting_on = case.breach_waiting_on;
    save(store, ws, &mut case, ts).await?;

    append_event(
        store,
        ws,
        id,
        EventKind::Breach,
        actor,
        serde_json::json!({
            "due_at": due_at,
            "breached_ts": due_at,
            "waiting_on": waiting_on,
            "policy_id": case.policy_id,
            "workflow": case.workflow,
            "assigned_to": case.assigned_to,
        }),
        ts,
    )
    .await?;
    Ok(Some(case))
}
