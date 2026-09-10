//! `reconcile_cases` — the restart-safe backstop AND the triage backfill (case-plane scope).
//!
//! It does one thing: **every OPEN insight with no open case gets one.** That is the invariant the
//! inline grouping at raise maintains going forward; this is what makes it true for records that
//! were raised before the case plane existed, and for the window where a node died between the
//! insight write and the grouping call.
//!
//! It is also the **migration**. Resolved decision 5 makes the case the owner of triage, so the
//! backfill must carry the existing human state across or the platform would appear to have thrown
//! it away on upgrade:
//!   - `insight.assigned_to` becomes the case's `assigned_to` (via [`super::group::open_for`]), and
//!   - every comment on the insight is replayed into the case's history as a `comment` event,
//!     **keeping its original author and timestamp** — a migration that restamped every note with
//!     `system:` and today's date would destroy exactly the record it was meant to preserve.
//!
//! **Idempotent.** A second pass returns `0`: every insight it touched now has an open case, so the
//! grouping call returns that case and this loop skips it. It re-derives rather than remembers,
//! which is what makes it safe to run on a timer for ever.

use std::sync::Arc;

use lb_cases::EventKind;
use lb_insights::{Insight, Status};
use lb_store::scan_all;

use super::error::CaseSvcError;
use super::group::group_insight;
use crate::boot::Node;

/// Give every open, ungrouped insight in `ws` a case. Returns how many were newly grouped.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (the reconcile backstop + backfill)
pub async fn reconcile_cases(node: &Arc<Node>, ws: &str, now: u64) -> Result<usize, CaseSvcError> {
    let mut grouped = 0usize;
    for insight in open_insights(node, ws).await? {
        // Ask the SAME question the inline path asks, so the two cannot drift.
        if lb_cases::find_open_case_for_insight(&node.store, ws, &insight.id)
            .await?
            .is_some()
        {
            continue;
        }
        let case_id = match group_insight(node, ws, &insight.id, now).await {
            Ok(id) => id,
            Err(e) => {
                // One un-groupable finding must not stop the pass — the next tick retries it.
                tracing::warn!(ws, insight_id = %insight.id, error = %e, "case reconcile: grouping failed");
                continue;
            }
        };
        backfill_comments(node, ws, &insight, &case_id).await;
        grouped += 1;
    }
    Ok(grouped)
}

/// Every OPEN insight in the workspace. `acked` counts as open work — it means somebody has SEEN
/// the finding, not that it is done; excluding it would leave the acknowledged half of the estate
/// without cases, which is the half people are actually working.
async fn open_insights(node: &Arc<Node>, ws: &str) -> Result<Vec<Insight>, CaseSvcError> {
    let rows = scan_all(&node.store, ws, lb_insights::INSIGHT_TABLE).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| unwrap_insight(row.data))
        .filter(|i| i.status != Status::Resolved)
        .collect())
}

/// Unwrap the `{ data, rev }` write envelope and decode. A row that will not decode is skipped: one
/// bad record must not stop the whole workspace being reconciled.
fn unwrap_insight(row: serde_json::Value) -> Option<Insight> {
    let inner = match row {
        serde_json::Value::Object(mut obj) => {
            obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
        }
        other => other,
    };
    serde_json::from_value(inner).ok()
}

/// Replay the insight's comment thread into the case history, preserving author and timestamp.
///
/// Best-effort: the case and its membership are already durable, so a failed copy is a thread the
/// next pass over a still-ungrouped insight would retry — never a reason to fail the reconcile.
async fn backfill_comments(node: &Arc<Node>, ws: &str, insight: &Insight, case_id: &str) {
    let thread = match lb_insights::comments(&node.store, ws, &insight.id).await {
        Ok(thread) => thread,
        Err(e) => {
            tracing::warn!(ws, insight_id = %insight.id, error = %e,
                "case backfill: insight comment thread unreadable");
            return;
        }
    };
    // Oldest-first, so the case history reads in the order the conversation actually happened
    // (`lb_insights::comments` returns newest-first, the order a responder reads a thread).
    for comment in thread.into_iter().rev() {
        if let Err(e) = lb_cases::append_event(
            &node.store,
            ws,
            case_id,
            EventKind::Comment,
            &comment.author,
            serde_json::json!({ "text": comment.text, "backfilled_from_insight": insight.id }),
            comment.ts,
        )
        .await
        {
            tracing::warn!(ws, case_id, insight_id = %insight.id, error = %e,
                "case backfill: comment not copied");
        }
    }
}
