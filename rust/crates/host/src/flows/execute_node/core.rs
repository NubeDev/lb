//! The spine node legs: `trigger` / `tool` / `rhai` / `rule` / `count` / `json` / `counter`
//! (flow-run-scope). Each returns a [`NodeOutcome`]; the data/JSON pack legs live in sibling files.

use std::sync::Arc;

use lb_auth::Principal;
use lb_flows::Flow;
use serde_json::{json, Value};

use crate::boot::Node;
use crate::tool_call::call_tool;

use super::super::run_store::NodeOutcome;
use super::{call_tool_node, payload_size, tool_err_string, unwrap_rule_output};

/// The entry node (D6): emits `{ payload: <firing value>, topic: <config.topic?> }`. The firing value
/// is read from params under the node id (a cron ts / injected payload), else the resolved `payload`.
pub(super) fn trigger(
    node_id: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    params: &serde_json::Map<String, Value>,
) -> NodeOutcome {
    let payload = params.get(node_id).cloned().unwrap_or_else(|| {
        inputs
            .get("payload")
            .cloned()
            .unwrap_or_else(|| Value::Object(inputs.clone()))
    });
    let mut emitted = serde_json::Map::new();
    emitted.insert("payload".into(), payload);
    if let Some(topic) = config.get("topic").filter(|t| !t.is_null()) {
        emitted.insert("topic".into(), topic.clone());
    }
    NodeOutcome::ok(Value::Object(emitted))
}

/// The everything-is-a-node generic: dispatch the granted MCP verb under the caller's own cap
/// (`caller ∩ grant`, no widening — the headline deny test). `config.args` merged with an object
/// `payload`; the verb's result becomes the emitted `payload`.
pub(super) async fn tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
) -> NodeOutcome {
    let verb = config.get("verb").and_then(|v| v.as_str()).unwrap_or("");
    if verb.is_empty() {
        return NodeOutcome::Err("tool node missing config.verb".into());
    }
    let mut args = config
        .get("args")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    if let (Value::Object(map), Some(Value::Object(p))) = (&mut args, inputs.get("payload")) {
        for (k, v) in p {
            map.insert(k.clone(), v.clone());
        }
    }
    match call_tool_node(node, principal, ws, verb, &args).await {
        NodeOutcome::Ok { emitted, .. } => NodeOutcome::ok(json!({ "payload": emitted })),
        other => other,
    }
}

/// The lb-rules rhai cage (via `rules.eval` — the flow-facing rule entry). The node's `source` is the
/// inline rule body; the resolved inputs ARE the message envelope. An optional `timeout_ms` config
/// overrides the cage deadline for this node (slice 2). See [`eval_rule`] for the result projection.
pub(super) async fn rhai(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let source = config.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let mut req = json!({
        "body": source,
        "envelope": Value::Object(inputs.clone()),
        "ts": now,
    });
    apply_timeout(&mut req, config);
    eval_rule(node, principal, ws, req).await
}

/// The saved-rule node: run a stored rule by name (`config.rule`) with the message envelope as params
/// plus any fixed `config.params` (rules-workflow-convergence scope, slice 1). Same cage, same result
/// projection as `rhai` — the only difference is the rule is selected by id, not inlined.
pub(super) async fn rule(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let rule_id = config.get("rule").and_then(|v| v.as_str()).unwrap_or("");
    if rule_id.is_empty() {
        return NodeOutcome::Err("rule node missing config.rule".into());
    }
    let mut req = json!({
        "rule_id": rule_id,
        "envelope": Value::Object(inputs.clone()),
        "params": config.get("params").cloned().unwrap_or(Value::Null),
        "ts": now,
    });
    apply_timeout(&mut req, config);
    eval_rule(node, principal, ws, req).await
}

/// Copy a node's `timeout_ms` config onto the `rules.eval` request (shared by `rhai`/`rule`).
fn apply_timeout(req: &mut Value, config: &Value) {
    if let Some(t) = config.get("timeout_ms").and_then(|v| v.as_u64()) {
        req["timeout_ms"] = json!(t);
    }
}

/// Dispatch `rules.eval` with a prepared request and project its result onto the emitted envelope. If
/// the rule returned an object with a `payload` key, that object IS the envelope (`return msg`);
/// otherwise the return is the new `payload`. Any `findings` ride alongside so they render on the
/// canvas. Dispatched under the caller's own authority (`caller ∩ grant`) — a caller lacking
/// `mcp:rules.eval:call` is denied at this node (no widening).
async fn eval_rule(node: &Arc<Node>, principal: &Principal, ws: &str, req: Value) -> NodeOutcome {
    match Box::pin(call_tool(
        node,
        principal,
        ws,
        "rules.eval",
        &req.to_string(),
    ))
    .await
    {
        Ok(out) => {
            let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
            let ret = unwrap_rule_output(v.get("output"));
            let findings = v.get("findings").cloned().unwrap_or(Value::Null);
            let mut emitted = match &ret {
                Value::Object(m) if m.contains_key("payload") => m.clone(),
                _ => {
                    let mut m = serde_json::Map::new();
                    m.insert("payload".into(), ret);
                    m
                }
            };
            if !findings.is_null() {
                emitted.insert("findings".into(), findings);
            }
            NodeOutcome::ok(Value::Object(emitted))
        }
        Err(e) => NodeOutcome::Err(tool_err_string(e)),
    }
}

/// A pure transform: count the input `payload` (array len / object keys / scalar→1). Stateless.
pub(super) fn count(inputs: &serde_json::Map<String, Value>) -> NodeOutcome {
    NodeOutcome::ok(json!({ "payload": payload_size(inputs.get("payload")) }))
}

/// The Node-RED `json` node: convert `payload` between a JSON string and a structured value.
/// `parse` (default): string→value (invalid JSON FAILS); `stringify`: value→JSON string.
pub(super) fn json(config: &Value, inputs: &serde_json::Map<String, Value>) -> NodeOutcome {
    let mode = config
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("parse");
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    match mode {
        "parse" => match &payload {
            Value::String(s) => match serde_json::from_str::<Value>(s) {
                Ok(parsed) => NodeOutcome::ok(json!({ "payload": parsed })),
                Err(e) => NodeOutcome::Err(format!("json.parse: invalid JSON: {e}")),
            },
            _ => NodeOutcome::Err("json.parse: expected a string payload".into()),
        },
        "stringify" => {
            let pretty = config
                .get("pretty")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let s = if pretty {
                serde_json::to_string_pretty(&payload)
            } else {
                serde_json::to_string(&payload)
            };
            match s {
                Ok(text) => NodeOutcome::ok(json!({ "payload": text })),
                Err(e) => NodeOutcome::Err(format!("json.stringify: {e}")),
            }
        }
        other => NodeOutcome::Err(format!("json node: unknown mode: {other}")),
    }
}

/// The stateful PLC counter: read this node's durable memory and add to it ATOMICALLY. `tick`→+step
/// per firing; `throughput`→+payload size (D7). `reset` zeroes before the add. New total = `payload`.
pub(super) async fn counter(
    node: &Arc<Node>,
    ws: &str,
    flow: &Flow,
    node_id: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let step = config.get("step").and_then(|v| v.as_i64()).unwrap_or(1);
    let reset = config
        .get("reset")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mode = config
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("tick");
    let by = match mode {
        "throughput" => payload_size(inputs.get("payload")) as i64,
        _ => step,
    };
    match lb_store::increment(
        &node.store,
        ws,
        super::super::record::FLOW_NODE_MEMORY_TABLE,
        &super::super::record::node_scoped_id(&flow.id, node_id),
        by,
        reset,
        now,
    )
    .await
    {
        Ok(total) => NodeOutcome::ok(json!({ "payload": total, "ts": now })),
        Err(e) => NodeOutcome::Err(format!("counter increment failed: {e}")),
    }
}
