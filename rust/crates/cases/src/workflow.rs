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
//! `closed` is re-derived from the workflow on every write ([`crate::save`]) — leaving `resolved`
//! REOPENS the case, clearing `resolution`/`resolved_ts`/`resolved_by` so a reopened job never
//! carries the last repair's closure reason.

use lb_store::Store;

use crate::case::{Case, Resolution, WaitingOn, Workflow};
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// Move case `id` in workspace `ws` to `next` as `actor` at logical ts `ts`.
///
/// `resolution` is REQUIRED for [`Workflow::Resolved`] and forbidden otherwise. `waiting_on` is
/// optional and orthogonal: `None` leaves the stored value alone, so a caller moving
/// `waiting_on_po → actioned` does not have to restate who they are waiting on.
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

    let previous = case.workflow;
    case.workflow = next;
    if let Some(w) = waiting_on {
        case.waiting_on = Some(w);
    }

    if next.is_terminal() {
        case.resolution = resolution;
        case.resolved_ts = Some(ts);
        case.resolved_by = Some(actor.to_string());
    } else if previous.is_terminal() {
        // Leaving `resolved` is a REOPEN: the last repair's closure reason is history, not the
        // state of the job that is open again.
        case.resolution = None;
        case.resolved_ts = None;
        case.resolved_by = None;
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
            "waiting_on": case.waiting_on,
        }),
        ts,
    )
    .await?;

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
