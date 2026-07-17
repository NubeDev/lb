//! `flows.enable` + `flows.inject` — the trigger/lifecycle verbs (triggers-lifecycle-scope).
//!
//! - `flows.enable {id, enabled, start_on_boot}` flips the durable lifecycle flags. `enabled=false`
//!   means **no trigger fires** (the cron scan skips it, the event subscription is dropped, boot
//!   won't fire). The `reconcile_flows` loop (separate file) converges the source arm/disarm state.
//! - `flows.inject {id, node, value, port?}` sets a node's **retained** value in `flow_input`
//!   (Decision 9). With `port`, the value lands in the per-port record `flow_input:{flow}:{node}:
//!   {port}`; without it, the node-level `flow_input:{flow}:{node}`. It
//!   and fires a run **only** when the target node is a *firing* trigger (`inject_mode: fire`). An
//!   inject into a `retain` node updates state and starts NO run — the control-loop pattern: a
//!   slider sets a retained `setpoint`, a switch a retained `enabled`, and event-triggered one-shot
//!   runs read them.
//!
//! Both gated (`mcp:flows.enable:call` / `mcp:flows.inject:call`), re-checked per call. Workspace
//! from the token (un-spoofable).

use std::sync::Arc;

use lb_auth::Principal;
use serde_json::Value;

use crate::boot::Node;

use super::error::FlowsError;
use super::record::FLOW_INPUT_TABLE;
use super::save::flows_get_internal;

/// `flows.enable {id, enabled, start_on_boot}` — flip the durable lifecycle flags (idempotent).
pub async fn flows_enable(
    node: &Arc<Node>,
    _principal: &Principal,
    ws: &str,
    id: &str,
    enabled: bool,
    start_on_boot: bool,
) -> Result<(), FlowsError> {
    let mut flow = flows_get_internal(&node.store, ws, id).await?;
    flow.enabled = enabled;
    flow.start_on_boot = start_on_boot;
    // On disable, the reconciler disarms sources (Decision 13); here the flag is the durable intent.
    persist_flow(&node.store, ws, &flow).await
}

/// `flows.inject {id, node, value, port?}` — set a node's retained value (Decision 9). With `port`,
/// the value lands in the **per-port** record `flow_input:{flow}:{node}:{port}`; without it, the
/// **node-level** record `flow_input:{flow}:{node}` (the whole-node payload cron/event runs read).
/// The run's binding resolver prefers per-port retained > node-level retained > static `with`/auto-
/// wire (flow-dashboard-binding-ux-scope). Returns whether a run was fired (only a `fire`-mode
/// trigger node starts a one-shot run).
#[allow(clippy::too_many_arguments)]
pub async fn flows_inject(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    value: Value,
    port: Option<&str>,
    now: u64,
) -> Result<bool, FlowsError> {
    let flow = flows_get_internal(&node.store, ws, flow_id).await?;

    // Decision 9: fire a run only for a FIRING trigger node. A trigger node's config carries
    // `inject_mode: "fire" | "retain"`; non-trigger nodes are retained (no run). An inject into a
    // retain node updates state and starts nothing.
    let firing = is_firing_trigger(&flow, node_id);

    // A fire is an EVENT, not state: it must leave no retained record behind. The firing run below
    // carries the value as an explicit run param, so skipping the write costs the fire nothing —
    // whereas keeping it corrupts every LATER run, because `merged_params_with_inputs` re-merges
    // every retained row into params by node id and would replay the stale poke as this trigger's
    // payload on each subsequent cron/event/boot run.
    if !firing {
        // Persist the retained input — the read-side every run consults. The record id is node-level
        // (`{flow}:{node}`) or per-port (`{flow}:{node}:{port}`); `port` is stashed on the body so the
        // node_state read-back and the resolver can recover which slot it drives.
        let (record_id, rec) = match port {
            Some(p) => (
                format!("{flow_id}:{node_id}:{p}"),
                serde_json::json!({ "flow": flow_id, "node": node_id, "port": p, "value": value }),
            ),
            None => (
                format!("{flow_id}:{node_id}"),
                serde_json::json!({ "flow": flow_id, "node": node_id, "value": value }),
            ),
        };
        lb_store::write(&node.store, ws, FLOW_INPUT_TABLE, &record_id, &rec)
            .await
            .map_err(|e| FlowsError::Internal(e.to_string()))?;
    }

    if firing && flow.enabled {
        let run_id = super::run::default_run_id(&format!("{flow_id}-inject-{node_id}"), now);
        // The injected value is the trigger payload: stash it as a run param the trigger node emits.
        let mut params = serde_json::Map::new();
        params.insert(node_id.to_string(), value);
        // Fire FROM the inject node (entry = node_id): only its downstream subgraph runs (Node-RED
        // "click the inject node"), never the whole flow.
        super::run::flows_run(
            node,
            principal,
            ws,
            flow_id,
            params,
            &run_id,
            now,
            Some(node_id),
        )
        .await?;
        return Ok(true);
    }
    Ok(false)
}

/// Whether `node_id` is a firing inject-trigger (its config `inject_mode` is `fire`, or it is a
/// trigger node without an explicit retain). Non-trigger nodes are retained (the control-loop read-
/// side), so an inject into them never fires.
///
/// This also decides RETENTION, not just firing: a firing target persists nothing (an event has no
/// state), a retained target persists and starts no run. A fire at a disabled flow is therefore a
/// dropped event — it writes no record and starts no run — which is the intended reading; a
/// momentary poke is not queued as state to replay when the flow is re-enabled.
fn is_firing_trigger(flow: &lb_flows::Flow, node_id: &str) -> bool {
    let Some(n) = flow.node(node_id) else {
        return false;
    };
    if n.node_type != "trigger" {
        return false;
    }
    let mode = n
        .config
        .get("inject_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("fire");
    let trig_mode = n
        .config
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("manual");
    // Only an `inject`-mode trigger fires on inject; other trigger kinds (cron/event/boot) do not.
    trig_mode == "inject" && mode == "fire"
}

async fn persist_flow(
    store: &lb_store::Store,
    ws: &str,
    flow: &lb_flows::Flow,
) -> Result<(), FlowsError> {
    lb_store::write(
        store,
        ws,
        super::record::FLOW_TABLE,
        &flow.id,
        &serde_json::to_value(flow).map_err(|e| FlowsError::Internal(e.to_string()))?,
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))
}
