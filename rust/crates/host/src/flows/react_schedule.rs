//! `react_to_flows_schedule` — the durable **clock-scan** for the `schedule` source node, sibling to
//! [`super::react_cron`] and [`super::react_interval`]. Same altitude/cadence: a stateless function
//! over a durable set, never a long-lived in-process timer (rule 4). This is the deliberate departure
//! from the Go node, which held a `time.Ticker` per node and re-emitted every tick: a ticker dies with
//! the process, drifts, and cannot be reasoned about after a restart.
//!
//! **Edge-triggered.** Each pass evaluates the node's referenced *global* schedule and compares the
//! result with the last state in the node's own durable cursor
//! ([`super::record::FlowTriggerState::schedule_active`]). A run fires only on a **transition**
//! (inactive→active or active→inactive), so runs stay proportional to real schedule changes rather
//! than to scan frequency. `emit_interval` opts a node into an additional per-evaluation heartbeat
//! without changing those transition semantics.
//!
//! The cursor holds the evaluated state AND the `schedule_id` it was evaluated against, so re-pointing
//! a node at a different schedule re-seeds it — a stale `true` from the old schedule can never swallow
//! the first transition of the new one. Restart-safe: the state is durable, so a transition that
//! already fired is not replayed, and one that happened while the node was down fires on the next pass.
//!
//! Workspace-walled at the scan; `now` is the INJECTED clock (never wall-clock) — deterministic
//! under test.

use std::sync::Arc;

use crate::boot::Node;

use super::error::FlowsError;
use super::react_cron::ReactorPass;
use super::record::FlowTriggerState;
use super::run;
use super::save::flows_list_internal;
use super::schedule_store::read_schedule_internal;
use super::trigger_store::{read_cursor, schedule_triggers, write_cursor, ScheduleTrigger};

/// Run one schedule-reactor pass over workspace `ws` at logical time `now`. Scans every `schedule`
/// source node of every `enabled` flow; each due node re-evaluates its schedule and fires when the
/// active state changed (or every interval when `emit_interval`).
pub async fn react_to_flows_schedule(
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
        for trig in schedule_triggers(flow) {
            // Per-node isolation: one missing schedule / one broken flow must not starve every other
            // trigger this pass. Log and keep scanning; the next tick retries the failed one.
            if let Err(e) =
                fire_one_schedule(node, principal, ws, flow, &trig, now, &mut pass).await
            {
                tracing::warn!(
                    ws = %ws, flow = %flow.id, node = %trig.node_id, schedule = %trig.schedule_id,
                    error = %e, "schedule evaluation failed; continuing the pass"
                );
            }
        }
    }
    Ok(pass)
}

/// Fire **one named** schedule node, if it is still a live trigger on a still-enabled flow. The seam a
/// per-node timer drives, mirroring [`super::react_interval::fire_flipflop_node`] — one owner for the
/// fire logic, so a timer-driven firing and a sweep-driven one are the same code.
pub async fn fire_schedule_node(
    node: &Arc<Node>,
    principal: &lb_auth::Principal,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    now: u64,
) -> Result<ReactorPass, FlowsError> {
    let mut pass = ReactorPass::default();
    let flows = flows_list_internal(&node.store, ws).await?;
    let Some(flow) = flows.iter().find(|f| f.id == flow_id && f.enabled) else {
        return Ok(pass);
    };
    let Some(trig) = schedule_triggers(flow)
        .into_iter()
        .find(|t| t.node_id == node_id)
    else {
        return Ok(pass);
    };
    fire_one_schedule(node, principal, ws, flow, &trig, now, &mut pass).await?;
    Ok(pass)
}

/// Drive a single schedule node's cursor one pass: init on first sight / on a schedule re-point,
/// evaluate when due, fire on a transition (or on every interval when `emit_interval`), advance.
async fn fire_one_schedule(
    node: &Arc<Node>,
    principal: &lb_auth::Principal,
    ws: &str,
    flow: &lb_flows::Flow,
    trig: &ScheduleTrigger,
    now: u64,
    pass: &mut ReactorPass,
) -> Result<(), FlowsError> {
    let node_id = &trig.node_id;
    let cursor = read_cursor(&node.store, ws, &flow.id, node_id)
        .await
        .map_err(FlowsError::Internal)?;

    // Initialise (or RE-initialise when the node is re-pointed at a different schedule, or its
    // interval is edited): seed the cursor at `now` with NO known state, so the next pass evaluates
    // and treats the first result as a transition. Re-seeding on a `schedule_id` change is what stops
    // the previous schedule's stale `true` from swallowing the new schedule's first transition.
    let needs_init = match &cursor {
        None => true,
        Some(c) => {
            c.next_attempt_ts == 0
                || c.schedule_id.as_deref() != Some(trig.schedule_id.as_str())
                || c.period_secs != Some(trig.evaluation_interval)
        }
    };
    if needs_init {
        persist_cursor(node, ws, &flow.id, node_id, trig, now, None).await?;
        return Ok(());
    }
    let cursor = cursor.expect("cursor present (needs_init handled None)");
    let scheduled_ts = cursor.next_attempt_ts;
    if scheduled_ts > now {
        return Ok(());
    }

    // Evaluate the referenced GLOBAL schedule. A schedule that has been deleted (or was never
    // created) evaluates as inactive rather than erroring: one removed record must not wedge every
    // flow that referenced it, and an operator deleting a schedule reasonably means "nothing is on".
    let record = read_schedule_internal(&node.store, ws, &trig.schedule_id).await?;
    let active = match &record {
        Some(r) => r.evaluate()?.is_active,
        None => {
            tracing::warn!(
                ws = %ws, flow = %flow.id, node = %node_id, schedule = %trig.schedule_id,
                "schedule node references a schedule that does not exist; treating as inactive"
            );
            false
        }
    };
    // `invert` is applied to the EMITTED value only, after the transition test, so an inverted node
    // still fires on exactly the same instants as a non-inverted one.
    let value = active != trig.invert;

    let changed = cursor.schedule_active != Some(active);
    let next = next_slot_after(scheduled_ts, trig.evaluation_interval, now);

    if !changed && !trig.emit_interval {
        // No transition and no heartbeat requested: advance the clock, keep the state, fire nothing.
        persist_cursor(node, ws, &flow.id, node_id, trig, next, Some(active)).await?;
        pass.skipped += 1;
        return Ok(());
    }

    let run_id = schedule_run_id(&flow.id, node_id, scheduled_ts);
    // Idempotency: a job already exists for this (flow, node, instant) → advance without re-firing.
    if lb_jobs::load(&node.store, ws, &run_id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?
        .is_some()
    {
        persist_cursor(node, ws, &flow.id, node_id, trig, next, Some(active)).await?;
        pass.skipped += 1;
        return Ok(());
    }

    // Fire one run FROM this node (entry = node_id → only its subgraph). The trigger leg reads its
    // value from params under the node id, exactly as the cron/flipflop legs do. SPAWNED, not awaited:
    // the run seeds durably (so the idempotency check above holds) and drives on a detached task.
    let mut params = serde_json::Map::new();
    params.insert(node_id.to_string(), serde_json::json!(value));
    run::flows_run_async(
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

    persist_cursor(node, ws, &flow.id, node_id, trig, next, Some(active)).await?;
    pass.fired += 1;
    Ok(())
}

/// A deterministic run id for a schedule firing: stable per (flow, node, scheduled instant).
pub fn schedule_run_id(flow_id: &str, node_id: &str, scheduled_ts: u64) -> String {
    format!("{flow_id}-sched-{node_id}-{scheduled_ts}")
}

/// The next slot on the evaluation grid anchored at `scheduled_ts` that lies **strictly after** `now`.
/// Identical discipline to the interval reactor's: a scan arriving late advances past `now` in ONE
/// step, so a stalled/slow sweep never leaves the cursor permanently behind the wall clock.
fn next_slot_after(scheduled_ts: u64, interval_secs: u64, now: u64) -> u64 {
    let period = interval_secs.max(1);
    if scheduled_ts > now {
        return scheduled_ts;
    }
    let elapsed = now - scheduled_ts;
    scheduled_ts + (elapsed / period + 1) * period
}

async fn persist_cursor(
    node: &Arc<Node>,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    trig: &ScheduleTrigger,
    next_attempt_ts: u64,
    schedule_active: Option<bool>,
) -> Result<(), FlowsError> {
    let state = FlowTriggerState {
        next_attempt_ts,
        period_secs: Some(trig.evaluation_interval),
        schedule_active,
        schedule_id: Some(trig.schedule_id.clone()),
        // Cron/flip-flop/webhook cursor fields are inert for a schedule source.
        ..Default::default()
    };
    write_cursor(&node.store, ws, flow_id, node_id, &state)
        .await
        .map_err(FlowsError::Internal)
}

#[cfg(test)]
mod tests {
    use super::next_slot_after;

    /// The on-time scan: evaluating exactly at the slot advances by exactly one interval.
    #[test]
    fn on_time_scan_advances_one_interval() {
        assert_eq!(next_slot_after(100, 10, 100), 110);
    }

    /// A scan later than the slot lands the cursor strictly in the future in ONE step — never
    /// `scheduled + interval`, which would slide further behind the wall clock on every tick.
    #[test]
    fn late_scan_skips_to_the_next_future_slot() {
        assert_eq!(next_slot_after(100, 10, 157), 160);
        assert_eq!(next_slot_after(100, 1, 157), 158);
        assert_eq!(next_slot_after(100, 10, 160), 170);
    }

    /// A not-yet-due slot is returned unchanged.
    #[test]
    fn future_slot_is_kept() {
        assert_eq!(next_slot_after(200, 10, 150), 200);
    }

    /// A corrupt zero interval neither divides by zero nor pins the cursor in the past.
    #[test]
    fn zero_interval_still_advances() {
        assert_eq!(next_slot_after(100, 0, 100), 101);
    }
}
