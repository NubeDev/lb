//! `spawn_agent_reactors` — the background driver for **detached channel agent runs** (run-lifecycle
//! #5), the twin of [`spawn_flow_reactors`](crate::spawn_flow_reactors). The channel agent worker
//! used to drive the run INLINE in `channel::post`, so it was tied to the held POST connection —
//! closing the tab mid-run cancelled it, and a node restart lost it. Now the worker only **enqueues**
//! a durable job (`channel-agent-run`); this reactor is the one owner that drains the queue and drives
//! each run off the request connection, so a run survives the tab closing and (because the enqueue is
//! durable + idempotent) a node restart.
//!
//! Shape mirrors the flow reactor exactly: one detached task per node, ticking a ws-scoped scan on a
//! cadence. Each tick lists the still-`Running` enqueue jobs ([`lb_jobs::pending`]), and for each one
//! it isn't already driving, **spawns** [`channel::drive_queued_run`](crate::channel) so a long run
//! never stalls the tick or the rest of the queue (unbounded per-ws concurrency, per run-lifecycle
//! #5's "unbounded per workspace" decision; the only cap is host self-protection, deferred).
//!
//! **No double-drive, two guards:**
//!   - *in-process:* an `in_flight` set of run ids currently being driven — a tick skips a job whose
//!     run it already spawned (the job stays `Running` until the drive marks it `Done`).
//!   - *durable (survives a restart / crash mid-drive):* `drive_queued_run` itself short-circuits if
//!     the correlated answer item (`a:<run_job>`) already exists — so a job that completed but wasn't
//!     retired (node died between the post and the `complete`) is never re-run, never re-spent.
//!
//! Errors are logged, never fatal — a single bad run must not stop the node's heartbeat; the next tick
//! re-scans. This is the thin role-aware wiring §3.1 permits (beside the engine, not inside a core
//! crate's logic), symmetric across edge/cloud (rule 1) — placement is config, never a code branch.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::boot::Node;
use crate::channel::{drive_queued_run, ChannelAgentJob, CHANNEL_AGENT_KIND};

/// Spawn the detached drain tick for the given workspaces. Returns immediately; the loop runs for the
/// life of the node. `period` is the scan cadence — a few seconds keeps a freshly-posted `agent`
/// request from waiting long before its run starts, and each tick is a cheap ws-scoped job scan.
pub fn spawn_agent_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    // Run ids currently being driven — the in-process no-double-drive guard, shared across ticks.
    let in_flight: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for ws in &workspaces {
                drain_spawning(&node, ws, &in_flight).await;
            }
        }
    });
}

/// One production drain pass for one workspace: list pending channel-agent-run jobs and **spawn** a
/// drive for each that isn't already in flight. Non-blocking — the drives run on detached tasks so a
/// long run never stalls the tick or the rest of the queue.
async fn drain_spawning(node: &Arc<Node>, ws: &str, in_flight: &Arc<Mutex<HashSet<String>>>) {
    let records = scan_drivable(node, ws).await;
    for (enqueue_id, record) in records {
        // IN-PROCESS guard: claim the run id; if already claimed this pass/an earlier one, skip it (its
        // drive is still running and the enqueue job legitimately stays `Running` until it retires).
        {
            let mut set = in_flight.lock().expect("in_flight lock");
            if !set.insert(record.run_job.clone()) {
                continue;
            }
        }
        let node = node.clone();
        let in_flight = in_flight.clone();
        let ws = ws.to_string();
        tokio::spawn(async move {
            drive_queued_run(&node, &ws, &enqueue_id, &record).await;
            // Release the claim once the drive has retired the job (or short-circuited on idempotency).
            in_flight
                .lock()
                .expect("in_flight lock")
                .remove(&record.run_job);
        });
    }
}

/// Drive every pending channel-agent-run in `ws` to completion **inline and sequentially**, returning
/// only when they have all posted their result/error item and retired their enqueue job. This is the
/// deterministic drain a test drives without the timer (and a caller that wants a synchronous flush) —
/// the same drive as the production tick, minus the spawn/in-flight bookkeeping the timer needs.
pub async fn drain_channel_agent_runs(node: &Arc<Node>, ws: &str) {
    for (enqueue_id, record) in scan_drivable(node, ws).await {
        drive_queued_run(node, ws, &enqueue_id, &record).await;
    }
}

/// List the drivable pending enqueue jobs in `ws` as `(enqueue_id, record)` pairs. A malformed record
/// can never be driven, so it is retired (`Failed`) here rather than re-scanned every tick — one owner
/// of "decode the queue", shared by the spawning tick and the blocking drain.
async fn scan_drivable(node: &Arc<Node>, ws: &str) -> Vec<(String, ChannelAgentJob)> {
    let pending = match lb_jobs::pending(&node.store, ws, CHANNEL_AGENT_KIND).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(ws = %ws, error = %e, "channel agent drain scan failed");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for job in pending {
        match serde_json::from_str::<ChannelAgentJob>(&job.payload) {
            Ok(record) => out.push((job.id, record)),
            Err(e) => {
                tracing::warn!(ws = %ws, job = %job.id, error = %e, "bad channel agent enqueue record; retiring");
                let _ =
                    lb_jobs::complete(&node.store, ws, &job.id, lb_jobs::JobStatus::Failed).await;
            }
        }
    }
    out
}
