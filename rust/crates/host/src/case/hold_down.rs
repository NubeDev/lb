//! The **hold-down reactor** — "did the repair hold?" (case-plane scope).
//!
//! When a finding fires again after its case was closed, the platform has to answer one question
//! that a naive "open a new case" gets wrong in the most expensive direction:
//!
//! > Was the last thing we did a REPAIR, and has it just failed?
//!
//! If the case closed as [`Resolution::Fixed`] and the fault is back inside the policy's
//! `hold_down_days`, that is not new work — it is **the same job, unfinished**. Opening a fresh
//! case would reset the clock, hide the failure from the scorecard, and let a contractor be paid
//! twice for a repair that never held. So the old case is REOPENED, `reopened_count` goes up, and a
//! `reopened` event records *repair did not hold*.
//!
//! Every other resolution lets a new case open, and each for a real reason: `self_cleared` and
//! `accepted_risk` were never repairs, `false_positive` says the detection was wrong (a re-fire is
//! evidence about the rule, not about a fix), and `duplicate` closed a case that was folded away.
//!
//! `hold_down_days` comes from the matching `service_policy` — most specific match wins — with the
//! crate's own default when a workspace has seeded no policy at all.

use std::sync::Arc;

use lb_cases::{Case, EventKind, Resolution, Workflow};
use lb_insights::Insight;

use super::error::CaseSvcError;
use super::group::GROUP_ACTOR;
use crate::boot::Node;

/// Milliseconds in a day — the unit `hold_down_days` is stated in, converted once.
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// Reopen the case that last repaired `insight` if the repair did not hold, returning its id.
///
/// `None` means "let the grouping reactor open a new case": there was no previous case, it closed
/// as something other than `fixed`, or the hold-down window has expired.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (hold-down)
pub async fn reopen_if_held(
    node: &Arc<Node>,
    ws: &str,
    insight: &Insight,
    now: u64,
) -> Result<Option<String>, CaseSvcError> {
    let store = &node.store;
    let Some(previous) = lb_cases::last_closed_case_for_insight(store, ws, &insight.id).await?
    else {
        return Ok(None);
    };
    if previous.resolution != Some(Resolution::Fixed) {
        return Ok(None);
    }
    let Some(resolved_ts) = previous.resolved_ts else {
        // A `fixed` case with no close time cannot be judged against a window. Treat it as expired
        // rather than reopening on a guess — a wrong reopen is louder than a wrong new case.
        return Ok(None);
    };
    let days = hold_down_days(node, ws, &previous).await?;
    if now > resolved_ts.saturating_add(u64::from(days) * DAY_MS) {
        return Ok(None);
    }

    // Reopen: `workflow` clears `resolution`/`resolved_ts`/`resolved_by` and re-derives `closed`,
    // so there is one implementation of what "not resolved any more" means.
    lb_cases::workflow(
        store,
        ws,
        &previous.id,
        Workflow::ToAction,
        None,
        None,
        GROUP_ACTOR,
        now,
    )
    .await?;
    bump_reopened_count(node, ws, &previous.id, now).await?;
    lb_cases::append_event(
        store,
        ws,
        &previous.id,
        EventKind::Reopened,
        GROUP_ACTOR,
        serde_json::json!({
            "reason": "repair did not hold",
            "insight_id": insight.id,
            "resolved_ts": resolved_ts,
            "hold_down_days": days,
        }),
        now,
    )
    .await?;
    Ok(Some(previous.id))
}

/// The `hold_down_days` for this case, from the most specific matching `service_policy`.
async fn hold_down_days(node: &Arc<Node>, ws: &str, case: &Case) -> Result<u32, CaseSvcError> {
    let matched = lb_cases::match_policy(
        &node.store,
        ws,
        case.site.as_deref(),
        case.category.as_deref(),
        Some(&case.severity),
    )
    .await?;
    Ok(matched.map_or(lb_cases::DEFAULT_HOLD_DOWN_DAYS, |p| p.hold_down_days))
}

/// Increment `reopened_count`. Its own step because the reopen is a `workflow` transition and the
/// count is not a workflow fact — folding it into that verb would put "how many times did a repair
/// fail" inside a function whose one job is the state machine.
async fn bump_reopened_count(
    node: &Arc<Node>,
    ws: &str,
    case_id: &str,
    now: u64,
) -> Result<(), CaseSvcError> {
    let Some(mut case) = lb_cases::get(&node.store, ws, case_id).await? else {
        return Ok(());
    };
    case.reopened_count = case.reopened_count.saturating_add(1);
    case.last_activity_ts = now;
    let value = serde_json::to_value(&case).map_err(|e| CaseSvcError::Store(e.to_string()))?;
    lb_store::write(&node.store, ws, lb_cases::CASE_TABLE, &case.id, &value).await?;
    Ok(())
}
