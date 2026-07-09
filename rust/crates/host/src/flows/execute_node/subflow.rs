//! The `subflow` node — park on a pinned child run (Decision 11). The child flow is loaded, a pinned
//! child run is created + driven to terminal inline (the parent step waits on child completion), then
//! the child's terminal node outputs map to this node's output. A child failure → this node's `Err`
//! under the parent's `FailurePolicy`. v1 realises "park" as an inline drive (the child IS a real
//! pinned `flow_run`; the CAS claim keeps it exactly-once).

use std::sync::Arc;

use lb_auth::Principal;
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::run::{child_run_id, run_flow_to_completion};
use super::super::run_store::{self, NodeOutcome};
use super::super::save::flows_get_internal;

pub(super) async fn dispatch_subflow(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let spec = config.get("flow").and_then(|v| v.as_str()).unwrap_or("");
    let (child_id, _child_version) = match spec.split_once('@') {
        Some((id, ver)) => (id, ver.parse::<u32>().unwrap_or(0)),
        None => (spec, 0),
    };
    let child = match flows_get_internal(&node.store, ws, child_id).await {
        Ok(c) => c,
        Err(e) => return NodeOutcome::Err(format!("subflow child {child_id}: {e}")),
    };
    let child_run = child_run_id(spec, now);
    let mut child_params = child.params.clone();
    // D6: pass the incoming envelope's fields into the child params (child roots read payload/topic).
    for (k, v) in inputs {
        child_params.insert(k, v);
    }
    match Box::pin(run_flow_to_completion(
        node,
        principal,
        ws,
        &child,
        child_params,
        &child_run,
        now,
        None,
    ))
    .await
    {
        Ok(status) if status == "success" => {
            let mut folded = serde_json::Map::new();
            for n in &child.nodes {
                if let Ok(Some(rec)) =
                    run_store::read_step(&node.store, ws, &child_run, &n.id, "").await
                {
                    if rec.outcome == "ok" {
                        folded.insert(n.id.clone(), rec.output);
                    }
                }
            }
            NodeOutcome::ok(json!({ "payload": Value::Object(folded) }))
        }
        Ok(status) => NodeOutcome::Err(format!("subflow child {child_id} ended {status}")),
        Err(e) => NodeOutcome::Err(e.to_string()),
    }
}
