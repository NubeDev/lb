//! `close_self_cleared_cases` — close open work whose insights all cleared by themselves.
//!
//! The case owns triage (case-plane scope, decision 5): a producer resolving its insight does NOT
//! close the case on the spot. Left alone that reads wrong at scale — a meter that went flat for a
//! day and came back leaves a case open for ever, the queue counts it as work, and an export of the
//! insights says "resolved" beside a case that says the opposite.
//!
//! A service policy takes that decision once, for the work it governs: `auto_close_self_cleared`.
//! When it is on, this pass resolves a case as [`lb_cases::Resolution::SelfCleared`] as soon as
//! **every** insight in the case is resolved — a grouped case with one member still firing is still
//! live work, whatever its primary says.
//!
//! **From any stage** (Insights review 2026-10-29). It used to close only work nobody had touched;
//! an assigned, emailed or held case stayed open after its problem went away, and the reader had a
//! hollow dot to tell the two apart. Now the problem going away closes the case wherever it is: the
//! owner stays on the closed case (who had it), a hold ends (the close path ends it), and an ask
//! still out to a party is withdrawn in the same pass, so the contractor's link dies with the job and
//! the nudge ladder stops chasing them. A REPLIED ask stays as it is: the answer is history.
//!
//! It never reopens anything and never touches a closed case. A problem that comes back raises its
//! insight again, and the grouping pass opens a NEW case for it — `SelfCleared` is not held down
//! (only `Fixed` is, `hold_down.rs`), so the history reads as two episodes, which is what they were.
//!
//! Same shape as [`super::reconcile`] and [`super::facet_backfill`]: derived from the durable records
//! each tick, idempotent (a second pass closes nothing), best-effort per case. It reads the insights,
//! the open cases and the memberships ONCE per tick and decides in memory.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lb_cases::{Case, CaseMember, EventKind, Resolution, Workflow};
use lb_insights::{Insight, Status};
use lb_store::scan_all;

use super::error::CaseSvcError;
use super::request_nudge_schedule::cancel_nudges;
use crate::boot::Node;

/// The actor stamped on the resolution. Not a person: a policy decided this, the platform did it.
pub const SELF_CLEAR_ACTOR: &str = "system:self-clear";

/// Close every open case, under a policy that opts in, whose insights are all resolved, and withdraw
/// the asks still out on it. Returns how many cases were closed.
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
            Ok(_) => {
                closed += 1;
                withdraw_open_asks(node, ws, &case.id, now).await;
            }
            // One case that will not close must not stop the pass; the next tick retries it.
            Err(e) => {
                tracing::warn!(ws, case_id = %case.id, error = %e, "case self-clear: resolve failed")
            }
        }
    }
    Ok(closed)
}

/// Withdraw every ask still out on a case that just closed itself: the token dies with the job and
/// the nudge ladder stops. Best-effort per ask, like the pass: one that will not withdraw is logged.
async fn withdraw_open_asks(node: &Arc<Node>, ws: &str, case_id: &str, now: u64) {
    let asks = match lb_cases::requests_of_case(&node.store, ws, case_id).await {
        Ok(asks) => asks,
        Err(e) => {
            tracing::warn!(ws, case_id, error = %e, "case self-clear: asks unreadable");
            return;
        }
    };
    for mut ask in asks.into_iter().filter(|r| r.status.is_live()) {
        match lb_cases::request_withdraw(&node.store, ws, &mut ask, now).await {
            Ok(true) => {
                cancel_nudges(&node.store, ws, &ask.id).await;
                let event = serde_json::json!({ "request_id": ask.id, "party_id": ask.party_id });
                if let Err(e) = lb_cases::append_event(
                    &node.store,
                    ws,
                    case_id,
                    EventKind::RequestWithdrawn,
                    SELF_CLEAR_ACTOR,
                    event,
                    now,
                )
                .await
                {
                    tracing::warn!(ws, case_id, error = %e, "case self-clear: withdraw event not written");
                }
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(ws, case_id, request_id = %ask.id, error = %e, "case self-clear: ask not withdrawn")
            }
        }
    }
}

/// Every row of `table`, unwrapped from the `{ data, rev }` envelope and decoded. A row that will
/// not decode is skipped: one bad record must not stop the pass (the `reconcile` precedent).
pub(super) async fn decode_all<T: serde::de::DeserializeOwned>(
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
