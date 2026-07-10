//! The boot **reactor** spawns (moved verbatim from `main.rs`): the flow / channel-agent / approval /
//! insight-digest background scan loops, plus the one-shot insight-timestamp heal. Gated by
//! [`BootConfig::reactors`] — the `node` binary spawns them (today's behaviour); an embedder wanting
//! store+auth+MCP only sets `reactors: false` and no background scans run.

use std::sync::Arc;
use std::time::Duration;

use lb_host::Node;

/// Spawn the background reactor loops for `ws` on `node`, and run the one-shot insight-ts heal. One
/// detached owner per reactor per node, each scanning the configured workspace on its own cadence.
pub async fn spawn(node: &Arc<Node>, ws: &str) {
    // FLOW REACTOR TICK: drive cron/reconcile scans so a `mode:"cron"` trigger actually fires. A
    // few-second period catches a minute-granularity cron promptly; each tick is a cheap ws scan.
    lb_host::spawn_flow_reactors(
        node.clone(),
        vec![ws.to_string()],
        lb_host::Role::Solo,
        Duration::from_secs(5),
    );

    // CHANNEL AGENT REACTOR TICK: drain durable `channel-agent-run` enqueue jobs and drive each run off
    // the reactor, so an in-channel agent run survives the tab closing and (durable + idempotent) a
    // node restart. One detached owner per node on a few-second cadence.
    lb_host::spawn_agent_reactors(node.clone(), vec![ws.to_string()], Duration::from_secs(2));

    // APPROVAL-RELEASE REACTOR TICK: release a rule's `held` gated effect the moment its
    // `needs:approval` item is approved (or discard on reject). Cheap ws-scoped scan; guarded transition.
    lb_host::spawn_approval_reactors(node.clone(), vec![ws.to_string()], Duration::from_secs(2));

    // INSIGHT TS HEAL (one-shot, idempotent): rewrite historical insights whose `ts` landed in the
    // seconds-band `[1e9, 1e12)` ×1000. A no-op once healed, so safe every boot.
    let _ = lb_host::heal_insight_timestamps(&node.store, ws).await;

    // INSIGHT DIGEST REACTOR TICK: digest the anti-spam ladder — one message per (sub, window), decay
    // quiet keys, post under each sub's stored principal. 30s cadence (windows are hours/days).
    lb_host::spawn_insight_digest_reactors(
        node.clone(),
        vec![ws.to_string()],
        Duration::from_secs(30),
    );
}
