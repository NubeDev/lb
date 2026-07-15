//! The per-flow **concurrency guard** (rules-workflow-convergence scope, slice 2). Before a firing
//! starts a run, the guard asks "does a live run of this flow already exist?" and applies the flow's
//! [`Concurrency`] policy: `Skip` drops the firing, `Queue` lets it start (overlap allowed), `Restart`
//! cancels the live run(s) then starts. Enforced at BOTH fire seams — the cron reactor and `flows.run`
//! — so a slow run never silently overlaps itself regardless of how it was triggered.
//!
//! "Live" = a run whose durable status is non-terminal (`pending`/`running`/`suspended`); a
//! `success`/`failed`/`cancelled`/`partialFailure` run is finished and never blocks a new firing. The
//! scan is workspace-scoped (`FLOW_RUN_TABLE` under `ws`), so a ws-B firing never sees or cancels a
//! ws-A run (the hard wall §7).

use std::sync::Arc;

use lb_flows::Concurrency;
use serde_json::Value;

use crate::boot::Node;

use super::error::FlowsError;
use super::record::FLOW_RUN_TABLE;
use super::run_store::set_run_status;
use super::scan_all::scan_all;

/// The terminal run statuses — a run in one of these is finished and does not count as live.
const TERMINAL: &[&str] = &["success", "failed", "cancelled", "partialFailure"];

/// What the guard decided a firing should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireDecision {
    /// No live run (or `Queue`) — start the new run normally.
    Start,
    /// `Skip` and a live run exists — drop this firing (no run starts).
    Skip,
}

/// Apply flow `flow_id`'s concurrency `policy` before a new firing. Returns [`FireDecision::Skip`] iff
/// the policy is `Skip` AND a live run exists; otherwise [`FireDecision::Start`] (having cancelled the
/// live run(s) first when the policy is `Restart`). `new_run_id` is excluded from the live scan so
/// re-driving an already-seeded run is never blocked by itself (idempotent fire). No wall-clock: the
/// cancel transition is a logical state write.
pub async fn apply_concurrency(
    node: &Arc<Node>,
    ws: &str,
    flow_id: &str,
    policy: Concurrency,
    new_run_id: &str,
) -> Result<FireDecision, FlowsError> {
    // `Queue` never inspects live runs — overlap is allowed, so the firing always starts.
    if policy == Concurrency::Queue {
        return Ok(FireDecision::Start);
    }
    let live = live_run_ids(node, ws, flow_id, new_run_id).await?;
    if live.is_empty() {
        return Ok(FireDecision::Start);
    }
    match policy {
        Concurrency::Skip => Ok(FireDecision::Skip),
        Concurrency::Restart => {
            // Cancel every live run so the drive loop halts it at its next control check (the same
            // seam `flows.cancel` uses), then start the new one. Terminal write is idempotent.
            for run_id in live {
                let _ = set_run_status(&node.store, ws, &run_id, "cancelled").await;
            }
            Ok(FireDecision::Start)
        }
        Concurrency::Queue => Ok(FireDecision::Start),
    }
}

/// The ids of every LIVE (non-terminal) run of `flow_id` in `ws`, excluding `exclude` (the run about
/// to start / being re-driven). A ws-scoped scan — never another workspace's runs.
async fn live_run_ids(
    node: &Arc<Node>,
    ws: &str,
    flow_id: &str,
    exclude: &str,
) -> Result<Vec<String>, FlowsError> {
    let rows = scan_all(&node.store, ws, FLOW_RUN_TABLE)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let mut live = Vec::new();
    for row in rows {
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if inner.get("flow_id").and_then(|v| v.as_str()) != Some(flow_id) {
            continue;
        }
        let run_id = inner.get("run_id").and_then(|v| v.as_str()).unwrap_or("");
        if run_id.is_empty() || run_id == exclude {
            continue;
        }
        let status = inner.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if !TERMINAL.contains(&status) {
            live.push(run_id.to_string());
        }
    }
    Ok(live)
}
