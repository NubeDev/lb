//! `close_self_cleared_cases` — resolve untouched work whose insights all cleared by themselves.
//!
//! The case owns triage (case-plane scope, decision 5): a producer resolving its insight does NOT
//! close the case, because closing work is somebody's decision. Left alone that is honest but it
//! reads wrong at scale — a meter that went flat for a day and came back leaves a case in "to
//! action" for ever, the queue counts it as open work, and an export of the insights says
//! "resolved" beside a case that says the opposite.
//!
//! A service policy can take that decision once, for the work it governs:
//! `auto_close_self_cleared`. When it is on, this pass resolves a case as
//! [`lb_cases::Resolution::SelfCleared`] when BOTH hold:
//!
//!   1. **every** insight in the case is resolved — a grouped case with one member still firing is
//!      still live work, whatever its primary says;
//!   2. **nobody has worked it**: still `to_action`, no assignee, not waiting on anyone, not snoozed,
//!      never reopened by the hold-down, and no event in its history from anyone but the platform
//!      (`system:` actors — grouping, the SLA clock, the facet backfill). A comment, an assignment or
//!      a stage change by a person, including a comment the reconcile replayed from the insight, keeps
//!      the case for a person to close.
//!
//! It never reopens anything and never touches a closed case. A problem that comes back raises its
//! insight again, and the grouping pass opens a NEW case for it — `SelfCleared` is not held down
//! (only `Fixed` is, `hold_down.rs`), so the history reads as two episodes, which is what they were.
//!
//! Same shape as [`super::reconcile`] and [`super::facet_backfill`]: derived from the durable records
//! each tick, idempotent (a second pass closes nothing), best-effort per case. It reads the insights,
//! the open cases and the memberships ONCE per tick and decides in memory; only a case that passes
//! every cheap check has its history read.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lb_cases::{Case, CaseMember, Resolution, Workflow};
use lb_insights::{Insight, Status};
use lb_store::scan_all;

use super::error::CaseSvcError;
use crate::boot::Node;

/// The actor stamped on the resolution. Not a person: a policy decided this, the platform did it.
pub const SELF_CLEAR_ACTOR: &str = "system:self-clear";

/// Close every untouched open case, under a policy that opts in, whose insights are all resolved.
/// Returns how many cases were closed.
// SCOPE: docs/scope/insights/case-self-clear-scope.md
pub async fn close_self_cleared_cases(
    node: &Arc<Node>,
    ws: &str,
    now: u64,
) -> Result<usize, CaseSvcError> {
    // Which policies opted in. None ⇒ nothing to do, and no scan of the estate at all.
    let opted_in: HashSet<String> = lb_cases::policy_list(&node.store, ws, true)
        .await?
        .into_iter()
        .filter(|p| p.auto_close_self_cleared)
        .map(|p| p.id)
        .collect();
    if opted_in.is_empty() {
        return Ok(0);
    }

    let candidates: Vec<Case> = decode_all::<Case>(node, ws, lb_cases::CASE_TABLE)
        .await?
        .into_iter()
        .filter(|c| !c.closed)
        .filter(|c| c.policy_id.as_deref().is_some_and(|p| opted_in.contains(p)))
        .filter(untouched_fields)
        .collect();
    if candidates.is_empty() {
        return Ok(0);
    }

    let resolved: HashSet<String> = decode_all::<Insight>(node, ws, lb_insights::INSIGHT_TABLE)
        .await?
        .into_iter()
        .filter(|i| i.status == Status::Resolved)
        .map(|i| i.id)
        .collect();
    let mut members: HashMap<String, Vec<String>> = HashMap::new();
    for m in decode_all::<CaseMember>(node, ws, lb_cases::CASE_MEMBER_TABLE).await? {
        members.entry(m.case_id).or_default().push(m.insight_id);
    }

    let mut closed = 0usize;
    for case in candidates {
        // A case with no membership rows is still judged by its primary, so a partial write can
        // never make a live case look cleared.
        let insights = members
            .get(&case.id)
            .cloned()
            .unwrap_or_else(|| vec![case.primary_insight.clone()]);
        if !insights.iter().all(|id| resolved.contains(id)) {
            continue;
        }
        match history_is_platform_only(node, ws, &case.id).await {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                tracing::warn!(ws, case_id = %case.id, error = %e, "case self-clear: history unreadable");
                continue;
            }
        }
        match lb_cases::workflow(
            &node.store,
            ws,
            &case.id,
            Workflow::Resolved,
            Some(Resolution::SelfCleared),
            None,
            SELF_CLEAR_ACTOR,
            now,
        )
        .await
        {
            Ok(_) => closed += 1,
            // One case that will not close must not stop the pass; the next tick retries it.
            Err(e) => {
                tracing::warn!(ws, case_id = %case.id, error = %e, "case self-clear: resolve failed")
            }
        }
    }
    Ok(closed)
}

/// The cheap half of "nobody has worked it" — the fields on the case itself. Any of these is a
/// person's decision (or, for `reopened_count`, a hold-down reopen after a person said "fixed").
fn untouched_fields(c: &Case) -> bool {
    c.workflow == Workflow::ToAction
        && c.assigned_to.is_none()
        && c.waiting_on.is_none()
        && c.snooze_until.is_none()
        && c.snoozed_by.is_none()
        && c.reopened_count == 0
}

/// The expensive half: every event in the case's history came from the platform (`system:`).
async fn history_is_platform_only(
    node: &Arc<Node>,
    ws: &str,
    case_id: &str,
) -> Result<bool, CaseSvcError> {
    let mut after = None;
    loop {
        let page =
            lb_cases::events(&node.store, ws, case_id, lb_cases::MAX_EVENT_PAGE, after).await?;
        if page.items.iter().any(|e| !e.actor.starts_with("system:")) {
            return Ok(false);
        }
        match page.next {
            Some(n) => after = Some(n),
            None => return Ok(true),
        }
    }
}

/// Every row of `table`, unwrapped from the `{ data, rev }` envelope and decoded. A row that will
/// not decode is skipped: one bad record must not stop the pass (the `reconcile` precedent).
async fn decode_all<T: serde::de::DeserializeOwned>(
    node: &Arc<Node>,
    ws: &str,
    table: &str,
) -> Result<Vec<T>, CaseSvcError> {
    let rows = scan_all(&node.store, ws, table).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let inner = match row.data {
                serde_json::Value::Object(mut obj) => {
                    obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
                }
                other => other,
            };
            serde_json::from_value(inner).ok()
        })
        .collect())
}
