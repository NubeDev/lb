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
use super::record::FlowTriggerState;
use super::run;
use super::save::flows_list_internal;
use super::trigger_store::{cron_triggers, read_cursor, write_cursor};

/// The outcome of one reactor pass: how many trigger nodes fired (and advanced), how many were
/// skipped as already-fired (the idempotent no-op).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReactorPass {
    pub fired: usize,
    pub skipped: usize,
}

/// Run one reactor pass over workspace `ws` at logical time `now`. Scans **every cron trigger node**
/// of every `enabled` flow — NOT one schedule per flow — so a flow with N cron triggers fires each on
/// its own clock. For each due trigger (`next_attempt_ts ≤ now`) it fires one run **from that node**
/// (so only that trigger's subgraph executes, Node-RED per-wire) with a deterministic id, then
/// advances **that node's** cursor (fire-once-then-skip). Returns the pass tally.
pub async fn react_to_flows_cron(
    node: &Arc<Node>,
    principal: &lb_auth::Principal,
    ws: &str,
    now: u64,
) -> Result<ReactorPass, FlowsError> {
    let mut pass = ReactorPass::default();
    let flows = flows_list_internal(&node.store, ws).await?;
    for flow in &flows {
        if !flow.enabled {
            continue;
        }
        // Each cron trigger node is independent: its own schedule (its `config.cron`) + its own cursor.
        for trig in cron_triggers(flow) {
            fire_one_trigger(
                node,
                principal,
                ws,
                flow,
                &trig.node_id,
                &trig.cron,
                now,
                &mut pass,
            )
            .await?;
        }
    }
    Ok(pass)
}

/// Drive a single cron trigger node's cursor one pass: init on first sight / on a schedule change,
/// fire when due (entry = this node → only its subgraph runs), advance fire-once-then-skip.
#[allow(clippy::too_many_arguments)]
async fn fire_one_trigger(
    node: &Arc<Node>,
    principal: &lb_auth::Principal,
    ws: &str,
    flow: &lb_flows::Flow,
    node_id: &str,
    schedule: &str,
    now: u64,
    pass: &mut ReactorPass,
) -> Result<(), FlowsError> {
    let cursor = read_cursor(&node.store, ws, &flow.id, node_id)
        .await
        .map_err(FlowsError::Internal)?;
    // Initialise (or RE-initialise on a schedule edit): point the cursor at the next slot strictly
    // after `now`. A changed spec resets the cursor so a stale instant for the OLD spec never fires.
    let needs_init = match &cursor {
        None => true,
        Some(c) => c.next_attempt_ts == 0 || c.cron.as_deref() != Some(schedule),
    };
    if needs_init {
        let next = next_after(schedule, now).unwrap_or(0);
        persist_cursor(node, ws, &flow.id, node_id, schedule, next).await?;
        return Ok(());
    }
    let scheduled_ts = cursor.map(|c| c.next_attempt_ts).unwrap_or(0);
    if scheduled_ts > now {
        return Ok(());
    }
    let run_id = cron_run_id(&flow.id, node_id, scheduled_ts);
    // Idempotency: a job already exists for this (flow, node, instant) → no-op (no double-fire).
    if lb_jobs::load(&node.store, ws, &run_id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?
        .is_some()
    {
        let next = next_after(schedule, now).unwrap_or(scheduled_ts);
        persist_cursor(node, ws, &flow.id, node_id, schedule, next).await?;
        pass.skipped += 1;
        return Ok(());
    }
    // Fire one run FROM this trigger node (entry = node_id → only its subgraph). Payload names the
    // scheduled instant; the trigger node emits it as its output.
    let mut params = serde_json::Map::new();
    params.insert("__cron_ts".into(), serde_json::json!(scheduled_ts));
    params.insert(
        node_id.to_string(),
        serde_json::json!({ "cron_ts": scheduled_ts }),
    );
    run::flows_run(
        node,
        principal,
        ws,
        &flow.id,
        params,
        &run_id,
        now,
        Some(node_id),
    )
    .await?;
    // Fire-once-then-skip: advance to the next slot strictly after NOW (no backfill storm).
    let next = next_after(schedule, now).unwrap_or(scheduled_ts);
    persist_cursor(node, ws, &flow.id, node_id, schedule, next).await?;
    pass.fired += 1;
    Ok(())
}

/// A deterministic run id for a cron firing: stable per (flow, **node**, scheduled instant). The node
/// segment is what lets two cron triggers in one flow fire at the same instant without colliding ids.
pub fn cron_run_id(flow_id: &str, node_id: &str, scheduled_ts: u64) -> String {
    format!("{flow_id}-cron-{node_id}-{scheduled_ts}")
}

async fn persist_cursor(
    node: &Arc<Node>,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    schedule: &str,
    next_attempt_ts: u64,
) -> Result<(), FlowsError> {
    let state = FlowTriggerState {
        next_attempt_ts,
        cron: Some(schedule.to_string()),
    };
    write_cursor(&node.store, ws, flow_id, node_id, &state)
        .await
        .map_err(FlowsError::Internal)
}

/// Re-export so callers can validate a cron spec before saving a flow.
pub fn cron_is_valid(schedule: &str) -> bool {
    is_valid(schedule)
}
