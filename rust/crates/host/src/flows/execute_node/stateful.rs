//! The **stateful** node legs (data-nodes Tier B): `filter` (report-by-exception), `unique` (dedupe),
//! `batch` (group N). Each reads durable per-node state across firings and survives a restart. `filter`
//! reuses the Decision-5 last-value record (`flow_node_state`); `unique`-stream and `batch` use the
//! bounded accumulator ([`super::super::buffer`]). A node that SUPPRESSES this firing (an unchanged
//! `filter`, a buffering `batch`, a `unique` duplicate) settles [`NodeOutcome::Skipped`], which gates
//! its downstream subtree in `execute_one` (no message flows).

use std::sync::Arc;

use lb_flows::ops::{path, predicate};
use lb_flows::Flow;
use lb_store::read;
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::buffer;
use super::super::record::FLOW_NODE_STATE_TABLE;
use super::super::run_store::NodeOutcome;

/// Read the comparison value out of `payload` (a `config.path` sub-value, or the whole payload).
fn keyed(config: &Value, payload: &Value) -> Value {
    match config.get("path").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => path::get(payload, p),
        _ => payload.clone(),
    }
}

/// `filter` — report-by-exception. Pass the message only if its value **changed** vs. the last one
/// (mode `changed`), or moved more than `deadband` numerically (mode `deadband`). The last value is
/// this node's own `flow_node_state` payload (Decision 5's record verbatim); the first firing always
/// passes. A pass emits the payload through; a suppress settles `Skipped`.
pub(super) async fn filter(
    node: &Arc<Node>,
    ws: &str,
    flow: &Flow,
    node_id: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    _now: u64,
) -> NodeOutcome {
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    let cur = keyed(config, &payload);
    let mode = config
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("changed");
    let deadband = config
        .get("deadband")
        .and_then(predicate::as_f64)
        .unwrap_or(0.0);

    // The node's last recorded output envelope (its previous firing's emission), if any.
    let last = match read(
        &node.store,
        ws,
        FLOW_NODE_STATE_TABLE,
        &format!("{}:{}", flow.id, node_id),
    )
    .await
    {
        Ok(Some(env)) => Some(keyed(config, env.get("payload").unwrap_or(&Value::Null))),
        _ => None,
    };

    let pass = match &last {
        None => true, // first firing always passes (nothing to compare against)
        Some(prev) => match mode {
            "deadband" => match (predicate::as_f64(prev), predicate::as_f64(&cur)) {
                (Some(a), Some(b)) => (a - b).abs() > deadband,
                // A non-numeric side under deadband falls back to "changed" semantics.
                _ => prev != &cur,
            },
            // "changed" (default): pass on any (deep) inequality.
            _ => prev != &cur,
        },
    };
    if pass {
        NodeOutcome::ok(json!({ "payload": payload }))
    } else {
        NodeOutcome::Skipped
    }
}

/// `unique` — dedupe. `array` mode (default): drop duplicate elements of an array payload, preserving
/// first-seen order (stateless); a non-array payload passes through unchanged. `stream` mode: drop a
/// payload already seen across firings (a durable, capped seen-set) — a new key passes, a duplicate
/// settles `Skipped`. The dedupe key is `config.path` of the element/payload, or the whole value.
pub(super) async fn unique(
    node: &Arc<Node>,
    ws: &str,
    flow: &Flow,
    node_id: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    let mode = config
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("array");
    if mode == "stream" {
        let key = keyed(config, &payload);
        return match buffer::unique_seen(&node.store, ws, &flow.id, node_id, &key, now).await {
            Ok(true) => NodeOutcome::ok(json!({ "payload": payload })),
            Ok(false) => NodeOutcome::Skipped,
            Err(e) => NodeOutcome::Err(format!("unique: {e}")),
        };
    }
    // array mode: stateless element dedupe by key, order-preserving.
    let Value::Array(arr) = &payload else {
        return NodeOutcome::ok(json!({ "payload": payload }));
    };
    let mut seen: Vec<Value> = Vec::new();
    let mut out: Vec<Value> = Vec::new();
    for elem in arr {
        let key = keyed(config, elem);
        if !seen.contains(&key) {
            seen.push(key);
            out.push(elem.clone());
        }
    }
    NodeOutcome::ok(json!({ "payload": out }))
}

/// `batch` — group N incoming payloads into one array. Appends this firing's payload to the durable
/// bounded buffer; when the buffer reaches `count` (or the buffer cap — force-release, Q3) it emits
/// the grouped array and clears. Otherwise it buffers and settles `Skipped` (suppress until release).
pub(super) async fn batch(
    node: &Arc<Node>,
    ws: &str,
    flow: &Flow,
    node_id: &str,
    inputs: &serde_json::Map<String, Value>,
    config: &Value,
    now: u64,
) -> NodeOutcome {
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    let count = config.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    match buffer::batch_append(&node.store, ws, &flow.id, node_id, payload, count, now).await {
        Ok(step) => match step.released {
            Some(items) => NodeOutcome::ok(json!({ "payload": items })),
            None => NodeOutcome::Skipped,
        },
        Err(e) => NodeOutcome::Err(format!("batch: {e}")),
    }
}
