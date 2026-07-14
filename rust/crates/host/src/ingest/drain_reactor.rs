//! `spawn_ingest_reactors` — the background driver that ticks [`drain_workspace`](super::drain_workspace)
//! on a cadence to commit a workspace's staged samples (drain-backpressure scope). It is the twin of
//! [`spawn_relay_reactors`](crate::spawn_relay_reactors): the outbox stages must-deliver effects OUT
//! and a reactor delivers them; ingest stages high-volume samples IN and this reactor commits them.
//!
//! **This is the driver the ingest scope always named and never got.** `ingest-scope.md` describes a
//! "commit worker mounted by the ingest role"; `ingest/mod.rs` calls `drain_workspace` "the commit
//! worker" — but nothing ever ticked it, and `drain.rs` said so outright ("There is no background
//! drain worker"). So every CALLER became the worker, synchronously and unbounded: `ingest.write`
//! drained until staging was empty and was billed for every other producer's rows (one sample
//! against a 4,671-row backlog measured 18.5s vs 21ms at backlog 0). Bounding the caller's drain is
//! only half a fix — without a driver the remainder would strand. This is the other half.
//!
//! One detached owner per node re-drains each ws's staging every tick, unbounded (the reactor is
//! exactly where an O(backlog) drain belongs — it is nobody's request). Errors are logged, never
//! fatal: a single bad ws must not stop the node's heartbeat.
//!
//! Symmetric across edge/cloud (rule 1) — which node runs it is config (the binary chooses to spawn
//! it, gated by `BootConfig::reactors` like its siblings), never a code branch.

use std::sync::Arc;
use std::time::Duration;

use crate::boot::Node;

use super::drain::drain_workspace;

/// Spawn the detached ingest-drain tick for `workspaces`. Returns immediately; the loop runs for the
/// life of the node.
///
/// `period` is the commit cadence. A few seconds is right: a caller's own samples already commit
/// inline (the bounded drain preserves the write-then-read round-trip), so this tick is only
/// responsible for the BACKLOG — nothing here is latency-critical. Ticks never overlap
/// (`MissedTickBehavior::Skip`), so a long drain over a deep backlog cannot pile passes on top of
/// each other.
pub fn spawn_ingest_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for ws in &workspaces {
                match drain_workspace(&node.store, ws).await {
                    // Only log a pass that did something — an idle node ticks quietly.
                    Ok(pass) if pass.committed > 0 => tracing::info!(
                        ws = %ws,
                        committed = pass.committed,
                        "ingest drain committed staged samples"
                    ),
                    Ok(_) => {}
                    Err(e) => tracing::warn!(ws = %ws, error = %e, "ingest drain pass failed"),
                }
            }
        }
    });
}
