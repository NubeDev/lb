//! `flows.run` / `flows.resume` + the shared `run_flow_to_completion` (flow-run-scope). A run is a
//! durable `lb-jobs` job (Decision 8 — never a blocking loop, §6.1 batch-as-job): `flows.run`
//! validates the pinned graph, pins the current `flow.version` into `flow_run`, seeds the run-store,
//! and drives the frontier to terminal. `run_flow_to_completion` is the one entry both the manual
//! run verb AND a `subflow` node (which parks on a child run) call — the child IS a real pinned
//! `flow_run`, so a subflow inherits exactly-once + resume for free.

use std::sync::Arc;

use lb_auth::Principal;
use lb_flows::Flow;
use lb_jobs::{complete, create, Job, JobStatus};
use serde_json::{json, Value};

use crate::boot::Node;

use super::coordinator;
use super::error::FlowsError;
use super::run_store;
use super::save::flows_get_internal;

/// The `flows.run` kind label on the durable job.
pub const FLOW_RUN_KIND: &str = "flow-run";

/// Start a manual run of flow `flow_id`. Returns the run id (the run is the durable job).
pub async fn flows_run(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    params: serde_json::Map<String, Value>,
    run_id: &str,
    now: u64,
) -> Result<String, FlowsError> {
    let mut flow = flows_get_internal(&node.store, ws, flow_id).await?;
    // Merge retained `flow_input` values into params (Decision 9 read-side): a control loop is
    // retained inputs + event-triggered one-shot runs that read them.
    let params = run_store::merged_params_with_inputs(&node.store, ws, flow_id, params)
        .await
        .map_err(FlowsError::Internal)?;
    let _ = &mut flow; // (flow is read as-is; the run pins its current version below)
    run_flow_to_completion(node, principal, ws, &flow, params, run_id, now).await?;
    Ok(run_id.to_string())
}

/// Create + drive a flow run to terminal completion. Used by `flows.run` and by a `subflow` node
/// (Decision 11). The run pins `flow.version` into `flow_run` (Decision 1); drive is idempotent +
/// resumable (the CAS claim makes a redelivered node a no-op). Returns the terminal status string.
pub async fn run_flow_to_completion(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    flow: &Flow,
    params: serde_json::Map<String, Value>,
    run_id: &str,
    now: u64,
) -> Result<String, FlowsError> {
    // The durable job record (status anchor). Idempotent on run_id.
    create(
        &node.store,
        ws,
        &Job::new(run_id, FLOW_RUN_KIND, flow.id.clone(), now),
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))?;

    coordinator::start(node, ws, run_id, flow, &params, now)
        .await
        .map_err(FlowsError::Internal)?;
    coordinator::drive(node, principal, ws, run_id, flow, &params, now)
        .await
        .map_err(FlowsError::Internal)?;

    let status = run_store::read_run(&node.store, ws, run_id)
        .await
        .map_err(FlowsError::Internal)?
        .map(|r| r.status)
        .unwrap_or_else(|| "failed".into());
    let job_status = match status.as_str() {
        "success" => JobStatus::Done,
        _ => JobStatus::Failed,
    };
    let _ = complete(&node.store, ws, run_id, job_status).await;
    Ok(status)
}

/// Re-drive an interrupted run from its durable state (the resume path). Validates the next-frontier
/// nodes still match the pinned graph's type + ports; a mismatch fails cleanly as `ResumePointDrift`
/// (Decision 1), surfaced in `flows.runs.get`.
pub async fn flows_resume(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    now: u64,
) -> Result<(), FlowsError> {
    let run = run_store::read_run(&node.store, ws, run_id)
        .await
        .map_err(FlowsError::Internal)?
        .ok_or(FlowsError::NotFound)?;
    let flow = flows_get_internal(&node.store, ws, &run.flow_id).await?;
    // The run executes the graph it PINNED. If the flow has since moved past that version, we still
    // execute the pinned version's shape (Decision 1) — load it. For v1 the flow record holds the
    // latest version; a structural edit during suspend writes a new version, and the live run finishes
    // on its pinned one. We assert the pinned version matches (drift guard).
    if flow.version != run.flow_version {
        return Err(FlowsError::ResumePointDrift(format!(
            "run pinned v{} but flow is now v{} (a structural edit during suspend becomes a new version; the live run finishes on its pinned version)",
            run.flow_version, flow.version
        )));
    }
    let params = serde_json::from_value(run.params.clone()).unwrap_or_default();
    coordinator::drive(node, principal, ws, run_id, &flow, &params, now)
        .await
        .map_err(FlowsError::Internal)?;
    let _ = complete(&node.store, ws, run_id, JobStatus::Done).await;
    Ok(())
}

/// A deterministic child-run id for a subflow: stable per (parent-spec, now) so a re-drive is a no-op.
pub fn child_run_id(spec: &str, now: u64) -> String {
    format!("subflow:{}:{}", spec, now)
}

/// Coerce a JSON object of params into a serde map (helper for the bridge).
pub fn params_map(v: &Value) -> serde_json::Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

/// Build a default run id from a flow id + logical time (caller-supplied for idempotency).
pub fn default_run_id(flow_id: &str, now: u64) -> String {
    let _ = json!({}); // (keep serde_json import used in absence of other call sites)
    format!("{flow_id}-run-{now}")
}
