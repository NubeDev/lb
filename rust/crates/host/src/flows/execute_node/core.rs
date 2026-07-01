//! The spine node legs: `trigger` / `tool` / `rhai` / `count` / `json` / `counter` (flow-run-scope).
//! Each returns a [`NodeOutcome`]; the data/JSON pack legs live in the sibling verb files.

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

/// The lb-rules rhai cage (via `rules.run`). If the script returns an object with a `payload` key,
/// that object IS the emitted envelope (`return msg`); otherwise the return is the new `payload`.
pub(super) async fn rhai(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let source = config.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let req = json!({ "body": source, "params": Value::Object(inputs.clone()), "ts": now });
    match Box::pin(call_tool(
        node,
        principal,
        ws,
        "rules.run",
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
