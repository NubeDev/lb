//! Execute one node (flow-run-scope). The frontier claims a node (CAS), resolves its `with` bindings
//! against recorded upstream envelopes + run params, dispatches it under the caller's authority,
//! records the outcome, then releases dependents / prunes on failure. Dispatch is by node type; the
//! non-trivial legs live in their own verb files (FILE-LAYOUT):
//! - [`core`] — the spine nodes (`trigger`/`tool`/`rhai`/`rule`/`count`/`json`/`counter`).
//! - [`sink`] / [`subflow`] — the terminal write + the pinned child-run park (Decision 11).
//! - [`pure`] — the pure Tier-A data/JSON pack (`change`…`aggregate`, `template`, `csv`…`base64`,
//!   `split`/`join`) — a thin wrapper over the `lb_flows::ops` functions.
//! - [`stateful`] — `filter` (RBE) + `batch`/`unique` (the durable bounded accumulator, Tier B).
//! - [`switch`] — multi-output conditional routing / edge gating (Decision 14).
//! - [`delay`] — durable delay + rate-limit, parking on the resume seam (Decision 16).
//! - [`debug`] — Node-RED's debug node: a motion-only sink that publishes each wire message onto the
//!   per-flow debug subject for the canvas debug panel to tail (debug-node-scope).
//!
//! Every dispatch goes through the one host chokepoint `call_tool`, so each node-tool's own gate is
//! re-checked — a flow whose node calls a tool the caller lacks is **denied at that node** (no
//! widening). The data/JSON pack nodes and the `debug` node dispatch no external tool, so they add no
//! new cap surface — the deny path stays the existing "no `flows.run` cap → the run never starts".

mod approval;
mod core;
mod debug;
mod delay;
mod pure;
mod sink;
mod stateful;
mod subflow;
mod switch;

use std::sync::Arc;

use lb_auth::Principal;
use lb_flows::{is_builtin_type, Flow};
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;
use crate::tool_call::call_tool;

use super::run_store::{self, NodeOutcome};

/// The result of dispatching a node: a normal terminal outcome, or a **park** (the delay node's
/// durable timer has not elapsed — the run suspends and a later resume re-drives this node).
pub(crate) enum Dispatched {
    Settled(NodeOutcome),
    Park,
}

/// Claim + run one node, then release its dependents / prune on failure (or park on a delay timer).
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
    let node_type = node_spec.node_type.clone();

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
    let run_store::ResolvedInputs { inputs, mut carry } = resolved;

    // `join` CONSUMES the sequence `parts` (Decision 15): it must not carry forward onto the joined
    // envelope (that residue would confuse a downstream second join). Every other node lets `parts`
    // ride like `topic` (so it survives a per-element `map` between `split` and `join`).
    if node_type == "join" {
        carry.remove(lb_flows::ops::sequence::PARTS);
    }

    // Per-node timeout (rules-workflow-convergence scope, slice 2): a `config.timeout_ms` wraps the
    // whole dispatch in a wall-clock ceiling. A node that exceeds it settles `err:"timeout"` — its
    // subtree is then gated exactly like any node failure (no downstream fires). This is the generic
    // guard for ANY node type; the `rhai`/`rule` cage deadline (also `timeout_ms`) is the finer-grained
    // rule budget passed through to the sandbox, and this outer wall is the hard ceiling over it.
    let timeout_ms = config.get("timeout_ms").and_then(|v| v.as_u64());
    let dispatch_fut = dispatch(
        node, principal, ws, run_id, flow, node_id, &node_type, &config, inputs, params, now,
    );
    let dispatched = match timeout_ms {
        Some(ms) if ms > 0 => {
            match tokio::time::timeout(std::time::Duration::from_millis(ms), dispatch_fut).await {
                Ok(d) => d,
                Err(_) => Dispatched::Settled(NodeOutcome::Err("timeout".into())),
            }
        }
        _ => dispatch_fut.await,
    };

    let outcome = match dispatched {
        Dispatched::Park => {
            // Durable delay (Decision 16): the timer has not elapsed. Reset this node to Enqueued (so a
            // resume re-drives it) and suspend the run — the drive loop halts at the next control check,
            // the un-run nodes stay pending, `flows.resume` with an advanced clock releases it.
            run_store::park_step(&node.store, ws, run_id, node_id).await?;
            run_store::set_run_status(&node.store, ws, run_id, "suspended").await?;
            return Ok(());
        }
        Dispatched::Settled(o) => o,
    };
    let failed = matches!(outcome, NodeOutcome::Err(_));
    // A stateful node that SUPPRESSES this firing (RBE `filter`, a buffering `batch`, a `unique`
    // duplicate) settles `Skipped` — it fired no message, so its downstream must not run this firing
    // (the Node-RED "the message stops here"). We gate its subtree exactly like a switch's unmatched
    // branch, regardless of `FailurePolicy` (a suppress is not a failure).
    let suppressed = matches!(outcome, NodeOutcome::Skipped);

    // D4 carry-forward: attach the carried fields (inputs minus `payload`) to the emitted envelope so
    // `topic`/`parts` propagate down a linear chain. A join (carry empty) merges nothing.
    let outcome = match outcome {
        NodeOutcome::Ok { emitted, .. } => NodeOutcome::Ok {
            emitted,
            carry: serde_json::Value::Object(carry),
        },
        other => other,
    };
    run_store::record_outcome(&node.store, ws, &flow.id, run_id, node_id, outcome).await?;

    // Record-THEN-publish (flow-runtime-control-scope): project the durable outcome onto the run's
    // settle subject so any watcher sees the node go terminal live. Fire-and-forget.
    if let Ok(Some(rec)) = run_store::read_step(&node.store, ws, run_id, node_id).await {
        let event = super::watch::node_settled_event(
            node_id,
            &rec.outcome,
            &rec.output,
            rec.error.as_deref(),
        );
        super::watch::publish_flow_event(&node.bus, ws, run_id, &event).await;
    }

    if (failed && flow.failure_policy == lb_flows::FailurePolicy::Halt) || suppressed {
        run_store::skip_subtree(&node.store, ws, flow, run_id, node_id).await?;
    } else if node_type == "switch" && !failed {
        // Decision 14 edge-gating: fire only the dependents the matched rules name; gate (skip) the
        // rest of the switch's exclusive subtrees. A `switch` that failed falls through to the normal
        // release path above's Halt/Continue handling.
        switch::release_matched(node, ws, flow, run_id, node_id, &config).await?;
    } else {
        let _ = run_store::ready_dependents(&node.store, ws, flow, run_id, node_id).await?;
    }
    Ok(())
}

/// Dispatch a node by type, returning its outcome (or a park). Every leg runs under `principal`
/// through `call_tool` so the node-tool's own gate re-checks (no widening).
#[allow(clippy::too_many_arguments)]
async fn dispatch(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    node_id: &str,
    node_type: &str,
    config: &Value,
    inputs: serde_json::Map<String, Value>,
    params: &serde_json::Map<String, Value>,
    now: u64,
) -> Dispatched {
    if !is_builtin_type(node_type) {
        // An extension node: dispatch its bound `<ext>.<tool>` under `caller ∩ install-grant`.
        let tool = resolve_ext_tool(&node.store, ws, node_type)
            .await
            .unwrap_or_else(|| node_type.to_string());
        let out = match call_tool_node(node, principal, ws, &tool, &Value::Object(inputs)).await {
            NodeOutcome::Ok { emitted, .. } => NodeOutcome::ok(json!({ "payload": emitted })),
            other => other,
        };
        return Dispatched::Settled(out);
    }

    // The pure data/JSON pack (no store, no dispatch) — the drop-in Tier-A majority + split/join.
    if let Some(out) = pure::dispatch_pure(node_type, config, &inputs) {
        return Dispatched::Settled(out);
    }

    // The stateful + engine-extending nodes, and the spine.
    let settled = match node_type {
        // `flipflop`/`webhook` are sources: each reads the value the reactor placed in params under its
        // node id (the flipped bool / the hit payload) and emits it — the same entry-node leg as
        // `trigger`. `webhook` fires once per hit via the series-event reactor (slice 5).
        "trigger" | "flipflop" | "webhook" => core::trigger(node_id, config, &inputs, params),
        "tool" => core::tool(node, principal, ws, config, &inputs).await,
        "rhai" => core::rhai(node, principal, ws, config, &inputs, now).await,
        "rule" => core::rule(node, principal, ws, config, &inputs, now).await,
        "count" => core::count(&inputs),
        "json" => core::json(config, &inputs),
        "counter" => core::counter(node, ws, flow, node_id, config, &inputs, now).await,
        "sink" => {
            sink::dispatch_sink(node, principal, ws, run_id, node_id, config, inputs, now).await
        }
        "subflow" => subflow::dispatch_subflow(node, principal, ws, config, inputs, now).await,
        "filter" => stateful::filter(node, ws, flow, node_id, config, &inputs, now).await,
        "unique" => stateful::unique(node, ws, flow, node_id, config, &inputs, now).await,
        "batch" => stateful::batch(node, ws, flow, node_id, &inputs, config, now).await,
        // `switch` passes the envelope through unchanged; the routing decision gates dependents in
        // `execute_one` (Decision 14), not here.
        "switch" => NodeOutcome::ok(
            json!({ "payload": inputs.get("payload").cloned().unwrap_or(Value::Null) }),
        ),
        "delay" => {
            return delay::dispatch_delay(node, ws, flow, node_id, config, &inputs, now).await
        }
        // Node-RED's debug node: publish the payload onto the flow's debug subject as motion (no
        // store, no downstream — a terminal observer). Runs under `flows.run`; no new cap.
        "debug" => {
            debug::dispatch_debug(node, ws, &flow.id, run_id, node_id, config, &inputs, now).await
        }
        // The approval gate parks the run (like `delay`) until a reviewer resolves its inbox item; the
        // flow-approval reactor resumes it. Returns `Dispatched` directly (it may Park).
        "approval" => {
            return approval::dispatch_approval(
                node, principal, ws, run_id, node_id, config, &inputs, now,
            )
            .await
        }
        other => NodeOutcome::Err(format!("unknown built-in node type: {other}")),
    };
    Dispatched::Settled(settled)
}

// --- shared helpers used by the category verb files ---

/// Resolve an extension node's bound MCP tool (`<ext>.<tool>`) from the merged registry by node type.
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
pub(super) async fn call_tool_node(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    verb: &str,
    args: &Value,
) -> NodeOutcome {
    match Box::pin(call_tool(node, principal, ws, verb, &args.to_string())).await {
        Ok(out) => {
            let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
            NodeOutcome::ok(v)
        }
        Err(e) => NodeOutcome::Err(tool_err_string(e)),
    }
}

/// The "size" of a `payload` for `count`/`counter` (D6): array → len, object → key count,
/// null/absent → 0, any scalar → 1.
pub(super) fn payload_size(payload: Option<&Value>) -> u64 {
    match payload {
        Some(Value::Array(a)) => a.len() as u64,
        Some(Value::Object(m)) => m.len() as u64,
        None | Some(Value::Null) => 0,
        Some(_) => 1,
    }
}

pub(super) fn tool_err_string(e: ToolError) -> String {
    match e {
        ToolError::Denied => "denied".into(),
        other => other.to_string(),
    }
}

/// Unwrap a serialized `RuleOutput` (`{kind:"scalar", value:v}` / `{kind:"grid", ...}`) to plain JSON.
pub(super) fn unwrap_rule_output(v: Option<&Value>) -> Value {
    let Some(v) = v else { return Value::Null };
    match v.get("kind").and_then(|k| k.as_str()) {
        Some("scalar") => v.get("value").cloned().unwrap_or(Value::Null),
        Some("grid") => serde_json::json!({ "columns": v["columns"], "rows": v["rows"] }),
        _ => v.clone(),
    }
}
