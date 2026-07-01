//! Execute one node (flow-run-scope). The frontier claims a node (CAS), resolves its `with` bindings
//! against recorded upstream outputs + the run params (declared ∪ retained `flow_input`, Decision 9),
//! dispatches it under the caller's authority, records the outcome, then releases dependents / prunes
//! on failure. Dispatch is by node type:
//! - `tool` — the "everything-is-a-node" generic: dispatches the granted MCP verb in its config under
//!   the caller's own cap (`caller ∩ grant`, no widening — the headline deny test).
//! - `rhai` — the lb-rules cage, via `rules.run`.
//! - `count` — a pure transform: counts its input (array length / object keys / scalar→1).
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

use super::run::{child_run_id, run_flow_to_completion};
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
    let node_spec = flow
        .node(node_id)
        .ok_or_else(|| format!("node {node_id} not in flow"))?;

    // Decision 1/12: a config-only `flows.patch_run` on this UNEXECUTED node overrides the flow's
    // node config for this run (the patched value the operator set during a suspend).
    let step = run_store::read_step(&node.store, ws, run_id, node_id).await?;
    let config = step
        .as_ref()
        .and_then(|s| s.patched_config.clone())
        .unwrap_or_else(|| node_spec.config.clone());

    let resolved = run_store::resolve_node_bindings(
        &node.store,
        ws,
        flow,
        run_id,
        node_id,
        &node_spec.with,
        params,
    )
    .await?;
    let run_store::ResolvedInputs { inputs, carry } = resolved;

    let outcome = dispatch(
        node, principal, ws, run_id, flow, node_id, &config, inputs, params, now,
    )
    .await;
    let failed = matches!(outcome, NodeOutcome::Err(_));

    // D4 carry-forward: attach the carried fields (inputs minus `payload`) to the emitted envelope so
    // `topic` propagates down a linear chain. A join (carry empty) merges nothing.
    let outcome = match outcome {
        NodeOutcome::Ok { emitted, .. } => NodeOutcome::Ok {
            emitted,
            carry: serde_json::Value::Object(carry),
        },
        other => other,
    };
    run_store::record_outcome(&node.store, ws, &flow.id, run_id, node_id, outcome).await?;

    // Record-THEN-publish (flow-runtime-control-scope): the durable outcome is written above; now
    // project it onto the run's settle subject so any watcher sees the node go terminal live. The
    // step is re-read so the event carries exactly what the snapshot would (one projection, no
    // drift). Fire-and-forget — a publish with no subscriber is a no-op and never fails the run.
    if let Ok(Some(rec)) = run_store::read_step(&node.store, ws, run_id, node_id).await {
        let event = super::watch::node_settled_event(
            node_id,
            &rec.outcome,
            &rec.output,
            rec.error.as_deref(),
        );
        super::watch::publish_flow_event(&node.bus, ws, run_id, &event).await;
    }

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
    let node_type = flow
        .node(node_id)
        .map(|n| n.node_type.as_str())
        .unwrap_or("");
    if !is_builtin_type(node_type) {
        // An extension node: dispatch its bound `<ext>.<tool>` (the descriptor's `tool` field), under
        // `caller ∩ install-grant` via the one `call_tool` chokepoint — `build_call_context` derives
        // `effective = caller ∩ install.granted` (extension-nodes-scope, two-direction deny). The
        // descriptor is resolved from the merged registry so the exact tool binding is used.
        let tool = resolve_ext_tool(&node.store, ws, node_type)
            .await
            .unwrap_or_else(|| node_type.to_string());
        // The ext node receives the whole envelope as its input `msg` and its result becomes the
        // emitted `payload` (the participate-in-carry-forward convention, node-descriptor-scope).
        return match call_tool_node(node, principal, ws, &tool, &Value::Object(inputs)).await {
            NodeOutcome::Ok { emitted, .. } => NodeOutcome::ok(json!({ "payload": emitted })),
            other => other,
        };
    }
    match node_type {
        "trigger" => {
            // The entry node (D6): emits `{ payload: <firing value>, topic: <config.topic?> }`. The
            // firing value is read from params under the node id (set by the firing path — a cron ts
            // or an injected payload), else the resolved `with`'s `payload`.
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
        "tool" => {
            let verb = config.get("verb").and_then(|v| v.as_str()).unwrap_or("");
            if verb.is_empty() {
                return NodeOutcome::Err("tool node missing config.verb".into());
            }
            // D6: `config.args` merged with `payload` when `payload` is an object (so a structured
            // message extends the static args). The verb's result becomes the emitted `payload`.
            let mut args = config
                .get("args")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));
            if let (Value::Object(map), Some(Value::Object(p))) = (&mut args, inputs.get("payload"))
            {
                for (k, v) in p {
                    map.insert(k.clone(), v.clone());
                }
            }
            match call_tool_node(node, principal, ws, verb, &args).await {
                NodeOutcome::Ok { emitted, .. } => NodeOutcome::ok(json!({ "payload": emitted })),
                other => other,
            }
        }
        "rhai" => {
            // D6: the whole `msg` is the script scope. If the script returns an object containing a
            // `payload` key, that object IS the emitted envelope (`return msg`); otherwise the return
            // is the new `payload`. A rules `findings` value rides the `findings` field.
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
                        // `return msg` — an object carrying `payload` IS the envelope.
                        Value::Object(m) if m.contains_key("payload") => m.clone(),
                        // Otherwise the return value is the new payload.
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
        "sink" => {
            dispatch_sink(
                node, principal, ws, run_id, flow, node_id, config, inputs, now,
            )
            .await
        }
        "subflow" => dispatch_subflow(node, principal, ws, config, inputs, now).await,
        "count" => {
            // A pure transform: count the input `payload` (D6). An array → its length, an object → its
            // key count, null → 0, any scalar → 1. Emits `{ payload: <size> }`. Stateless: the same
            // input always yields the same count. For a running total use `counter`.
            let n = payload_size(inputs.get("payload"));
            NodeOutcome::ok(json!({ "payload": n }))
        }
        "json" => {
            // The Node-RED `json` node: convert `payload` between a JSON string and a structured value
            // at a text boundary. Stateless, no tool dispatch — a pure transform like `count`. `topic`
            // and friends carry forward via the executor's `carry` merge (we emit only `payload`).
            let mode = config
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("parse");
            let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
            match mode {
                // string → value. A non-string or invalid JSON FAILS (Node-RED parity) so a bad body
                // surfaces under the flow's `FailurePolicy` instead of silently flowing a wrong shape.
                "parse" => match &payload {
                    Value::String(s) => match serde_json::from_str::<Value>(s) {
                        Ok(parsed) => NodeOutcome::ok(json!({ "payload": parsed })),
                        Err(e) => NodeOutcome::Err(format!("json.parse: invalid JSON: {e}")),
                    },
                    _ => NodeOutcome::Err("json.parse: expected a string payload".into()),
                },
                // value → JSON string. `pretty` indents. Never fails (any serde_json::Value serializes).
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
        "counter" => {
            // A STATEFUL accumulator (Node-RED / PLC counter): read this node's durable memory and add
            // to it ATOMICALLY. `mode` is EXPLICIT (D7, the trap removed): `tick` (default) → +`step`
            // every firing regardless of payload; `throughput` → +the size of `payload`. `reset`
            // zeroes the total before this firing's add. The new total is the emitted `payload`.
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
                // "tick" (default): a plain per-firing increment of `step`, REGARDLESS of payload.
                _ => step,
            };
            match lb_store::increment(
                &node.store,
                ws,
                super::record::FLOW_NODE_MEMORY_TABLE,
                &super::record::node_scoped_id(&flow.id, node_id),
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
    // D6: the destination = `msg.topic ?? config.name` (the topic routes the message, like Node-RED).
    let config_name = config.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let topic = inputs.get("topic").and_then(|v| v.as_str());
    let name = topic.filter(|t| !t.is_empty()).unwrap_or(config_name);
    // D6: the sink writes `msg.payload`.
    let value = inputs.get("payload").cloned().unwrap_or(Value::Null);
    match target {
        "series" => {
            // `ingest.write` deserializes each sample into the full `Sample` shape: `producer` is
            // stamped to the authenticated principal INSIDE the verb but must be PRESENT to deserialize
            // (send ""), `seq` is the monotonic (series,producer) dedup key — use `now` so a retry of
            // THIS firing reuses it (idempotent) while successive firings advance it, and the point
            // rides in `payload` (NOT `value`). The prior `{series,value,ts}` shape failed deserialize
            // with "missing field `producer`".
            let req = json!({ "samples": [{
                "series": name,
                "producer": "",
                "ts": now,
                "seq": now,
                "payload": value.clone(),
            }] });
            match Box::pin(call_tool(
                node,
                principal,
                ws,
                "ingest.write",
                &req.to_string(),
            ))
            .await
            {
                // D6 pass-through: the sink emits `{ payload }` so a downstream node (and the canvas)
                // sees what was written; `topic` carries forward.
                Ok(_) => NodeOutcome::ok(json!({ "payload": value })),
                Err(e) => NodeOutcome::Err(tool_err_string(e)),
            }
        }
        "outbox" => {
            // A must-deliver sink stages an outbox effect (transactional, idempotent on the effect
            // id). The deterministic id from (run, node) makes a resume/retry a no-op (no double-send).
            let effect_id = format!("{run_id}:{node_id}");
            match crate::outbox::enqueue_outbox(
                &node.store,
                principal,
                ws,
                &effect_id,
                name,
                "write",
                &value.to_string(),
                now,
            )
            .await
            {
                Ok(()) => NodeOutcome::ok(json!({ "payload": value })),
                Err(e) => NodeOutcome::Err(e.to_string()),
            }
        }
        "channel" | "inbox" => {
            // `inbox.record` requires a stable `id` (idempotent on (channel,id)) — derive it from
            // (run,node) so a resume/retry upserts the same item, never a duplicate; each new firing has
            // a fresh run id so it records a new item. `body` must be a STRING, so stringify a
            // structured value. `author` is forced to the principal inside the verb. The prior
            // `{channel,body}` shape failed with "missing arg: id".
            let id = format!("{run_id}:{node_id}");
            let body = match &value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let req = json!({ "channel": name, "id": id, "body": body, "ts": now });
            match Box::pin(call_tool(
                node,
                principal,
                ws,
                "inbox.record",
                &req.to_string(),
            ))
            .await
            {
                Ok(_) => NodeOutcome::ok(json!({ "payload": value })),
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
    // D6: the subflow reads `payload` in — pass the incoming envelope's fields into the child params
    // (so the child's roots can read `payload`/`topic`).
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
        None, // a subflow runs its child whole-graph (every root), not from one entry.
    ))
    .await
    {
        Ok(status) if status == "success" => {
            // Read the child's terminal-node outputs and fold them into this node's output.
            let mut folded = serde_json::Map::new();
            for n in &child.nodes {
                if let Ok(Some(rec)) =
                    run_store::read_step(&node.store, ws, &child_run, &n.id).await
                {
                    if rec.outcome == "ok" {
                        folded.insert(n.id.clone(), rec.output);
                    }
                }
            }
            // D6: the child's outputs become this node's `payload` (the subflow emits an envelope).
            NodeOutcome::ok(json!({ "payload": Value::Object(folded) }))
        }
        Ok(status) => NodeOutcome::Err(format!("subflow child {child_id} ended {status}")),
        Err(e) => NodeOutcome::Err(e.to_string()),
    }
}

/// Resolve an extension node's bound MCP tool (`<ext>.<tool>`) from the merged registry by node
/// type. Falls back to the node type itself if the descriptor is unavailable (an uninstalled ext —
/// the dispatch then denies at the install-grant gate, honestly).
async fn resolve_ext_tool(store: &lb_store::Store, ws: &str, node_type: &str) -> Option<String> {
    let registry = super::nodes::merged_registry_internal(store, ws)
        .await
        .ok()?;
    registry
        .into_iter()
        .find(|d| d.r#type == node_type)
        .map(|d| d.tool)
}

/// Dispatch a `<verb>` call through the one chokepoint and reduce to a `NodeOutcome`.
async fn call_tool_node(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    verb: &str,
    args: &Value,
) -> NodeOutcome {
    match Box::pin(call_tool(node, principal, ws, verb, &args.to_string())).await {
        Ok(out) => {
            let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
            // The raw verb result; callers wrap it into a `payload` envelope (D6).
            NodeOutcome::ok(v)
        }
        Err(e) => NodeOutcome::Err(tool_err_string(e)),
    }
}

/// The "size" of a `payload` for `count`/`counter` (D6): an array → its length, an object → its key
/// count, null/absent → 0, any scalar → 1.
fn payload_size(payload: Option<&Value>) -> u64 {
    match payload {
        Some(Value::Array(a)) => a.len() as u64,
        Some(Value::Object(m)) => m.len() as u64,
        None | Some(Value::Null) => 0,
        Some(_) => 1,
    }
}

fn tool_err_string(e: ToolError) -> String {
    match e {
        ToolError::Denied => "denied".into(),
        other => other.to_string(),
    }
}

/// Unwrap a serialized `RuleOutput` (`{kind:"scalar", value:v}` / `{kind:"grid", columns, rows}`)
/// to the plain JSON the rhai node turns into its emitted `payload`/envelope (D6).
fn unwrap_rule_output(v: Option<&Value>) -> Value {
    let Some(v) = v else { return Value::Null };
    match v.get("kind").and_then(|k| k.as_str()) {
        Some("scalar") => v.get("value").cloned().unwrap_or(Value::Null),
        Some("grid") => serde_json::json!({ "columns": v["columns"], "rows": v["rows"] }),
        _ => v.clone(),
    }
}
