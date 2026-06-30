//! `react_to_flows_cron` — the durable **clock-scan** for the `cron` trigger kind
//! (triggers-lifecycle-scope), modelled on the shipped `react_to_reminders`. Same altitude/cadence:
//! a stateless function over a durable set, never a long-lived in-process timer (rule 4).
//!
//! Idempotency — **one scheduled instant → one run.** The scan derives a deterministic run id from
//! `(flow_id, scheduled_ts)` and skips an instant whose job already exists — so an at-least-once
//! re-scan never double-fires. Missed-firing policy — **fire-once-then-skip-to-next-future-slot**
//! (no backfill storm): after an outage the scan fires ONCE for the due instant and advances
//! `next_attempt_ts` to the next slot strictly after `now`.
//!
//! Workspace-walled at the scan (the flow directory is ws-scoped); a ws-B reactor never sees/fires/
//! advances a ws-A flow. `cron` is stored as a 5-field spec; `next_after` is computed on the INJECTED
//! clock (never wall-clock) — deterministic under test.

use std::sync::Arc;

use lb_reminders::{is_valid, next_after};

use crate::boot::Node;

use super::error::FlowsError;
use super::run;
use super::save::flows_list_internal;

/// The outcome of one reactor pass: how many flows fired (and advanced), how many were skipped as
/// already-fired (the idempotent no-op).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReactorPass {
    pub fired: usize,
    pub skipped: usize,
}

/// Run one reactor pass over workspace `ws` at logical time `now`: for every `enabled` flow whose
/// `cron` trigger is `due` (`next_attempt_ts ≤ now`), enqueue one run (deterministic id), then
/// advance `next_attempt_ts` (fire-once-then-skip). Returns the pass tally.
pub async fn react_to_flows_cron(
    node: &Arc<Node>,
    principal: &lb_auth::Principal,
    ws: &str,
    now: u64,
) -> Result<ReactorPass, FlowsError> {
    let mut pass = ReactorPass::default();
    let mut flows = flows_list_full(node, ws).await?;
    for flow in flows.iter_mut() {
        if !flow.enabled {
            continue;
        }
        let Some(schedule) = flow.cron.clone() else {
            continue;
        };
        // Initialise next_attempt_ts on first sight (the next slot strictly after the flow was saved).
        if flow.next_attempt_ts == 0 {
            flow.next_attempt_ts = next_after(&schedule, now).unwrap_or(0);
            persist(node, ws, flow).await?;
            continue;
        }
        if flow.next_attempt_ts > now {
            continue;
        }
        let scheduled_ts = flow.next_attempt_ts;
        let run_id = cron_run_id(&flow.id, scheduled_ts);
        // Idempotency: a job already exists for this (flow, instant) → no-op (no double-fire).
        if lb_jobs::load(&node.store, ws, &run_id).await.map_err(|e| FlowsError::Internal(e.to_string()))?.is_some() {
            // Still advance so a re-scan before the next slot doesn't loop on this instant.
            flow.next_attempt_ts = next_after(&schedule, now).unwrap_or(scheduled_ts);
            persist(node, ws, flow).await?;
            pass.skipped += 1;
            continue;
        }
        // Fire one run pinned to the current version (Decision 1). Payload names the scheduled instant.
        let mut params = serde_json::Map::new();
        params.insert("__cron_ts".into(), serde_json::json!(scheduled_ts));
        run::flows_run(node, principal, ws, &flow.id, params, &run_id, now).await?;
        // Fire-once-then-skip: advance to the next slot strictly after NOW (no backfill storm).
        flow.next_attempt_ts = next_after(&schedule, now).unwrap_or(scheduled_ts);
        persist(node, ws, flow).await?;
        pass.fired += 1;
    }
    Ok(pass)
}

/// A deterministic run id for a cron firing: stable per (flow, scheduled instant).
pub fn cron_run_id(flow_id: &str, scheduled_ts: u64) -> String {
    format!("{flow_id}-cron-{scheduled_ts}")
}

async fn flows_list_full(node: &Arc<Node>, ws: &str) -> Result<Vec<lb_flows::Flow>, FlowsError> {
    flows_list_internal(&node.store, ws).await
}

async fn persist(node: &Arc<Node>, ws: &str, flow: &lb_flows::Flow) -> Result<(), FlowsError> {
    lb_store::write(
        &node.store,
        ws,
        super::record::FLOW_TABLE,
        &flow.id,
        &serde_json::to_value(flow).map_err(|e| FlowsError::Internal(e.to_string()))?,
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))
}

/// Re-export so callers can validate a cron spec before saving a flow.
pub fn cron_is_valid(schedule: &str) -> bool {
    is_valid(schedule)
}
