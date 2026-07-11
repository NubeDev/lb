//! The `switch` node's **edge gating** (Decision 14, resolving data-nodes Open Q1). The frontier
//! driver normally activates *every* dependent of a settled node; `switch` needs a settled-but-gated
//! outcome per branch. We resolve Q1 **without a new `Outcome` variant on the wire**: `switch` settles
//! `Ok` (a pass-through envelope) like any node, and the executor computes the routing decision from
//! the node's `config.rules` + the routed value, then releases only the **matched** dependents and
//! gates (skips the subtree of) the rest.
//!
//! Each rule carries `to: [node_ids]` — the downstream nodes wired to that output (the rule→port→wire
//! mapping, made explicit in config since this DAG's edges are node-id `needs` with no port label).
//! *Rejected:* port-labelled edges (a bigger edge-model change than this pack owns) and a null/skip
//! sentinel payload (dependents can't tell "gated" from "a legitimately null value"). Branches are
//! expected disjoint (a Node-RED switch fans to distinct wires); a dependent shared with a live branch
//! is left to fire on its other path.

use std::collections::HashSet;
use std::sync::Arc;

use lb_flows::ops::{path, predicate};
use lb_flows::Flow;
use serde_json::Value;

use crate::boot::Node;

use super::super::run_store;

/// After a `switch` settles (under `fctx`), release its matched dependents and gate the rest — under
/// the SAME `fctx`. Reads the switch's recorded output payload, evaluates the ordered rules against
/// the routed value, and fans out. A matched dependent is released **policy-aware** through the one
/// [`run_store::release_one_dependent`] seam (flow-plain-wiring-scope — the matched-release fix): an
/// `any` dependent port gets a normal minted/propagated firing (`triggered_by` = the switch), an
/// explicit-`all` port keeps the barrier decrement. Releasing unconditionally through the barrier
/// path was the latent hang: a matched switch plus plain wires into one `any` node seeded a Pending
/// barrier slot the sibling any-firings never touched, so the run never reached terminal. An
/// unmatched dependent's `(dep, fctx)` slot is gated Skipped (a `switch` upstream of an `any` port
/// simply contributes no firing for that branch).
#[allow(clippy::too_many_arguments)]
pub(super) async fn release_matched(
    node: &Arc<Node>,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    node_id: &str,
    fctx: &str,
    config: &Value,
    policies: &std::collections::HashMap<String, lb_flows::NodeDescriptor>,
    subgraph: &HashSet<String>,
) -> Result<(), String> {
    // The routed value: the switch's payload (its input, passed through), narrowed by `property`.
    let payload = run_store::read_step(&node.store, ws, run_id, node_id, fctx)
        .await?
        .map(|s| s.output.get("payload").cloned().unwrap_or(Value::Null))
        .unwrap_or(Value::Null);
    let routed = match config.get("property").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => path::get(&payload, p),
        _ => payload,
    };

    let fire = matched_targets(config, &routed);

    let deps = flow.dependents();
    let switch_deps = deps.get(node_id).cloned().unwrap_or_default();
    for dep in switch_deps {
        if !subgraph.contains(&dep) {
            continue;
        }
        if fire.contains(&dep) {
            run_store::release_one_dependent(
                &node.store,
                ws,
                flow,
                run_id,
                &dep,
                node_id,
                fctx,
                policies,
                subgraph,
            )
            .await?;
        } else {
            run_store::skip_gated(&node.store, ws, flow, run_id, &dep, fctx).await?;
        }
    }
    Ok(())
}

/// The union of `to` node ids across matched rules (or just the first match when `stop_on_first`). A
/// rule matches when `predicate::eval(op, routed, value)` is true; an `else` rule is the **fallthrough**
/// — it contributes its targets only when **no** non-`else` rule matched (Node-RED "otherwise"), never
/// unconditionally.
fn matched_targets(config: &Value, routed: &Value) -> HashSet<String> {
    let stop_on_first = config
        .get("stop_on_first")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let rules: Vec<&Value> = config
        .get("rules")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let is_else = |r: &Value| r.get("op").and_then(|v| v.as_str()) == Some("else");
    let targets = |r: &Value, fire: &mut HashSet<String>| {
        for t in r.get("to").and_then(|v| v.as_array()).into_iter().flatten() {
            if let Some(id) = t.as_str() {
                fire.insert(id.to_string());
            }
        }
    };

    let mut fire = HashSet::new();
    let mut matched_any = false;
    for rule in &rules {
        if is_else(rule) {
            continue; // fallthrough handled below
        }
        let op = rule.get("op").and_then(|v| v.as_str()).unwrap_or("");
        let operand = rule.get("value").cloned().unwrap_or(Value::Null);
        if predicate::eval(op, routed, &operand) {
            targets(rule, &mut fire);
            matched_any = true;
            if stop_on_first {
                return fire;
            }
        }
    }
    // No concrete rule matched → fire the `else` fallthrough branch(es).
    if !matched_any {
        for rule in rules.iter().filter(|r| is_else(r)) {
            targets(rule, &mut fire);
            if stop_on_first {
                break;
            }
        }
    }
    fire
}
