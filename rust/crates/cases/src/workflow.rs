//! `workflow` — move a case between the four workflow states, and the one place the
//! resolution invariant is enforced (case-plane scope).
//!
//! **Two refusals, both directions of one rule:**
//!   - entering [`Workflow::Resolved`] WITHOUT a [`Resolution`] is `BadInput`, and
//!   - supplying a [`Resolution`] on any OTHER transition is `BadInput`.
//!
//! The first is the load-bearing one. "Closed" with no reason is how a queue becomes a graveyard:
//! nobody can tell a repair from a false positive six months later, the hold-down reactor cannot
//! decide whether a re-fire means the fix failed, and the scorecard has nothing to count. The
//! second exists so the two fields can never disagree — a `resolution` on an `actioned` case would
//! be a closure reason on an open job.
//!
//! Every transition appends a `case_event`, so the drawer shows the real sequence of states rather
//! than only the current one. Resolving appends [`EventKind::Resolved`] (the kind the audit and the
//! scorecard read); every other move appends [`EventKind::Workflow`].
//!
//! **A closing reason in the person's words** ([`workflow_with_reason`]) may ride the `resolved`
//! transition beside the closed `resolution` — "Fixed remotely" is a `fixed`, "No action required"
//! an `accepted_risk`. The resolution stays the fact the hold-down and the scorecard read; the reason
//! is what the person chose, kept so the history and the export can say it. Like the resolution it is
//! refused on any other transition and cleared on a reopen.
//!
//! `closed` is re-derived from the workflow on every write ([`crate::save`]) — leaving `resolved`
//! REOPENS the case, clearing `resolution`/`resolved_ts`/`resolved_by` so a reopened job never
//! carries the last repair's closure reason.

use lb_store::Store;

use crate::case::{Case, Resolution, WaitingOn, Workflow};
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// The longest closing reason accepted, in characters: a label or a short phrase, not a report.
pub const MAX_RESOLUTION_REASON: usize = 200;

/// Move case `id` in workspace `ws` to `next` as `actor` at logical ts `ts`.
///
/// `resolution` is REQUIRED for [`Workflow::Resolved`] and forbidden otherwise. `waiting_on` is
/// optional and orthogonal: `None` leaves the stored value alone, so a caller moving a case on does
/// not have to restate who they are waiting on.
///
/// **A stage change ends a hold.** A case is in one state at a time: moving it to another stage
/// (closing, reopening, sending it back to the queue) clears a running snooze first
/// ([`crate::snooze::end_hold`]), SLA pause and all. A move to the stage it is already in is not a
/// change and leaves the hold alone.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.workflow)
// Eight arguments, deliberately positional: every one is a distinct fact about the write, and a
// parameter struct here would add a type whose only job is to be destructured one line later. The
// host layer is the only caller.
#[allow(clippy::too_many_arguments)]
pub async fn workflow(
    store: &Store,
    ws: &str,
    id: &str,
    next: Workflow,
    resolution: Option<Resolution>,
    waiting_on: Option<WaitingOn>,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    workflow_with_reason(store, ws, id, next, resolution, waiting_on, None, actor, ts).await
}

/// [`workflow`], with the closing person's own `reason` for a `resolved` transition (see the module
/// note). A blank reason is no reason; one on any other transition, or longer than
/// [`MAX_RESOLUTION_REASON`], is `BadInput`.
#[allow(clippy::too_many_arguments)]
pub async fn workflow_with_reason(
    store: &Store,
    ws: &str,
    id: &str,
    next: Workflow,
    resolution: Option<Resolution>,
    waiting_on: Option<WaitingOn>,
    reason: Option<&str>,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    if let Some(r) = reason {
        if !next.is_terminal() {
            return Err(CasesError::BadInput(format!(
                "`reason` is only legal on the `resolved` transition, not `{next:?}`"
            )));
        }
        if r.chars().count() > MAX_RESOLUTION_REASON {
            return Err(CasesError::BadInput(format!(
                "`reason` is at most {MAX_RESOLUTION_REASON} characters"
            )));
        }
    }
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };

    match (next, resolution) {
        (Workflow::Resolved, None) => {
            return Err(CasesError::BadInput(
                "case.workflow(resolved) requires a `resolution` — a case closed with no reason \
                 cannot be told from a false positive later, and the hold-down window has nothing \
                 to judge"
                    .into(),
            ));
        }
        (state, Some(_)) if !state.is_terminal() => {
            return Err(CasesError::BadInput(format!(
                "`resolution` is only legal on the `resolved` transition, not `{state:?}` — a \
                 closure reason on an open case is two fields that disagree"
            )));
        }
        _ => {}
    }

    if next != case.workflow {
        case = crate::snooze::end_hold(store, ws, case, actor, ts).await?;
        // Closing ends the email-out too, and a reopened case starts clean: it is never "Emailed"
        // on the strength of a send made before it closed. (Any other move keeps it — the send's
        // own Open → Actioned step must not undo the send.)
        if next == Workflow::Resolved || case.workflow == Workflow::Resolved {
            case.waiting_on = None;
            case.emailed_by = None;
        }
    }
    let previous = case.workflow;
    case.workflow = next;
    if let Some(w) = waiting_on {
        case.waiting_on = Some(w);
    }

    if next.is_terminal() {
        case.resolution = resolution;
        case.resolution_reason = reason.map(str::to_string);
        case.resolved_ts = Some(ts);
        case.resolved_by = Some(actor.to_string());
    } else if previous.is_terminal() {
        // Leaving `resolved` is a REOPEN: the last repair's closure reason is history, not the
        // state of the job that is open again.
        case.resolution = None;
        case.resolution_reason = None;
        case.resolved_ts = None;
        case.resolved_by = None;
    }
    // A close, by hand or by a reactor, ends the work: the owner comes off, so a finding that comes
    // back starts the whole workflow again with nobody holding it. Who held it is kept for the
    // *Closed* lane (`owned_at_close`) and in the close event below.
    let owner_at_close = if next.is_terminal() && !previous.is_terminal() {
        let owner = case.assigned_to.take();
        case.assigned_by = None;
        case.owned_at_close = owner.clone();
        owner
    } else {
        None
    };
    // A job reopened to Open goes back to the queue: whoever picks it up owns it, as with a new one.
    if previous.is_terminal() && !next.is_terminal() {
        case.owned_at_close = None;
    }
    let unowned = (previous.is_terminal() && next == Workflow::ToAction)
        .then(|| case.assigned_to.take())
        .flatten();
    if unowned.is_some() {
        case.assigned_by = None;
    }

    save(store, ws, &mut case, ts).await?;

    let kind = if next.is_terminal() {
        EventKind::Resolved
    } else {
        EventKind::Workflow
    };
    append_event(
        store,
        ws,
        id,
        kind,
        actor,
        serde_json::json!({
            "from": previous,
            "to": next,
            "resolution": case.resolution,
            "reason": case.resolution_reason,
            "waiting_on": case.waiting_on,
            "owner": owner_at_close,
        }),
        ts,
    )
    .await?;
    if let Some(from) = unowned {
        append_event(
            store,
            ws,
            id,
            EventKind::Assigned,
            actor,
            serde_json::json!({ "from": from, "to": null }),
            ts,
        )
        .await?;
    }

    Ok(case)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::case::Grouping;
    use crate::open::{open, OpenInput};
    use lb_store::Store;

    async fn seed(store: &Store) -> String {
        open(
            store,
            "nube",
            OpenInput {
                title: "a job".into(),
                grouping: Grouping::Single,
                primary_insight: "i-1".into(),
                severity: "warning".into(),
                ..Default::default()
            },
            "user:test",
            1,
        )
        .await
        .unwrap()
        .id
    }

    /// Both directions of the one invariant, against a REAL store.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn resolved_needs_a_resolution_and_nothing_else_may_carry_one() {
        let store = Store::memory().await.unwrap();
        let id = seed(&store).await;

        assert!(matches!(
            workflow(
                &store,
                "nube",
                &id,
                Workflow::Resolved,
                None,
                None,
                "user:test",
                2
            )
            .await,
            Err(CasesError::BadInput(_))
        ));
        assert!(matches!(
            workflow(
                &store,
                "nube",
                &id,
                Workflow::Actioned,
                Some(Resolution::Fixed),
                None,
                "user:test",
                2
            )
            .await,
            Err(CasesError::BadInput(_))
        ));

        // Neither refusal moved the case.
        let case = crate::get::get(&store, "nube", &id).await.unwrap().unwrap();
        assert_eq!(case.workflow, Workflow::ToAction);
        assert!(!case.closed);
    }

    /// Leaving `resolved` is a REOPEN: the last repair's closure reason is history, not the state of
    /// a job that is open again. A stale `resolution` on a reopened case would make the hold-down
    /// reactor judge the wrong repair.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn reopening_clears_the_closure_reason() {
        let store = Store::memory().await.unwrap();
        let id = seed(&store).await;
        let closed = workflow(
            &store,
            "nube",
            &id,
            Workflow::Resolved,
            Some(Resolution::Fixed),
            None,
            "user:test",
            2,
        )
        .await
        .unwrap();
        assert!(closed.closed);
        assert_eq!(closed.resolved_by.as_deref(), Some("user:test"));

        let reopened = workflow(
            &store,
            "nube",
            &id,
            Workflow::ToAction,
            None,
            None,
            "system:case-group",
            3,
        )
        .await
        .unwrap();
        assert!(!reopened.closed);
        assert_eq!(reopened.resolution, None);
        assert_eq!(reopened.resolved_ts, None);
        assert_eq!(reopened.resolved_by, None);
    }
}
