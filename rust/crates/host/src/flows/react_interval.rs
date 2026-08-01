//! `react_to_flows_interval` — the durable **clock-scan** for the `flipflop` source node, the interval
//! sibling of [`super::react_cron`]. Same altitude/cadence: a stateless function over a durable set,
//! never a long-lived in-process timer (rule 4). A `flipflop` is a self-driving boolean oscillator —
//! no input, one output, flipping `true`/`false` every `period_secs`.
//!
//! One durable record per node ([`super::record::FlowTriggerState`]) holds BOTH the clock cursor
//! (`next_attempt_ts`, advanced by [`next_slot_after`] — the next period-grid slot strictly after
//! `now`, NEVER a blind `+period` which drifts unboundedly behind a slow scan) AND the last emitted
//! value (`flop`) — so value and clock move together and both survive restart. Idempotency: a
//! scheduled instant derives a deterministic run id and is skipped if its job already exists (an
//! at-least-once re-scan never double-flips). Missed-firing policy — fire-once-then-skip-to-next-
//! future-slot (no backfill storm). A firing is SPAWNED (seed-durably-then-drive-detached, the
//! `flows_run_async` seam) so N due nodes fire independently — the scan never blocks on a subgraph.
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
            // Per-node isolation: one broken node/flow must not starve every other trigger this pass
            // (a `?` here aborted the rest of the workspace scan for the tick — interval-source-clock
            // scope, defect 3). Log and keep scanning; the next tick retries the failed one.
            if let Err(e) =
                fire_one_flipflop(node, principal, ws, flow, &trig, now, &mut pass).await
            {
                tracing::warn!(
                    ws = %ws, flow = %flow.id, node = %trig.node_id, error = %e,
                    "flip-flop firing failed; continuing the pass"
                );
            }
        }
    }
    Ok(pass)
}

/// Fire **one named** flip-flop node, if it is still a live trigger on a still-enabled flow. This is
/// the seam [`super::interval_timers`] drives: a per-node timer owns the CADENCE, but the firing
/// itself stays here — one owner for the fire logic, so a timer-driven firing and a sweep-driven one
/// are the same code (same idempotency, same cursor, same caps wall).
///
/// Returns the pass tally. A disabled/deleted flow, or a node that is no longer a flip-flop, is a
/// silent no-op tally — the reconciler will retire the timer on its next pass; racing that teardown
/// must not error.
pub async fn fire_flipflop_node(
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
    let Some(trig) = flipflop_triggers(flow)
        .into_iter()
        .find(|t| t.node_id == node_id)
    else {
        return Ok(pass);
    };
    fire_one_flipflop(node, principal, ws, flow, &trig, now, &mut pass).await?;
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
        let next = next_slot_after(scheduled_ts, trig.period_secs, now);
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
    // from params under the node id, exactly as the cron leg reads `cron_ts`. SPAWNED, not awaited: the
    // run seeds durably (job + run record exist on return, so the idempotency check above holds) and
    // drives on a detached task — ten due flip-flops fire this pass, not one per slow subgraph.
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
    // Advance the clock to the next FUTURE slot AND persist the value just emitted (so the next firing
    // flips it). `next_slot_after`, never `+period`: a scan later than the slot (the 5s sweep, a stall)
    // must not leave the cursor in the past, or it gains one period per scan and drifts without bound.
    let next = next_slot_after(scheduled_ts, trig.period_secs, now);
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

/// The next slot on the period grid anchored at `scheduled_ts` that lies **strictly after** `now` —
/// the interval counterpart of `react_cron`'s `next_after(schedule, now)`. This is what makes
/// fire-once-then-skip true: a scan arriving late (the sweep floor, a stall, an outage) advances the
/// cursor past `now` in ONE step, instead of one period per scan (which left the cursor permanently
/// behind the wall clock, sliding further adrift every tick). Pure + clock-injected — deterministic
/// under test.
fn next_slot_after(scheduled_ts: u64, period_secs: u64, now: u64) -> u64 {
    // A zero period cannot reach here (descriptor `minimum` rejects it at save); guard anyway so a
    // corrupt record can never divide by zero or spin the cursor in place.
    let period = period_secs.max(1);
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
    period_secs: u64,
    next_attempt_ts: u64,
    flop: Option<bool>,
) -> Result<(), FlowsError> {
    let state = FlowTriggerState {
        next_attempt_ts,
        cron: None,
        period_secs: Some(period_secs),
        flop,
        last_seq: None,
        // Schedule-source fields are inert here (this reactor owns a different source kind).
        ..Default::default()
    };
    write_cursor(&node.store, ws, flow_id, node_id, &state)
        .await
        .map_err(FlowsError::Internal)
}

#[cfg(test)]
mod tests {
    use super::next_slot_after;

    /// The on-time scan: firing exactly at the slot advances by exactly one period.
    #[test]
    fn on_time_scan_advances_one_period() {
        assert_eq!(next_slot_after(100, 10, 100), 110);
    }

    /// **The drift regression** (interval-source-clock scope, defect 2): a scan later than the slot
    /// lands the cursor strictly in the future in one step — never `scheduled + period` (which at
    /// `period=1` under a 5s sweep slid 4s further behind every tick, observed ~60s adrift live).
    #[test]
    fn late_scan_skips_to_the_next_future_slot() {
        // Slot was 100, period 1, the sweep arrives at 157: next is 158, NOT 101.
        assert_eq!(next_slot_after(100, 1, 157), 158);
        // Period 10, scan at 157: the grid is 100,110,…,150,160 → 160.
        assert_eq!(next_slot_after(100, 10, 157), 160);
        // Exactly on a later grid slot: strictly after, so the NEXT one.
        assert_eq!(next_slot_after(100, 10, 160), 170);
    }

    /// A not-yet-due slot is returned unchanged (the caller's `scheduled_ts > now` early-return
    /// normally prevents this path; the function stays total anyway).
    #[test]
    fn future_slot_is_kept() {
        assert_eq!(next_slot_after(200, 10, 150), 200);
    }

    /// A corrupt zero period neither divides by zero nor pins the cursor in the past.
    #[test]
    fn zero_period_still_advances() {
        assert_eq!(next_slot_after(100, 0, 100), 101);
    }
}
