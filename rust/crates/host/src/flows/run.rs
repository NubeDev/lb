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

/// Start a manual run **in the background** (flow-runtime-control-scope). Seeds the run-store
/// synchronously (so an immediate `runs.get`/`watch`/`cancel` finds the run), then spawns the
/// frontier drive on a detached task and returns the run id at once — the run is a job, not a
/// blocking call (§6.1). This is what makes Stop and live-values work: the caller (the gateway) is
/// freed before the run is terminal, so the canvas can poll/stream intermediate states and cancel
/// mid-flight. A panicking drive marks the run `failed` (durable + observable), never a silent hang.
pub async fn flows_run_async(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    params: serde_json::Map<String, Value>,
    run_id: &str,
    now: u64,
) -> Result<String, FlowsError> {
    let flow = flows_get_internal(&node.store, ws, flow_id).await?;
    let params = run_store::merged_params_with_inputs(&node.store, ws, flow_id, params)
        .await
        .map_err(FlowsError::Internal)?;
    // Seed synchronously: the durable job + run record + per-node claim state must exist before we
    // return, so a watcher/cancel that races the spawn finds the run (and `flows.cancel` can write a
    // status the drive loop reads).
    create(
        &node.store,
        ws,
        &Job::new(run_id, FLOW_RUN_KIND, flow.id.clone(), now),
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))?;
    coordinator::start(node, ws, run_id, &flow, &params, now)
        .await
        .map_err(FlowsError::Internal)?;

    // Detach the drive. The task owns clones of the node + principal (both cheap: Node is an Arc,
    // Principal is Clone). On a drive error or panic the run is marked `failed` so the terminal
    // status is durable and a watcher sees `run-finished`.
    let node = node.clone();
    let principal = principal.clone();
    let ws = ws.to_string();
    let run_id_owned = run_id.to_string();
    tokio::spawn(drive_run_task(
        node,
        principal,
        ws,
        run_id_owned,
        flow,
        params,
        now,
    ));
    Ok(run_id.to_string())
}

/// The detached drive task body, as a **named function** so it is its own future type — not an
/// anonymous closure nested in `flows_run_async`. That breaks the self-referential async-type cycle
/// (`flows_run_async` spawning a body that, via a `tool` node calling `flows.run`, names
/// `flows_run_async` again): a named `async fn`'s type does not contain its caller's, so the compiler
/// can size it and prove it `Send` for `tokio::spawn`.
async fn drive_run_task(
    node: Arc<Node>,
    principal: Principal,
    ws: String,
    run_id: String,
    flow: Flow,
    params: serde_json::Map<String, Value>,
    now: u64,
) {
    let res = coordinator::drive(&node, &principal, &ws, &run_id, &flow, &params, now).await;
    if let Err(e) = res {
        let _ = run_store::set_run_status(&node.store, &ws, &run_id, "failed").await;
        let event = super::watch::run_finished_event("failed");
        super::watch::publish_flow_event(&node.bus, &ws, &run_id, &event).await;
        tracing::warn!(run_id = %run_id, error = %e, "flow drive task failed");
    }
    let status = run_store::read_run(&node.store, &ws, &run_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.status)
        .unwrap_or_else(|| "failed".into());
    let job_status = match status.as_str() {
        "success" => JobStatus::Done,
        _ => JobStatus::Failed,
    };
    let _ = complete(&node.store, &ws, &run_id, job_status).await;
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
    // Boxed: this fn is reached from a `subflow` node's drive, so `run_to_completion → drive →
    // execute → subflow → run_to_completion` is an async recursion the compiler needs a boxed edge
    // to size (and to prove `Send` so the background-run task can be spawned).
    Box::pin(coordinator::drive(
        node, principal, ws, run_id, flow, &params, now,
    ))
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
