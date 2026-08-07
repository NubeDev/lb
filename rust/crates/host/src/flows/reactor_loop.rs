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

use super::interval_timers::{reconcile_interval_timers, IntervalTimers};
use super::react_cron::react_to_flows_cron;
use super::reconcile::reconcile_flows;
use super::retention_sweep::{should_sweep, sweep_retention};

/// The caps the reactor's system principal needs to drive a flow run headless: the flows run surface
/// + the store read/write the run-store + reconciler touch. Scoped per workspace (minted fresh for
///   each ws each tick — the principal carries the ws, the hard wall). This is a NODE-internal actor
///   (the reactor IS the node acting on its own durable flows), not a user; it is the same authority
///   the cron/boot reactors always assumed they ran under.
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
        // ext-store-nodes scope: the built-in platform nodes a HEADLESS (cron / flip-flop / webhook)
        // flow drives dispatch these MCP verbs under this reactor principal — the scope's own nightly-
        // cron example is exactly such a flow (ext-list branch + store-write heartbeat). Without them a
        // scheduled flow's `ext-list`/`store-*` node is `denied` while a MANUAL run (the user's token)
        // succeeds — the asymmetry that reads as `partialFailure`. Each is already backstopped: the
        // store MUTATE verbs re-check the reserved-table wall (a reactor cannot brick `flow`/`install`/
        // `dashboard` any more than a user can), and `store:*:read/write` above is the surface cap they
        // pair with. `ext.list` is the READ inventory verb (host-native by exact name, rule 10).
        //
        // NOT added: `ext-call` to an arbitrary `<ext>.<tool>` (that would need `mcp:*.*:call` — a
        // near-omnipotent MCP grant for a system actor). A scheduled flow that must call a specific
        // extension tool is the case for run-as-owner (mint the flow author's caps), tracked as a
        // follow-up; a fixed system principal deliberately does not carry blanket third-party reach.
        "mcp:ext.list:call".into(),
        "mcp:store.query:call".into(),
        "mcp:store.write:call".into(),
        "mcp:store.delete:call".into(),
    ]
}

/// Spawn the detached reactor tick for the given workspaces. Returns immediately; the loop runs for
/// the life of the node. `period` is the scan cadence — a cron with minute granularity wants a
/// sub-minute tick so a due instant is caught promptly (a few seconds is plenty and cheap: each tick
/// is a ws-scoped store scan).
pub fn spawn_flow_reactors(node: Arc<Node>, workspaces: Vec<String>, role: Role, period: Duration) {
    // The node's live interval timers (interval-source-clock scope, Phase 2). Owned by this loop and
    // reconciled on every tick — the tick is the CONVERGENCE cadence (how fast an enable/disable takes
    // effect), no longer the FIRING cadence (which each timer now owns exactly). This is what makes
    // `period_secs: 1` mean one second: the sweep interval is no longer a floor on any interval node.
    let timers = Arc::new(IntervalTimers::new());
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
                tick_once(&timers, &node, &principal, ws, role, now).await;
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
async fn tick_once(
    timers: &Arc<IntervalTimers>,
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    role: Role,
    now: u64,
) {
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
    // INTERVAL SOURCES: converge the per-node timers, do NOT fire here. A timer fires the same
    // deterministic run id this sweep would, so running both would race the idempotency read — the
    // timers own every flip-flop exclusively (interval-source-clock scope, Phase 2). The tick is now
    // only how fast an enable/disable/period-edit takes effect, never how often a node can fire.
    match reconcile_interval_timers(timers, node, principal, ws).await {
        Ok(pass) if pass.started > 0 || pass.stopped > 0 => {
            tracing::info!(
                ws = %ws, started = pass.started, stopped = pass.stopped,
                "flow interval timers reconciled"
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow interval timer reconcile failed"),
    }
    // Fire a run per new webhook hit on each `webhook` source node's series (slice 5).
    match super::react_source::react_to_flow_sources(node, principal, ws, now).await {
        Ok(pass) if pass.fired > 0 => {
            tracing::info!(ws = %ws, fired = pass.fired, "flow webhook source reactor fired");
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow webhook source reactor pass failed"),
    }
    // SCHEDULE SOURCES: evaluate each `schedule` node's referenced global schedule and fire on a
    // transition. This one stays on the sweep (unlike the interval timers): schedule windows are
    // minute-grained at finest, so the tick cadence is ample resolution, and a sweep-driven scan needs
    // no per-node timer to converge when a shared schedule record is edited out from under N nodes.
    match super::react_schedule::react_to_flows_schedule(node, principal, ws, now).await {
        Ok(pass) if pass.fired > 0 => {
            tracing::info!(ws = %ws, fired = pass.fired, "flow schedule reactor fired");
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow schedule reactor pass failed"),
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

#[cfg(test)]
mod tests {
    use super::reactor_caps;
    use lb_caps::{matches, Action, Request, Surface};

    /// Does the reactor's system principal authorize `cap` (a `surface:resource:action` grant) through
    /// the real matcher — i.e. would a headless flow node dispatching it pass Gate 2?
    fn reactor_authorizes(cap: &str) -> bool {
        let caps = reactor_caps();
        let mut parts = cap.splitn(3, ':');
        let surface = Surface::parse(parts.next().unwrap()).unwrap();
        let resource = parts.next().unwrap();
        let action = Action::parse(parts.next().unwrap()).unwrap();
        matches(&caps, &Request::new("nube", surface, resource, action))
    }

    /// **The headless-flow node reach** (ext-store-nodes scope). A cron / flip-flop / webhook flow runs
    /// under the reactor's system principal, so that principal must authorize every MCP verb the
    /// scope's BUILT-IN platform nodes dispatch — or a scheduled flow's `ext-list`/`store-*` node is
    /// `denied` while the author's MANUAL run succeeds (the `partialFailure` asymmetry this pins). It
    /// must NOT authorize an arbitrary third-party `ext-call` verb: that is the deliberate boundary
    /// (blanket `mcp:*.*:call` for a system actor is the escalation we refuse — run-as-owner is the
    /// path for a scheduled ext-call).
    #[test]
    fn reactor_drives_the_builtin_platform_nodes_but_not_arbitrary_ext_call() {
        for needed in [
            "mcp:ext.list:call",     // ext-list node
            "mcp:store.query:call",  // store-read node
            "mcp:store.write:call",  // store-write node
            "mcp:store.delete:call", // store-delete node
            "mcp:flows.run:call",    // the run surface itself (already held pre-fix)
        ] {
            assert!(
                reactor_authorizes(needed),
                "the reactor must authorize {needed} so a headless flow's built-in node is not denied"
            );
        }
        // The boundary: a headless flow cannot reach an arbitrary extension's own tool through the
        // reactor. `mcp:*.call:call` covers only `<x>.call` verbs (e.g. `native.call`), never a
        // third-party `<ext>.<verb>` like `modbus.point.read`.
        for denied in ["mcp:modbus.point.read:call", "mcp:ext.uninstall:call"] {
            assert!(
                !reactor_authorizes(denied),
                "the reactor must NOT authorize {denied} — arbitrary ext-call / lifecycle is off-limits \
                 to the system principal (run-as-owner is the path for a scheduled ext-call)"
            );
        }
    }
}
