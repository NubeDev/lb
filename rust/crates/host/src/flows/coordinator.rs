//! The coordinator — `start` + `drive` (flow-run-scope, ported from the chain `coordinator`). `start`
//! seeds the run + per-node state; `drive` runs the ready frontier (each node: CAS-claim → resolve
//! bindings → execute under `caller ∩ grant` → record outcome → release dependents / fan-in / apply
//! failure policy), looping until the frontier exhausts, then finalises. The durable per-node
//! records + the CAS claim make a restart resume the un-run nodes exactly once (Decision 8).

use std::sync::Arc;

use lb_auth::Principal;
use lb_flows::Flow;
use serde_json::Value;

use crate::boot::Node;

use super::execute_node;
use super::record::ClaimState;
use super::run_store;

/// Seed the run (the coordinator record + per-node claim state). Idempotent on `run_id`.
pub async fn start(
    node: &Arc<Node>,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    params: &serde_json::Map<String, Value>,
    now: u64,
) -> Result<(), String> {
    run_store::create_run(&node.store, ws, run_id, flow, params, now).await
}

/// Drive the run toward completion. Idempotent + resumable: re-driving reads the durable per-node
/// state, claims only un-run ready nodes (CAS), and finalises when every node is terminal. Returns
/// when the frontier exhausts. A suspended run stops enqueuing the next frontier (the unexecuted
/// nodes stay Pending/Enqueued); `flows.resume` re-drives.
///
/// **Mid-run control bites between frontier batches** (flow-runtime-control-scope): before each
/// batch the durable run status is re-read; a `cancelled`/`suspended` written by `flows.cancel`/
/// `flows.suspend` stops the drive — the remaining nodes stay un-run (audit kept), which is what
/// makes Stop actually stop a backgrounded run. On any terminal exit a `run-finished` settle event
/// is published so a watcher retires its live controls.
pub async fn drive(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    params: &serde_json::Map<String, Value>,
    now: u64,
) -> Result<(), String> {
    loop {
        // Control check: a cancel/suspend landed since the last batch → stop driving this run.
        if let Some(status) = control_halt(node, ws, run_id).await? {
            publish_finished(node, ws, run_id, &status).await;
            return Ok(());
        }
        let ready = ready_frontier(&node.store, ws, run_id, flow).await?;
        if ready.is_empty() {
            break;
        }
        for node_id in ready {
            // Box the per-node execution: a `subflow` node re-enters the run engine (drive → execute
            // → subflow → run_to_completion → drive), an async recursion the compiler can only prove
            // `Send` (so the manual run can be `tokio::spawn`ed as a background job) when the cycle is
            // broken by a boxed future here.
            Box::pin(execute_node::execute_one(
                node, principal, ws, run_id, flow, &node_id, params, now,
            ))
            .await?;
        }
        if let Some(status) = run_store::finalize_if_complete(&node.store, ws, flow, run_id).await?
        {
            publish_finished(node, ws, run_id, &status).await;
            break;
        }
    }
    Ok(())
}

/// Read the run's durable status; return `Some(status)` if it is a control-terminal the driver must
/// halt on (`cancelled`/`suspended`). A missing run (deleted mid-drive) also halts.
async fn control_halt(node: &Arc<Node>, ws: &str, run_id: &str) -> Result<Option<String>, String> {
    match run_store::read_run(&node.store, ws, run_id).await? {
        Some(run) if run.status == "cancelled" || run.status == "suspended" => Ok(Some(run.status)),
        Some(_) => Ok(None),
        None => Ok(Some("cancelled".into())),
    }
}

/// Publish the terminal `run-finished` settle event (best-effort motion).
async fn publish_finished(node: &Arc<Node>, ws: &str, run_id: &str, status: &str) {
    let event = super::watch::run_finished_event(status);
    super::watch::publish_flow_event(&node.bus, ws, run_id, &event).await;
}

/// The set of `Enqueued` (ready) node ids from the durable state.
async fn ready_frontier(
    store: &lb_store::Store,
    ws: &str,
    run_id: &str,
    flow: &Flow,
) -> Result<Vec<String>, String> {
    let mut ready = Vec::new();
    for n in &flow.nodes {
        if let Some(rec) = run_store::read_step(store, ws, run_id, &n.id).await? {
            if rec.claim == ClaimState::Enqueued {
                ready.push(n.id.clone());
            }
        }
    }
    Ok(ready)
}
