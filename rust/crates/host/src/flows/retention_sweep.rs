//! The reactor-tick **retention sweep** — the one place that trims the three tables that grow
//! unbounded from routine reactor traffic (`job`, `flow_run`, `flow_step_output`). It runs on the
//! flow reactor's cadence but **throttled**: a DELETE pass every few seconds per workspace is wasteful
//! when the tables move slowly, so the sweep fires only once every [`SWEEP_EVERY_N_TICKS`] ticks. The
//! bound is soft (a retention bound, not a correctness ring cap), so the overshoot between sweeps is
//! acceptable — the invariant that matters (a live job/run is never trimmed) lives in the trim verbs
//! themselves (`lb_jobs::retain_terminal`, [`retain_runs`](super::retain_runs::retain_runs)), not here.
//!
//! Errors are logged, never fatal — a failed trim must not stop the node's heartbeat; the next
//! eligible tick retries. Workspace-scoped (the caller ticks per ws), so the hard wall holds: a ws-B
//! sweep trims only ws-B's rows.

use std::sync::Arc;

use crate::boot::Node;

use super::retain_runs::{retain_runs, DEFAULT_FINISHED_RUN_CAP};

/// Run the retention sweep once every N reactor ticks. At the flow reactor's 5s cadence this is a trim
/// roughly every ~2.5 minutes — often enough to keep the tables bounded, rare enough that the DELETE
/// cost is negligible against the tick budget. A compiled default (per `capped.rs`, "defaults live in
/// the caller"); an operator knob would resolve a value here, not change the primitive.
pub const SWEEP_EVERY_N_TICKS: u64 = 30;

/// Trim the terminal `job` rows and the finished `flow_run` / `flow_step_output` rows in `ws` to their
/// per-workspace caps. Called from the reactor tick when the tick counter lands on the sweep cadence
/// (see [`should_sweep`]). Each trim is independent — a failure in one is logged and does not block
/// the other, and never fails the tick.
pub async fn sweep_retention(node: &Arc<Node>, ws: &str) {
    match lb_jobs::retain_terminal(&node.store, ws, lb_jobs::DEFAULT_TERMINAL_JOB_CAP).await {
        Ok(0) => {}
        Ok(n) => tracing::debug!(ws = %ws, deleted = n, "job retention trimmed terminal jobs"),
        Err(e) => tracing::warn!(ws = %ws, error = %e, "job retention sweep failed"),
    }
    match retain_runs(&node.store, ws, DEFAULT_FINISHED_RUN_CAP).await {
        Ok(0) => {}
        Ok(n) => tracing::debug!(ws = %ws, deleted = n, "flow-run retention trimmed finished runs"),
        Err(e) => tracing::warn!(ws = %ws, error = %e, "flow-run retention sweep failed"),
    }
}

/// Whether the tick at `tick_count` (0-based, incremented once per reactor tick across all
/// workspaces) should run the retention sweep. Firing on `tick_count % N == 0` including tick 0 means
/// a freshly-booted node reclaims a bloated store on its first tick, then settles into the cadence.
pub fn should_sweep(tick_count: u64) -> bool {
    tick_count % SWEEP_EVERY_N_TICKS == 0
}
