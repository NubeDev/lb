//! `react_to_flows_interval` — the durable **clock-scan** for the `flipflop` source node, the interval
//! sibling of [`super::react_cron`]. Same altitude/cadence: a stateless function over a durable set,
//! never a long-lived in-process timer (rule 4). A `flipflop` is a self-driving boolean oscillator —
//! no input, one output, flipping `true`/`false` every `period_secs`.
//!
//! One durable record per node ([`super::record::FlowTriggerState`]) holds BOTH the clock cursor
//! (`next_attempt_ts`, advanced by `period_secs`) AND the last emitted value (`flop`) — so value and
//! clock move together and both survive restart. Idempotency: a scheduled instant derives a
//! deterministic run id and is skipped if its job already exists (an at-least-once re-scan never
//! double-flips). Missed-firing policy — fire-once-then-skip-to-next-future-slot (no backfill storm).
//!
//! Workspace-walled at the scan (the flow directory is ws-scoped); a ws-B reactor never sees/fires a
//! ws-A flip-flop. `now` is the INJECTED clock (never wall-clock) — deterministic under test.

use std::sync::Arc;

use crate::boot::Node;

use super::error::FlowsError;
use super::react_cron::ReactorPass;
use super::record::FlowTriggerState;
use super::run;
use super::save::flows_list_internal;
use super::trigger_store::{flipflop_triggers, read_cursor, write_cursor, FlipFlopTrigger};

/// Run one interval-reactor pass over workspace `ws` at logical time `now`. Scans every `flipflop`
/// source node of every `enabled` flow; each due node (`next_attempt_ts ≤ now`) fires one run from
/// that node, emitting the flipped value, then advances its own cursor. Returns the pass tally
/// (shared [`ReactorPass`] shape with the cron reactor).
pub async fn react_to_flows_interval(
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
        for trig in flipflop_triggers(flow) {
            fire_one_flipflop(node, principal, ws, flow, &trig, now, &mut pass).await?;
        }
    }
    Ok(pass)
}

/// Drive a single flip-flop node's cursor one pass: init on first sight / on a period change, fire when
/// due (entry = this node → only its subgraph runs), flip the value, advance fire-once-then-skip.
async fn fire_one_flipflop(
    node: &Arc<Node>,
    principal: &lb_auth::Principal,
    ws: &str,
    flow: &lb_flows::Flow,
    trig: &FlipFlopTrigger,
    now: u64,
    pass: &mut ReactorPass,
) -> Result<(), FlowsError> {
    let node_id = &trig.node_id;
    let cursor = read_cursor(&node.store, ws, &flow.id, node_id)
        .await
        .map_err(FlowsError::Internal)?;
    // Initialise (or RE-initialise on a period edit): point the cursor at `now` so the FIRST value fires
    // on the next pass, seeding `flop = None` (→ emit `start`). A changed period re-seeds the value too.
    let needs_init = match &cursor {
        None => true,
        Some(c) => c.next_attempt_ts == 0 || c.period_secs != Some(trig.period_secs),
    };
    if needs_init {
        persist_cursor(node, ws, &flow.id, node_id, trig.period_secs, now, None).await?;
        return Ok(());
    }
    let cursor = cursor.expect("cursor present (needs_init handled None)");
    let scheduled_ts = cursor.next_attempt_ts;
    if scheduled_ts > now {
        return Ok(());
    }
    // The value to emit this firing: flip the last, or `start` on the very first firing.
    let value = match cursor.flop {
        Some(last) => !last,
        None => trig.start,
    };
    let run_id = flipflop_run_id(&flow.id, node_id, scheduled_ts);
    // Idempotency: a job already exists for this (flow, node, instant) → advance without re-flipping.
    if lb_jobs::load(&node.store, ws, &run_id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?
        .is_some()
    {
        let next = scheduled_ts + trig.period_secs;
        persist_cursor(
            node,
            ws,
            &flow.id,
            node_id,
            trig.period_secs,
            next,
            cursor.flop,
        )
        .await?;
        pass.skipped += 1;
        return Ok(());
    }
    // Fire one run FROM this node (entry = node_id → only its subgraph). The trigger leg reads its value
    // from params under the node id, exactly as the cron leg reads `cron_ts`.
    let mut params = serde_json::Map::new();
    params.insert(node_id.to_string(), serde_json::json!(value));
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
    // Advance the clock to the next slot AND persist the value just emitted (so the next firing flips it).
    let next = scheduled_ts + trig.period_secs;
    persist_cursor(
        node,
        ws,
        &flow.id,
        node_id,
        trig.period_secs,
        next,
        Some(value),
    )
    .await?;
    pass.fired += 1;
    Ok(())
}

/// A deterministic run id for a flip-flop firing: stable per (flow, node, scheduled instant).
pub fn flipflop_run_id(flow_id: &str, node_id: &str, scheduled_ts: u64) -> String {
    format!("{flow_id}-flip-{node_id}-{scheduled_ts}")
}

async fn persist_cursor(
    node: &Arc<Node>,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    period_secs: u64,
    next_attempt_ts: u64,
    flop: Option<bool>,
) -> Result<(), FlowsError> {
    let state = FlowTriggerState {
        next_attempt_ts,
        cron: None,
        period_secs: Some(period_secs),
        flop,
    };
    write_cursor(&node.store, ws, flow_id, node_id, &state)
        .await
        .map_err(FlowsError::Internal)
}
