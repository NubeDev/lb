//! The boot **reactor** spawns (moved verbatim from `main.rs`): the flow / channel-agent / approval /
//! insight-digest background scan loops, plus the one-shot insight-timestamp heal. Gated by
//! [`BootConfig::reactors`] — the `node` binary spawns them (today's behaviour); an embedder wanting
//! store+auth+MCP only sets `reactors: false` and no background scans run.

use std::sync::Arc;
use std::time::Duration;

use lb_host::Node;

use crate::config::OutboxProviders;

/// Spawn the background reactor loops for `ws` on `node`, and run the one-shot insight-ts heal. One
/// detached owner per reactor per node, each scanning the configured workspace on its own cadence.
/// `providers` is the boot provider-injection seam (release scope, gap 1): the relay reactor
/// delivers email/push effects through them; unset providers fall back to the logging no-ops.
pub async fn spawn(node: &Arc<Node>, ws: &str, providers: &OutboxProviders) {
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

    // OUTBOX RELAY REACTOR TICK (release scope, gap 1 — previously never booted): drain staged
    // outbox effects through the registered delivery adapters. The RouterTarget dispatches on the
    // effect's opaque `target` string (rule 10): `email` → EmailTarget, `push` → PushTarget. A
    // provider the embedder didn't configure falls back to the logging no-op — the relay still
    // drains (never crash boot, never strand effects); the send is logged, not performed.
    let email_provider: Box<dyn lb_host::EmailProvider> = match &providers.email {
        Some(p) => Box::new(p.clone()),
        None => Box::new(lb_host::LoggingEmailProvider),
    };
    let push_provider: Box<dyn lb_host::PushProvider> = match &providers.push {
        Some(p) => Box::new(p.clone()),
        None => Box::new(lb_host::LoggingPushProvider),
    };
    let router = lb_host::RouterTarget::new()
        .route(
            lb_host::EMAIL_TARGET,
            lb_host::EmailTarget::new(email_provider),
        )
        .route(
            lb_host::PUSH_TARGET,
            lb_host::PushTarget::new(push_provider, node.store.clone()),
        );
    lb_host::spawn_relay_reactors(
        node.clone(),
        vec![ws.to_string()],
        router,
        Duration::from_secs(2),
    );

    // INGEST DRAIN REACTOR TICK (drain-backpressure scope — previously never booted): commit staged
    // samples → the `series` tables off every caller's request path. The ingest scope always named a
    // "commit worker mounted by the ingest role" and `drain.rs` said outright there was no
    // background drain worker — so every CALLER was the worker, draining the whole workspace backlog
    // inside its own call (one sample against a 4,671-row backlog measured 18.5s vs 21ms at backlog
    // 0, and it never recovered: a caller that timed out abandoned only the wait). Callers now drain
    // only their own batch; this tick owns the backlog. A few seconds is ample — a writer's own
    // samples already commit inline, so nothing here is latency-critical.
    lb_host::spawn_ingest_reactors(node.clone(), vec![ws.to_string()], Duration::from_secs(2));

    // INSIGHT DIGEST REACTOR TICK: digest the anti-spam ladder — one message per (sub, window), decay
    // quiet keys, post under each sub's stored principal. 30s cadence (windows are hours/days).
    lb_host::spawn_insight_digest_reactors(
        node.clone(),
        vec![ws.to_string()],
        Duration::from_secs(30),
    );
}
