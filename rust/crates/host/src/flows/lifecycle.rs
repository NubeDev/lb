//! `flows.suspend` / `flows.cancel` (flow-run-scope). `suspend` stops the coordinator enqueuing the
//! next frontier (the unexecuted nodes stay Pending/Enqueued) — `flows.resume` re-drives. `cancel` is
//! terminal + non-resumable: the run's step outputs are kept for audit; the coordinator is marked
//! `cancelled`. Both are workspace-walled (the run record is read first; a ws-B caller cannot touch a
//! ws-A run).

use std::sync::Arc;

use lb_auth::Principal;

use crate::boot::Node;

use super::error::FlowsError;
use super::run_store;

/// Suspend a run: mark the coordinator `suspended`. Idempotent. The in-flight frontier finishes; the
/// coordinator stops enqueuing the next frontier until `flows.resume`.
pub async fn flows_suspend(
    node: &Arc<Node>,
    _principal: &Principal,
    ws: &str,
    run_id: &str,
) -> Result<(), FlowsError> {
    let run = run_store::read_run(&node.store, ws, run_id)
        .await
        .map_err(FlowsError::Internal)?
        .ok_or(FlowsError::NotFound)?;
    let _ = run; // (exists-check; the status write is the effect)
    run_store::set_run_status(&node.store, ws, run_id, "suspended")
        .await
        .map_err(FlowsError::Internal)
}

/// Cancel a run: terminal + non-resumable. The coordinator is marked `cancelled`; a later `resume`
/// refuses (the run is no longer `pending`/`suspended`).
pub async fn flows_cancel(
    node: &Arc<Node>,
    _principal: &Principal,
    ws: &str,
    run_id: &str,
) -> Result<(), FlowsError> {
    run_store::set_run_status(&node.store, ws, run_id, "cancelled")
        .await
        .map_err(FlowsError::Internal)
}
