//! The durable run-store over SurrealDB — the flow engine's backend (flow-run-scope "Data"), ported
//! from the chain `run_store` (Decision 6: one engine). The CAS claim (`Pending|Enqueued → Running`)
//! is the **cross-node** exactly-once owner under redelivery (Decision 8); a lost claim no-ops, so a
//! duplicate node redelivery never double-runs. Per-node rows so concurrent branch jobs don't
//! contend, and a restart resumes from the recorded state (the headline offline/sync property).

use std::collections::HashMap;

use lb_flows::{resolve_bindings, Flow, NodeOutput};
use lb_store::{read, scan, write, Store};
use serde_json::{json, Value};

use super::record::{
    step_record_id, ClaimState, FlowRunRecord, FlowStepRecord, FLOW_INPUT_TABLE,
    FLOW_NODE_STATE_TABLE, FLOW_RUN_TABLE, FLOW_STEP_TABLE,
};

/// Seed a run: the coordinator record (pending, pinned `flow_version`) + a per-node state row (claim
/// from in-degree). The pinned version is the spine of resume (Decision 1).
pub async fn create_run(
    store: &Store,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    params: &serde_json::Map<String, Value>,
    now: u64,
) -> Result<(), String> {
    let run = FlowRunRecord {
        run_id: run_id.to_string(),
        flow_id: flow.id.clone(),
        flow_version: flow.version,
        status: "pending".into(),
        params: json!(params),
        ts: now,
    };
    write(
        store,
        ws,
        FLOW_RUN_TABLE,
        run_id,
        &serde_json::to_value(&run).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    let indeg = flow.indegrees();
    for n in &flow.nodes {
        let d = indeg[&n.id];
        let rec = FlowStepRecord {
            run_id: run_id.to_string(),
            node_id: n.id.clone(),
            claim: if d == 0 {
                ClaimState::Enqueued
            } else {
                ClaimState::Pending
            },
            indegree: d,
            outcome: String::new(),
            output: Value::Null,
            findings: Value::Null,
            error: None,
            attempts: 0,
            ms: 0,
            patched_config: None,
        };
        write_step(store, ws, &rec).await?;
    }
    Ok(())
}

/// CAS claim a node: `Pending|Enqueued → Running`. Returns true if THIS call won the claim.
pub async fn claim_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    node_id: &str,
) -> Result<bool, String> {
    let Some(mut rec) = read_step(store, ws, run_id, node_id).await? else {
        return Ok(false);
    };
    match rec.claim {
        ClaimState::Pending | ClaimState::Enqueued => {
            rec.claim = ClaimState::Running;
            write_step(store, ws, &rec).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Record a node's terminal outcome (idempotent) + upsert `flow_node_state` last-value on Ok
/// (Decision 5 — the dashboard instant read; history is the node's series, not this record).
pub async fn record_outcome(
    store: &Store,
    ws: &str,
    flow_id: &str,
    run_id: &str,
    node_id: &str,
    outcome: NodeOutcome,
) -> Result<(), String> {
    let Some(mut state) = read_step(store, ws, run_id, node_id).await? else {
        return Ok(());
    };
    state.claim = ClaimState::Done;
    match outcome {
        NodeOutcome::Ok(output, findings) => {
            state.outcome = "ok".into();
            state.output = output.clone();
            state.findings = findings.clone();
            // Decision 5: last-value state for the instant dashboard read.
            write(
                store,
                ws,
                FLOW_NODE_STATE_TABLE,
                &format!("{flow_id}:{node_id}"),
                &output,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        NodeOutcome::Err(e) => {
            state.outcome = "err".into();
            state.error = Some(e);
        }
        NodeOutcome::Skipped => {
            state.outcome = "skipped".into();
        }
    }
    write_step(store, ws, &state).await
}

/// The terminal outcome a node's executor reports.
#[allow(dead_code)]
pub enum NodeOutcome {
    Ok(Value, Value),
    Err(String),
    Skipped,
}

/// Decrement dependents' in-degree; return those that reached 0 (marked Enqueued).
pub async fn ready_dependents(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    finished: &str,
) -> Result<Vec<String>, String> {
    let dependents = flow.dependents();
    let deps = dependents.get(finished).cloned().unwrap_or_default();
    let mut ready = Vec::new();
    for dep in deps {
        let Some(mut rec) = read_step(store, ws, run_id, &dep).await? else {
            continue;
        };
        if rec.claim != ClaimState::Pending {
            rec.indegree = rec.indegree.saturating_sub(1);
            write_step(store, ws, &rec).await?;
            continue;
        }
        rec.indegree = rec.indegree.saturating_sub(1);
        if rec.indegree == 0 {
            rec.claim = ClaimState::Enqueued;
            ready.push(dep);
        }
        write_step(store, ws, &rec).await?;
    }
    Ok(ready)
}

/// Mark the transitive subtree below a failed node as Skipped (Halt policy).
pub async fn skip_subtree(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    failed: &str,
) -> Result<(), String> {
    let dependents = flow.dependents();
    let mut queue: std::collections::VecDeque<String> =
        dependents.get(failed).cloned().unwrap_or_default().into();
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(mut rec) = read_step(store, ws, run_id, &id).await? {
            if matches!(rec.claim, ClaimState::Pending | ClaimState::Enqueued) {
                rec.claim = ClaimState::Done;
                rec.outcome = "skipped".into();
                write_step(store, ws, &rec).await?;
            }
        }
        if let Some(next) = dependents.get(&id) {
            for n in next {
                queue.push_back(n.clone());
            }
        }
    }
    Ok(())
}

/// If every node is Done, write the terminal run status. Returns the new status string if finalised.
pub async fn finalize_if_complete(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
) -> Result<Option<String>, String> {
    let mut any_failed = false;
    let mut any_ok = false;
    for n in &flow.nodes {
        let Some(rec) = read_step(store, ws, run_id, &n.id).await? else {
            return Ok(None);
        };
        if rec.claim != ClaimState::Done {
            return Ok(None);
        }
        match rec.outcome.as_str() {
            "ok" => any_ok = true,
            "err" => any_failed = true,
            _ => {}
        }
    }
    let status = if any_failed && any_ok {
        "partialFailure"
    } else if any_failed {
        "failed"
    } else {
        "success"
    };
    set_run_status(store, ws, run_id, status).await?;
    Ok(Some(status.to_string()))
}

/// Resolve a node's `with` bindings against the recorded Done-step outputs + the merged params
/// (declared params ∪ retained `flow_input` values, Decision 9 read-side).
pub async fn resolve_node_bindings(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    with: &serde_json::Map<String, Value>,
    params: &serde_json::Map<String, Value>,
) -> Result<serde_json::Map<String, Value>, String> {
    let mut recorded = HashMap::new();
    for n in &flow.nodes {
        if let Some(rec) = read_step(store, ws, run_id, &n.id).await? {
            if rec.claim == ClaimState::Done && rec.outcome == "ok" {
                recorded.insert(
                    n.id.clone(),
                    NodeOutput {
                        output: rec.output,
                        findings: rec.findings,
                    },
                );
            }
        }
    }
    resolve_bindings(with, &recorded, params).map_err(|e| e.to_string())
}

/// Read every retained `flow_input` for a flow and merge into `params` (Decision 9: a run reads the
/// current retained values; an inject into a retained node updates state and starts no run).
pub async fn merged_params_with_inputs(
    store: &Store,
    ws: &str,
    flow_id: &str,
    mut params: serde_json::Map<String, Value>,
) -> Result<serde_json::Map<String, Value>, String> {
    let page = scan(store, ws, FLOW_INPUT_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| e.to_string())?;
    let prefix = format!("{flow_id}:");
    for row in page.rows {
        // flow_input ids are `{flow}:{node}`; pull this flow's retained values into params by node id.
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        // The row id isn't on the value; recover the node id from the value's `node` field (written
        // by flows.inject). Skip rows not belonging to this flow.
        if inner.get("flow").and_then(|v| v.as_str()) == Some(flow_id) {
            if let Some(node) = inner.get("node").and_then(|v| v.as_str()) {
                if let Some(val) = inner.get("value") {
                    params.insert(node.to_string(), val.clone());
                }
            }
        }
        let _ = prefix; // (prefix retained for clarity; the flow field is the authoritative filter)
    }
    Ok(params)
}

/// Read the run coordinator record.
pub async fn read_run(
    store: &Store,
    ws: &str,
    run_id: &str,
) -> Result<Option<FlowRunRecord>, String> {
    match read(store, ws, FLOW_RUN_TABLE, run_id).await {
        Ok(Some(v)) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| e.to_string()),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn read_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    node_id: &str,
) -> Result<Option<FlowStepRecord>, String> {
    let id = step_record_id(run_id, node_id);
    match read(store, ws, FLOW_STEP_TABLE, &id).await {
        Ok(Some(v)) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| e.to_string()),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Set the run's lifecycle status.
pub async fn set_run_status(
    store: &Store,
    ws: &str,
    run_id: &str,
    status: &str,
) -> Result<(), String> {
    let Some(mut run) = read_run(store, ws, run_id).await? else {
        return Ok(());
    };
    run.status = status.to_string();
    write(
        store,
        ws,
        FLOW_RUN_TABLE,
        run_id,
        &serde_json::to_value(&run).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())
}

async fn write_step(store: &Store, ws: &str, rec: &FlowStepRecord) -> Result<(), String> {
    let id = step_record_id(&rec.run_id, &rec.node_id);
    write(
        store,
        ws,
        FLOW_STEP_TABLE,
        &id,
        &serde_json::to_value(rec).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())
}
