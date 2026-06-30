//! Execute one node (flow-run-scope). The frontier claims a node (CAS), resolves its `with` bindings
//! against recorded upstream outputs + the run params (declared ∪ retained `flow_input`, Decision 9),
//! dispatches it under the caller's authority, records the outcome, then releases dependents / prunes
//! on failure. Dispatch is by node type:
//! - `tool` — the "everything-is-a-node" generic: dispatches the granted MCP verb in its config under
//!   the caller's own cap (`caller ∩ grant`, no widening — the headline deny test).
//! - `rhai` — the lb-rules cage, via `rules.run`.
//! - `sink` — a terminal write: `series`→`ingest.write`, `outbox`→the outbox (must-deliver),
//!   `channel`/`inbox`→`inbox.record`.
//! - `subflow` — a pinned child run the node parks on (Decision 11); the child is driven to
//!   terminal, then its outputs map to this node's output.
//! - `trigger` — the run's entry; its output is the firing payload (the trigger value).
//! - an extension node (`<ext>.<type>`) — its bound `<ext>.<tool>` dispatched through `call_tool`
//!   under `caller ∩ install-grant` (slice 3 detail).
//!
//! Every dispatch goes through the one host chokepoint `call_tool`, so each node-tool's own gate is
//! re-checked — a flow whose node calls a tool the caller lacks is **denied at that node**, recorded
//! `Err`, and the run continues under `FailurePolicy` (no widening).

use std::sync::Arc;

use lb_auth::Principal;
use lb_flows::{is_builtin_type, Flow};
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;
use crate::tool_call::call_tool;

use super::run::{run_flow_to_completion, child_run_id};
use super::run_store::{self, NodeOutcome};
use super::save::flows_get_internal;

/// Claim + run one node, then release its dependents / prune on failure.
#[allow(clippy::too_many_arguments)]
pub async fn execute_one(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    node_id: &str,
    params: &serde_json::Map<String, Value>,
    now: u64,
) -> Result<(), String> {
    // CAS: only the winner runs the node (redelivery no-op).
    if !run_store::claim_step(&node.store, ws, run_id, node_id).await? {
        return Ok(());
    }
    let node_spec = flow.node(node_id).ok_or_else(|| format!("node {node_id} not in flow"))?;

    // Decision 1/12: a config-only `flows.patch_run` on this UNEXECUTED node overrides the flow's
    // node config for this run (the patched value the operator set during a suspend).
    let step = run_store::read_step(&node.store, ws, run_id, node_id).await?;
    let config = step
        .as_ref()
        .and_then(|s| s.patched_config.clone())
        .unwrap_or_else(|| node_spec.config.clone());

    let inputs =
        run_store::resolve_node_bindings(&node.store, ws, flow, run_id, &node_spec.with, params)
            .await?;

    let outcome = dispatch(node, principal, ws, run_id, flow, node_id, &config, inputs, params, now)
        .await;
    let failed = matches!(outcome, NodeOutcome::Err(_));

    run_store::record_outcome(&node.store, ws, &flow.id, run_id, node_id, outcome).await?;

    if failed && flow.failure_policy == lb_flows::FailurePolicy::Halt {
        run_store::skip_subtree(&node.store, ws, flow, run_id, node_id).await?;
    } else {
        let _ = run_store::ready_dependents(&node.store, ws, flow, run_id, node_id).await?;
    }
    Ok(())
}

/// Dispatch a node by type, returning its outcome. Every leg runs under `principal` through
/// `call_tool` so the node-tool's own gate re-checks (no widening).
#[allow(clippy::too_many_arguments)]
async fn dispatch(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    node_id: &str,
    config: &Value,
    inputs: serde_json::Map<String, Value>,
    params: &serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let node_type = flow.node(node_id).map(|n| n.node_type.as_str()).unwrap_or("");
    if !is_builtin_type(node_type) {
        // An extension node: dispatch its bound `<ext>.<tool>` (the node_type IS `<ext>.<type>`;
        // the tool binding lives on the descriptor — recovered as `<ext>.<tool>` where tool is the
        // sub-type after the dot... but a node's executing tool is the descriptor's `tool`, which for
        // an ext node is `<ext>.<tool>`. Without the descriptor here, the contract is: the node's
        // `config` carries everything the tool needs; dispatch `<ext>.<tool>` where <tool> is named by
        // the node's config `tool` field, falling back to the sub-type. Slice 3 resolves the
        // descriptor and passes the exact tool binding.
        let ext_tool = config
            .get("tool")
            .and_then(|v| v.as_str())
            .map(|t| format!("{node_type}"))
            .unwrap_or_else(|| node_type.to_string());
        return call_tool_node(node, principal, ws, &ext_tool, &inputs).await;
    }
    match node_type {
        "trigger" => {
            // The entry node: its output is the firing payload (the trigger value), read from params
            // under the node id (set by the firing path), else the resolved `with`.
            let payload = params.get(node_id).cloned().unwrap_or_else(|| Value::Object(inputs));
            NodeOutcome::Ok(payload, Value::Null)
        }
        "tool" => {
            let verb = config.get("verb").and_then(|v| v.as_str()).unwrap_or("");
            if verb.is_empty() {
                return NodeOutcome::Err("tool node missing config.verb".into());
            }
            let mut args = config.get("args").cloned().unwrap_or(Value::Object(Default::default()));
            if let Value::Object(map) = &mut args {
                for (k, v) in inputs {
                    map.insert(k, v);
                }
            }
            call_tool_node(node, principal, ws, verb, &serde_args(&args)).await
        }
        "rhai" => {
            let source = config.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let req = json!({ "body": source, "params": Value::Object(inputs), "ts": now });
            match call_tool(node, principal, ws, "rules.run", &req.to_string()).await {
                Ok(out) => {
                    let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
                    NodeOutcome::Ok(v.get("output").cloned().unwrap_or(Value::Null), v.get("findings").cloned().unwrap_or(Value::Null))
                }
                Err(e) => NodeOutcome::Err(tool_err_string(e)),
            }
        }
        "sink" => dispatch_sink(node, principal, ws, run_id, flow, node_id, config, inputs, now).await,
        "subflow" => dispatch_subflow(node, principal, ws, config, inputs, now).await,
        _ => NodeOutcome::Err(format!("unknown built-in node type: {node_type}")),
    }
}

/// A `sink` node: a terminal write. `series`→`ingest.write`, `outbox`→the outbox (must-deliver,
/// idempotent), `channel`/`inbox`→`inbox.record`.
async fn dispatch_sink(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    _flow: &Flow,
    node_id: &str,
    config: &Value,
    inputs: serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let target = config.get("target").and_then(|v| v.as_str()).unwrap_or("");
    let name = config.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let value = inputs.get("value").cloned().unwrap_or(Value::Null);
    match target {
        "series" => {
            let req = json!({ "samples": [{ "series": name, "value": value, "ts": now }] });
            match call_tool(node, principal, ws, "ingest.write", &req.to_string()).await {
                Ok(_) => NodeOutcome::Ok(json!({"accepted": 1}), Value::Null),
                Err(e) => NodeOutcome::Err(tool_err_string(e)),
            }
        }
        "outbox" => {
            // A must-deliver sink stages an outbox effect (transactional, idempotent on the effect
            // id). The deterministic id from (run, node) makes a resume/retry a no-op (no double-send).
            let effect_id = format!("{run_id}:{node_id}");
            match crate::outbox::enqueue_outbox(&node.store, principal, ws, &effect_id, name, "write", &value.to_string(), now).await {
                Ok(()) => NodeOutcome::Ok(json!({"enqueued": effect_id}), Value::Null),
                Err(e) => NodeOutcome::Err(e.to_string()),
            }
        }
        "channel" | "inbox" => {
            let req = json!({ "channel": name, "body": value });
            match call_tool(node, principal, ws, "inbox.record", &req.to_string()).await {
                Ok(_) => NodeOutcome::Ok(json!({"recorded": true}), Value::Null),
                Err(e) => NodeOutcome::Err(tool_err_string(e)),
            }
        }
        _ => NodeOutcome::Err(format!("sink node has unknown target: {target}")),
    }
}

/// A `subflow` node: park on a pinned child run (Decision 11). The child flow is loaded, a pinned
/// child run is created + driven to terminal inline (the parent step waits on child completion),
/// then the child's terminal node outputs map to this node's output. A child failure → this node's
/// `Err` under the parent's `FailurePolicy`. v1 realises "park" as an inline drive (the child IS a
/// real pinned flow_run; the CAS claim keeps it exactly-once); a reactor-driven park is a follow-up.
async fn dispatch_subflow(
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
    // Map parent inputs → child params by name (Decision 4 binding grammar: whole-value references
    // resolved on the parent side arrive here as literals).
    for (k, v) in inputs {
        child_params.insert(k, v);
    }
    match run_flow_to_completion(node, principal, ws, &child, child_params, &child_run, now).await {
        Ok(status) if status == "success" => {
            // Read the child's terminal-node outputs and fold them into this node's output.
            let mut folded = serde_json::Map::new();
            for n in &child.nodes {
                if let Ok(Some(rec)) = run_store::read_step(&node.store, ws, &child_run, &n.id).await {
                    if rec.outcome == "ok" {
                        folded.insert(n.id.clone(), rec.output);
                    }
                }
            }
            NodeOutcome::Ok(Value::Object(folded), Value::Null)
        }
        Ok(status) => NodeOutcome::Err(format!("subflow child {child_id} ended {status}")),
        Err(e) => NodeOutcome::Err(e),
    }
}

/// Dispatch a `<verb>` call through the one chokepoint and reduce to a `NodeOutcome`.
async fn call_tool_node(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    verb: &str,
    args: &serde_json::Map<String, Value>,
) -> NodeOutcome {
    match call_tool(node, principal, ws, verb, &serde_args(&Value::Object(args.clone()))).await {
        Ok(out) => {
            let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
            NodeOutcome::Ok(v, Value::Null)
        }
        Err(e) => NodeOutcome::Err(tool_err_string(e)),
    }
}

fn serde_args(v: &Value) -> String {
    v.to_string()
}

fn tool_err_string(e: ToolError) -> String {
    match e {
        ToolError::Denied => "denied".into(),
        other => other.to_string(),
    }
}
