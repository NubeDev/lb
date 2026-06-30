//! `flows.runs.get` / `flows.runs.list` — run inspection reads (flow-run-scope "Get / List"). `runs.get`
//! rebuilds a run's live per-node status + outcomes + the pinned `flow_version` from the durable
//! records (the canvas poll + the late-join snapshot + the `ResumePointDrift` surface). `runs.list`
//! is the **reattach** surface: a reopened canvas holding only `flow_id` finds the active `run_id`.

use lb_auth::Principal;
use lb_store::Store;
use serde_json::{json, Value};

use super::error::FlowsError;
use super::record::{ClaimState, FLOW_RUN_TABLE};
use super::run_store::read_run;
use super::save::authorize_store_read;

/// `flows.runs.get {run_id}` — a snapshot of per-node status + outcomes + the pinned version.
pub async fn flows_runs_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    run_id: &str,
) -> Result<Value, FlowsError> {
    authorize_store_read(principal, ws)?;
    let run = read_run(store, ws, run_id)
        .await
        .map_err(FlowsError::Internal)?
        .ok_or(FlowsError::NotFound)?;
    // Rebuild the per-node snapshot from the step records. The flow's node order is the canvas order;
    // a node with no step record yet is "pending" (not seeded).
    let steps = node_snapshot(store, ws, run_id).await?;
    Ok(json!({
        "runId": run.run_id,
        "flowId": run.flow_id,
        "flowVersion": run.flow_version,
        "status": run.status,
        // The trigger this run fired from (per-wire run); `null` for a whole-graph run.
        "entryNode": run.entry_node,
        "steps": steps,
    }))
}

/// `flows.runs.list {flow_id, status?}` — the runs of a flow (optionally status-filtered). The
/// reattach surface: a reopened canvas finds the active `run_id` to subscribe `flows.watch` /
/// run-controls to. Never another workspace's runs (the scan is ws-scoped).
pub async fn flows_runs_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    status: Option<&str>,
) -> Result<Value, FlowsError> {
    authorize_store_read(principal, ws)?;
    let page = lb_store::scan(store, ws, FLOW_RUN_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let mut runs = Vec::new();
    for row in page.rows {
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if inner.get("flow_id").and_then(|v| v.as_str()) != Some(flow_id) {
            continue;
        }
        let s = inner.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(want) = status {
            if s != want {
                continue;
            }
        }
        runs.push(json!({
            "runId": inner.get("run_id").cloned().unwrap_or(Value::Null),
            "flowId": flow_id,
            "flowVersion": inner.get("flow_version").cloned().unwrap_or(Value::Null),
            "status": s,
            // The run's logical start instant — the canvas reads it to find the MOST RECENT run (a
            // cron flow's runs are each finite, so "is it running" is "armed + recent runs", not a
            // single live run) and to show "last fired N ago".
            "ts": inner.get("ts").cloned().unwrap_or(Value::Null),
        }));
    }
    // Newest first so the canvas's "latest run" is `runs[0]` without a client-side sort.
    runs.sort_by(|a, b| {
        let ta = a.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
        let tb = b.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
        tb.cmp(&ta)
    });
    Ok(json!({ "runs": runs }))
}

async fn node_snapshot(store: &Store, ws: &str, run_id: &str) -> Result<Vec<Value>, FlowsError> {
    let page = lb_store::scan(
        store,
        ws,
        super::record::FLOW_STEP_TABLE,
        lb_store::MAX_SCAN_LIMIT,
        None,
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let mut steps = Vec::new();
    for row in page.rows {
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if inner.get("run_id").and_then(|v| v.as_str()) != Some(run_id) {
            continue;
        }
        let claim = inner
            .get("claim")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        let terminal = claim == "done";
        steps.push(json!({
            "id": inner.get("node_id").cloned().unwrap_or(Value::Null),
            "claim": claim_for_display(claim),
            "terminal": terminal,
            "outcome": inner.get("outcome").cloned().unwrap_or(Value::Null),
            "output": inner.get("output").cloned().unwrap_or(Value::Null),
            "error": inner.get("error").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(steps)
}

fn claim_for_display(claim: &str) -> &'static str {
    match claim {
        "pending" => "pending",
        "enqueued" => "enqueued",
        "running" => "running",
        "done" => "done",
        _ => "pending",
    }
}

/// Re-export the CAS claim enum for callers that walk the snapshot.
#[allow(dead_code)]
fn _claim_used(_c: ClaimState) {}
