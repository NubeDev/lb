//! `spawn_relay_reactors` — the background driver that ticks [`relay_outbox`](super::relay_outbox) on a
//! cadence to deliver a workspace's staged outbox effects (rules-workflow-convergence scope, slice 4).
//! It is the twin of [`spawn_approval_reactors`](crate::spawn_approval_reactors) /
//! [`spawn_flow_reactors`](crate::spawn_flow_reactors), and the generic replacement for the retired
//! github-workflow driver loop — which was the ONLY thing that drove `relay_outbox` in-process.
//!
//! Without this tick the relay is dormant: the `sink` flow node (`target=outbox`) stages a pending
//! effect, but nothing delivers it. One detached owner per node re-reads the durable `due` set each
//! tick and delivers through the supplied [`Target`] with retry/backoff/dead-letter. The `Target` is
//! **provider-free** (rule 10) — a real delivery adapter is an extension that implements it, supplied
//! by the binary at boot (as github-workflow used to supply its own); core names no provider. Errors
//! are logged, never fatal (a single bad ws must not stop the node's heartbeat).
//!
//! Symmetric across edge/cloud (rule 1) — which node runs it is config (the binary chooses to spawn it
//! with a `Target`), never a code branch.

use std::sync::Arc;
use std::time::Duration;

use crate::boot::Node;

use super::relay::relay_outbox;
use super::target::Target;

/// Spawn the detached relay tick for `workspaces`, delivering each ws's due effects through `target`.
/// Returns immediately; the loop runs for the life of the node. `now` is a live wall-clock read per
/// tick (the binary is the clock boundary — the relay is deterministic under an injected clock in
/// tests). `period` is the delivery cadence — a few seconds keeps a just-staged effect from waiting
/// long, and each tick is a cheap ws-scoped `due` scan.
pub fn spawn_relay_reactors<T>(
    node: Arc<Node>,
    workspaces: Vec<String>,
    target: T,
    period: Duration,
) where
    T: Target + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            for ws in &workspaces {
                match relay_outbox(&node.store, ws, &target, now).await {
                    Ok(pass) if pass.delivered > 0 || pass.dead_lettered > 0 => {
                        tracing::info!(
                            ws = %ws,
                            delivered = pass.delivered,
                            failed = pass.failed,
                            dead_lettered = pass.dead_lettered,
                            "outbox relay delivered effects"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(ws = %ws, error = %e, "outbox relay pass failed"),
                }
            }
        }
    });
}
