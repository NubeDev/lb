//! `spawn_retention_reactors` — the background driver that ticks [`run_gc`](lb_ingest::run_gc) so
//! series retention actually EXECUTES (series-sample-cap scope, issue #65).
//!
//! **This is the driver retention shipped without.** `run_gc` was reachable only from tests and the
//! on-demand `series.retention.gc` verb — so a correctly-configured time horizon evicted *nothing*
//! on a real node unless an operator called the verb by hand. The mechanism shipped; its heartbeat
//! didn't. That is the same missing-driver class as the ingest drain bug that preceded it
//! ([`spawn_ingest_reactors`](super::spawn_ingest_reactors) — `drain_workspace` had no reactor
//! either), which is why this slice treats "it runs" as part of the feature rather than a follow-up.
//! Without this loop, `max_samples` would be decorative.
//!
//! One detached owner per node ticks each ws's GC on a SLOW cadence. Retention is not
//! latency-critical: nothing waits on an eviction, and the pass is deliberately expensive (a
//! `count()` per series, serialized behind the store's global session mutex — up to 10k series per
//! workspace). Minutes, not seconds. `debugging/agent/dev-node-cpu-job-scan.md` is the precedent for
//! why a fast tick over a full table scan is a CPU bug waiting to happen. Ticks never overlap
//! (`MissedTickBehavior::Skip`), so a long pass over a deep backlog cannot pile up.
//!
//! Errors are logged, never fatal: one bad workspace must not stop the node's heartbeat.
//!
//! The reactor mints no principal and needs no capability — it executes durable policy an authorized
//! admin already wrote through the capability-gated `series.retention.set`, exactly as the drain and
//! relay reactors execute their own durable state. Symmetric across edge/cloud (rule 1): which node
//! runs it is config (`BootConfig::reactors`), never a code branch.

use std::sync::Arc;
use std::time::Duration;

use lb_ingest::run_gc;

use crate::boot::Node;

/// The retention cadence: slow, because a pass is expensive and nothing waits on it.
pub const RETENTION_PERIOD: Duration = Duration::from_secs(300);

/// Spawn the detached retention-GC tick for `workspaces`. Returns immediately; the loop runs for the
/// life of the node.
///
/// `period` is the GC cadence — minutes (see [`RETENTION_PERIOD`]). `now_ms` is stamped wall-clock
/// per tick, the same source the `series.retention.gc` verb uses when a caller omits it.
pub fn spawn_retention_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for ws in &workspaces {
                match run_gc(&node.store, ws, now_wall_ms()).await {
                    Ok(pass) => {
                        // Only log a pass that did something — an idle node ticks quietly.
                        if pass.evicted_raw > 0 || pass.capped_raw > 0 || pass.evicted_rollup > 0 {
                            tracing::info!(
                                ws = %ws,
                                evicted_raw = pass.evicted_raw,
                                capped_raw = pass.capped_raw,
                                rollup_rows = pass.rollup_rows,
                                evicted_rollup = pass.evicted_rollup,
                                "retention gc pass"
                            );
                        }
                        // The advisory over-cap warnings: an unbounded series past the recommended
                        // cap. This is release 1's job on the default axis — make the need for a
                        // policy VISIBLE before a future release starts evicting by default.
                        for warning in &pass.warnings {
                            tracing::warn!(ws = %ws, "{warning}");
                        }
                    }
                    Err(e) => tracing::warn!(ws = %ws, error = %e, "retention gc pass failed"),
                }
            }
        }
    });
}

/// Wall-clock now in epoch ms — the reactor's own clock (the GC takes `now_ms` injected so tests can
/// stamp a constant; determinism §3).
fn now_wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
