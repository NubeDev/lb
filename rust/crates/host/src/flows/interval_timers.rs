//! `interval_timers` — the **per-node interval timer reconciler** (interval-source-clock scope, Phase
//! 2 / Option A). The ONE owner of live interval cadence on a node: it converges a set of running
//! timer tasks against the durable enabled graph, and nothing else in flows may hold a `tokio` timer
//! for a trigger.
//!
//! ## Why this is not a rule-4 violation
//!
//! "State in the store, motion on the bus, no long-lived in-process timer owns state" is load-bearing.
//! An interval source's **value** (`flop`) and its **next slot** stay exactly where they were: durable
//! in `FlowTriggerState`, in SurrealDB, surviving restart. What moves in here is only the **cadence** —
//! *when to wake up and run the existing idempotent fire path*. That is motion, not state. Today's
//! sweep persists the cadence and polls it, which is the category error this fixes: a period is not an
//! instant, and modelling it as a durable row a coarse sweeper notices costs a full workspace store
//! scan to approximate what `sleep(period)` does for free — **and is still wrong at any period below
//! the sweep** (`period_secs: 1` fired every 5s and drifted without bound).
//!
//! A timer here owns **no state**: kill the process and every timer dies with it; the reconciler
//! rebuilds the set from the durable graph on the next tick, and each rebuilt timer reads its cursor
//! and value back out of the store. The exception is argued in the scope, not smuggled in — so do NOT
//! add a `tokio::interval`/`sleep` for a trigger anywhere outside this module.
//!
//! ## Shape
//!
//! - **Cron keeps the 5s sweep.** Cron has absolute wall-clock instants and minute granularity, so a
//!   durable cursor is exactly right and the sweep is invisible there. Only interval sources move.
//! - **The timer does not re-implement firing.** It sleeps to the durable `next_attempt_ts`, then
//!   calls [`super::react_interval::fire_flipflop_node`] — the same function the sweep called, with
//!   the same deterministic run id, the same `lb_jobs::load` idempotency check, the same caps wall,
//!   and the same `next_slot_after` advance. The timer is a *precise scheduler for an existing
//!   idempotent scan*, which is what makes double-fire impossible by construction rather than by
//!   coordination.
//! - **Exclusive ownership.** Because a timer fires the same deterministic run id the sweep would,
//!   running BOTH would race the idempotency read. So the sweep's interval leg is replaced by
//!   [`reconcile_interval_timers`] — timers own every flip-flop, or nothing does.
//! - **Teardown is structural.** The registry is the sole owner of each `JoinHandle` and
//!   [`LiveTimer`] aborts on `Drop`, so removing a key from the map IS the teardown. A leaked timer
//!   firing a disabled flow is far worse than the bug being fixed, so it must not depend on
//!   remembering to call `abort()`.
//!
//! Workspace-walled: a pass reconciles exactly one ws and never touches another ws's keys, so a ws-B
//! pass can neither start nor stop a ws-A timer.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use lb_auth::Principal;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::boot::Node;

use super::error::FlowsError;
use super::react_interval::fire_flipflop_node;
use super::save::flows_list_internal;
use super::trigger_store::{flipflop_triggers, read_cursor};

/// The identity of one interval timer: a trigger node of a flow in a workspace.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TimerKey {
    pub ws: String,
    pub flow_id: String,
    pub node_id: String,
}

/// A live timer task + the period it was started for. **`Drop` aborts the task** — the registry owns
/// the handle exclusively, so removing the entry tears the timer down with no separate teardown call
/// to forget. A period change is a remove + insert, never an in-place mutate.
struct LiveTimer {
    period_secs: u64,
    task: JoinHandle<()>,
}

impl Drop for LiveTimer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The node's live interval timers, keyed by trigger node. One registry per node, created by
/// [`super::reactor_loop::spawn_flow_reactors`] and reconciled on its tick.
#[derive(Default)]
pub struct IntervalTimers {
    live: Mutex<HashMap<TimerKey, LiveTimer>>,
}

/// What one reconcile pass converged (for logs and for the lifecycle tests).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TimerReconcilePass {
    /// Timers spawned this pass (a newly-enabled flow / a new interval node / a restarted node).
    pub started: usize,
    /// Timers torn down this pass (disabled, deleted, node removed, or a period edit's teardown half).
    pub stopped: usize,
}

impl IntervalTimers {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many timers are live right now (all workspaces), i.e. how many the REGISTRY believes it
    /// owns. Note for anyone writing a teardown test: neither this count nor "did anything fire?" is
    /// evidence that a task actually stopped. The fire path re-reads the flow and skips a disabled
    /// one, so an orphaned timer fires nothing anyway — measured: with `LiveTimer::drop` gutted, both
    /// checks still pass. Assert `num_alive_tasks` (see `flows_interval_timers_test`).
    pub async fn count(&self) -> usize {
        self.live.lock().await.len()
    }

    /// The live keys, for assertions and diagnostics.
    pub async fn keys(&self) -> Vec<TimerKey> {
        self.live.lock().await.keys().cloned().collect()
    }

    /// Stop every timer (node shutdown / test teardown). Clearing the map drops each `LiveTimer`,
    /// which aborts its task.
    pub async fn shutdown(&self) {
        self.live.lock().await.clear();
    }
}

/// Converge the live timer set for `ws` against the durable enabled graph. Idempotent: calling it on
/// an unchanged graph starts and stops nothing, so it is safe (and intended) to run every tick.
///
/// The desired set is every `flipflop` node of every **enabled, non-deleted** flow. A live timer is
/// retired when its key leaves that set (disable / delete / node removed) **or its period changed**
/// (a period edit is teardown + restart, so the new cadence takes effect at once rather than after
/// the old sleep expires). Only keys belonging to `ws` are considered — another workspace's timers
/// are converged by that workspace's own pass.
pub async fn reconcile_interval_timers(
    timers: &Arc<IntervalTimers>,
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
) -> Result<TimerReconcilePass, FlowsError> {
    let mut pass = TimerReconcilePass::default();

    // Desired: every flip-flop node of every enabled flow in this ws.
    let flows = flows_list_internal(&node.store, ws).await?;
    let mut desired: HashMap<TimerKey, u64> = HashMap::new();
    for flow in &flows {
        if !flow.enabled || flow.deleted {
            continue;
        }
        for trig in flipflop_triggers(flow) {
            desired.insert(
                TimerKey {
                    ws: ws.to_string(),
                    flow_id: flow.id.clone(),
                    node_id: trig.node_id,
                },
                trig.period_secs,
            );
        }
    }

    let mut live = timers.live.lock().await;

    // Retire: this ws's live timers that are no longer desired, or whose period changed.
    let stale: Vec<TimerKey> = live
        .iter()
        .filter(|(k, v)| {
            k.ws == ws
                && match desired.get(*k) {
                    None => true,
                    Some(period) => *period != v.period_secs,
                }
        })
        .map(|(k, _)| k.clone())
        .collect();
    for key in stale {
        // `remove` drops the `LiveTimer`, whose `Drop` aborts the task. That is the whole teardown.
        live.remove(&key);
        pass.stopped += 1;
    }

    // Start: desired timers with no live task (newly enabled, newly added, restarted, or the restart
    // half of a period edit).
    for (key, period_secs) in desired {
        if live.contains_key(&key) {
            continue;
        }
        let task = tokio::spawn(timer_loop(
            node.clone(),
            principal.clone(),
            key.clone(),
            period_secs,
        ));
        live.insert(key, LiveTimer { period_secs, task });
        pass.started += 1;
    }

    Ok(pass)
}

/// One node's timer: sleep to its durable next slot, run the shared idempotent fire, repeat.
///
/// The loop deliberately re-reads the cursor every iteration rather than keeping the schedule in
/// memory: the cursor is the authority (a manual fire, a restart, another node in a cluster, or a
/// config edit all move it), and re-reading is one cheap keyed read per period.
async fn timer_loop(node: Arc<Node>, principal: Principal, key: TimerKey, period_secs: u64) {
    let period = period_secs.max(1);
    loop {
        let now = wall_now();
        let cursor = read_cursor(&node.store, &key.ws, &key.flow_id, &key.node_id).await;
        // An UNINITIALISED cursor (absent, or the `0` a save writes to force recompute) means the fire
        // path's init branch will seed it and return WITHOUT firing. That is a legitimate no-progress
        // iteration, and it can happen at most once per timer (init writes a non-zero cursor), so we
        // go straight round rather than backing off — otherwise every enable would cost a dead period.
        let uninit = !matches!(&cursor, Ok(Some(c)) if c.next_attempt_ts > 0);
        let deadline = match &cursor {
            Ok(Some(c)) if c.next_attempt_ts > now => c.next_attempt_ts,
            Ok(_) => now,
            Err(e) => {
                tracing::warn!(
                    ws = %key.ws, flow = %key.flow_id, node = %key.node_id, error = %e,
                    "interval timer could not read its cursor; retrying next period"
                );
                tokio::time::sleep(Duration::from_secs(period)).await;
                continue;
            }
        };
        if deadline > now {
            tokio::time::sleep(Duration::from_secs(deadline - now)).await;
        }

        // Fire through the SHARED path — same deterministic run id, same idempotency read, same caps
        // wall as a sweep-driven firing. A failure is logged and retried next period, never fatal:
        // one broken flow must not stop its own clock forever (and cannot touch any other node's).
        let fired_at = wall_now();
        if let Err(e) = fire_flipflop_node(
            &node,
            &principal,
            &key.ws,
            &key.flow_id,
            &key.node_id,
            fired_at,
        )
        .await
        {
            tracing::warn!(
                ws = %key.ws, flow = %key.flow_id, node = %key.node_id, error = %e,
                "interval timer firing failed; retrying next period"
            );
            tokio::time::sleep(Duration::from_secs(period)).await;
            continue;
        }
        if uninit {
            continue;
        }

        // Forward-progress guard. In steady state the fire advanced the cursor past `deadline`, so the
        // next iteration sleeps a real period. If it did NOT (a shape we don't expect, e.g. the flow
        // was disabled mid-flight and the fire no-oped), sleep a period anyway so this task can never
        // become a hot spin loop against the store.
        let advanced = matches!(
            read_cursor(&node.store, &key.ws, &key.flow_id, &key.node_id).await,
            Ok(Some(c)) if c.next_attempt_ts > deadline
        );
        if !advanced {
            tokio::time::sleep(Duration::from_secs(period)).await;
        }
    }
}

/// Wall-clock seconds. The timer is production motion, so it reads real time — the *arithmetic* it
/// drives (`next_slot_after`) stays pure and clock-injected, which is where determinism belongs.
fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
