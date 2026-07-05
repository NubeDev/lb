//! `spawn_flow_reactors` — the production **clock tick** that drives the otherwise-dormant flow
//! reactors. `react_to_flows_cron` and `reconcile_flows` are stateless scans over the durable flow
//! set (rule 4 — no long-lived in-process timer owns state); but a scan only matters if *something*
//! calls it on a cadence. Nothing did: both were wired only into tests, so a `mode:"cron"` trigger
//! never fired on a running node. This is that missing driver — one detached task that ticks the
//! scans every `period`, exactly as a PLC scans its rungs.
//!
//! It is the thin role-aware wiring §3.1 permits (it lives beside the engine, not inside a core
//! crate's logic): a single owner per node re-reads the durable set each tick and converges it. The
//! clock is a **live** wall-clock read per tick (the reactors are deterministic under an injected
//! clock in tests; here, in production, real time is what advances `next_attempt_ts`). On restart the
//! scan resumes from durable `next_attempt_ts` — no firing is lost, none is backfilled (fire-once-
//! then-skip, the reactor's own policy).

use std::sync::Arc;
use std::time::Duration;

use lb_auth::Principal;

use crate::boot::Node;
use crate::Role;

use super::react_cron::react_to_flows_cron;
use super::react_interval::react_to_flows_interval;
use super::reconcile::reconcile_flows;
use super::retention_sweep::{should_sweep, sweep_retention};

/// The caps the reactor's system principal needs to drive a flow run headless: the flows run surface
/// + the store read/write the run-store + reconciler touch. Scoped per workspace (minted fresh for
/// each ws each tick — the principal carries the ws, the hard wall). This is a NODE-internal actor
/// (the reactor IS the node acting on its own durable flows), not a user; it is the same authority
/// the cron/boot reactors always assumed they ran under.
fn reactor_caps() -> Vec<String> {
    vec![
        "mcp:flows.run:call".into(),
        "mcp:flows.enable:call".into(),
        "mcp:flows.inject:call".into(),
        // Resume/cancel a run parked on an approval gate (the flow-approval reactor, slice 4).
        "mcp:flows.resume:call".into(),
        "mcp:flows.cancel:call".into(),
        // Read a webhook source's series to fire a run per new hit (the source reactor, slice 5).
        "mcp:series.read:call".into(),
        "store:flow:read".into(),
        "store:flow:write".into(),
        "store:*:read".into(),
        "store:*:write".into(),
        "mcp:*.call:call".into(),
    ]
}

/// Spawn the detached reactor tick for the given workspaces. Returns immediately; the loop runs for
/// the life of the node. `period` is the scan cadence — a cron with minute granularity wants a
/// sub-minute tick so a due instant is caught promptly (a few seconds is plenty and cheap: each tick
/// is a ws-scoped store scan).
pub fn spawn_flow_reactors(node: Arc<Node>, workspaces: Vec<String>, role: Role, period: Duration) {
    tokio::spawn(async move {
        // First tick after one period (boot bring-up already armed start_on_boot flows elsewhere).
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Ticks since boot — gates the throttled retention sweep (fires on tick 0, then every N).
        let mut tick_count: u64 = 0;
        loop {
            ticker.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let sweep = should_sweep(tick_count);
            for ws in &workspaces {
                let principal = Principal::routed("node:reactor", ws.clone(), reactor_caps());
                tick_once(&node, &principal, ws, role, now).await;
                // Bounded retention for the tables that grow from routine reactor traffic — trimmed on
                // the same ws-scoped tick as the drain, throttled to every Nth tick (see
                // `retention_sweep`). Keeps `job`/`flow_run`/`flow_step_output` finite so even a naïve
                // scan stays cheap and the store stops bloating on disk.
                if sweep {
                    sweep_retention(&node, ws).await;
                }
            }
            tick_count = tick_count.wrapping_add(1);
        }
    });
}

/// One reactor pass for one workspace: reconcile sources/boot, then fire due cron. Errors are logged,
/// never fatal — a single bad flow must not stop the node's heartbeat (the next tick retries).
async fn tick_once(node: &Arc<Node>, principal: &Principal, ws: &str, role: Role, now: u64) {
    if let Err(e) = reconcile_flows(node, principal, ws, role, now).await {
        tracing::warn!(ws = %ws, error = %e, "flow reconcile pass failed");
    }
    match react_to_flows_cron(node, principal, ws, now).await {
        Ok(pass) if pass.fired > 0 => {
            tracing::info!(ws = %ws, fired = pass.fired, "flow cron reactor fired");
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow cron reactor pass failed"),
    }
    match react_to_flows_interval(node, principal, ws, now).await {
        Ok(pass) if pass.fired > 0 => {
            tracing::info!(ws = %ws, fired = pass.fired, "flow flip-flop reactor fired");
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow flip-flop reactor pass failed"),
    }
    // Fire a run per new webhook hit on each `webhook` source node's series (slice 5).
    match super::react_source::react_to_flow_sources(node, principal, ws, now).await {
        Ok(pass) if pass.fired > 0 => {
            tracing::info!(ws = %ws, fired = pass.fired, "flow webhook source reactor fired");
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow webhook source reactor pass failed"),
    }
    // Resume/cancel runs parked on an approval gate whose inbox item has resolved (slice 4).
    match super::react_approval::react_to_flow_approvals(node, principal, ws, now).await {
        Ok(pass) if pass.resumed > 0 || pass.cancelled > 0 => {
            tracing::info!(
                ws = %ws,
                resumed = pass.resumed,
                cancelled = pass.cancelled,
                "flow approval reactor resumed/cancelled parked runs"
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow approval reactor pass failed"),
    }
}
