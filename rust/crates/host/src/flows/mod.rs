//! The `flows.*` host service — the visual node-graph engine over `lb-jobs` + SurrealDB (flows
//! scope). The pure node model + descriptor contract + DAG math lives in `lb-flows`; the durable run
//! engine, the merged registry, and the `flows.*` MCP surface live HERE.
//!
//! Verbs land one file each (FILE-LAYOUT). This slice ships the **full run surface** over the
//! node-descriptor contract: `flows.save`/`get`/`list`/`delete` (CRUD, DAG + config validated),
//! `flows.run`/`resume`/`suspend`/`cancel` (the durable run + lifecycle), `flows.patch_run`
//! (config-only to an unexecuted node), `flows.runs.get`/`runs.list` (inspection + reattach), and
//! `flows.nodes` (the merged registry). Host-native; gated `mcp:flows.<verb>:call` at the bridge,
//! then each verb's own store surface. Composition, never widening: `flows.run` plus every node-tool's
//! own gate under `caller ∩ grant` (flow-run-scope deny matrix).

pub mod buffer;
pub mod concurrency;
pub mod coordinator;
pub mod error;
pub mod execute_node;
pub mod lifecycle;
pub mod node_config;
pub mod node_state;
pub mod nodes;
pub mod orphan_sweep;
pub mod patch_run;
pub mod react_approval;
pub mod react_cron;
pub mod react_interval;
pub mod react_source;
pub mod reactor_loop;
pub mod reconcile;
pub mod record;
pub mod retain_runs;
pub mod retention_sweep;
pub mod run;
pub mod run_debug;
pub mod run_store;
pub mod runs;
pub mod save;
pub mod source;
pub mod trigger_store;
pub mod triggers;
pub mod watch;

pub use react_approval::{react_to_flow_approvals, FlowApprovalPass};
pub use react_cron::{
    cron_is_valid, cron_run_id, react_to_flows_cron, ReactorPass as FlowReactorPass,
};
pub use react_interval::{flipflop_run_id, react_to_flows_interval};
pub use react_source::{react_to_flow_sources, source_run_id, SourceReactorPass};
pub use reactor_loop::spawn_flow_reactors;
pub use reconcile::{placement_matches, reconcile_flows, ReconcilePass as FlowReconcilePass};
pub use run_debug::{watch_flow_debug, FlowDebugWatch};
pub use source::{arm_source, disarm_source, source_series};
pub use watch::{watch_flow_run, FlowWatch};

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;

use error::FlowsError;
use run::params_map;

/// Dispatch a `flows.*` MCP call (the cap gate already ran in `tool_call`). Each verb re-authorizes
/// its own store surface where it touches data; the run engine re-checks every node-tool's own gate.
pub async fn call_flows_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    dispatch(node, principal, ws, qualified_tool, input)
        .await
        .map_err(FlowsError::to_tool)
}

/// The flows dispatch with a **concrete, boxed** future type — the recursion-cutting entry the host
/// dispatcher (`tool_call.rs`) and a `tool` node re-entry use. Because the return type is a named
/// `Pin<Box<dyn Future + Send>>` (not an opaque `impl Future`), the self-referential cycle that arises
/// when a flow's `tool` node calls `flows.run` (→ background drive → tool → dispatch → here) is
/// broken: the compiler can size the type and prove it `Send`, which the manual run's `tokio::spawn`
/// requires. The `Send` bound is honest — `dispatch` touches only `Arc`/`Store`/`Bus`.
pub fn call_flows_tool_boxed<'a>(
    node: &'a Arc<Node>,
    principal: &'a Principal,
    ws: &'a str,
    qualified_tool: &'a str,
    input: &'a Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, ToolError>> + Send + 'a>> {
    Box::pin(call_flows_tool(node, principal, ws, qualified_tool, input))
}

async fn dispatch(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, FlowsError> {
    match qualified_tool {
        "flows.nodes" => {
            let registry = nodes::flows_nodes(&node.store, principal, ws)
                .await
                .map_err(FlowsError::from)?;
            Ok(json!({ "nodes": registry }))
        }
        "flows.save" => {
            let mut flow: lb_flows::Flow = serde_json::from_value(input.clone())
                .map_err(|e| FlowsError::BadInput(e.to_string()))?;
            let id = save::flows_save(&node.store, principal, ws, &mut flow).await?;
            Ok(json!({ "id": id, "version": flow.version }))
        }
        "flows.get" => {
            let id = str_arg(input, "id")?;
            let flow = save::flows_get(&node.store, principal, ws, id).await?;
            serde_json::to_value(flow).map_err(|e| FlowsError::Internal(e.to_string()))
        }
        "flows.list" => {
            let flows = save::flows_list(&node.store, principal, ws).await?;
            Ok(json!({ "flows": flows }))
        }
        "flows.delete" => {
            let id = str_arg(input, "id")?;
            save::flows_delete(&node.store, principal, ws, id).await?;
            Ok(json!({ "ok": true }))
        }
        "flows.run" => {
            let flow_id = str_arg(input, "id")?;
            let params = params_map(input.get("params").unwrap_or(&Value::Null));
            let now = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            // A manual run with no caller-supplied id mints a fresh, collision-proof ULID — NOT
            // `default_run_id(flow_id, now)`, which derived a constant id whenever `now` was coarse
            // or frozen (the live gateway clock froze at boot), so every re-run re-drove the SAME
            // terminal run and any two overlapping runs raced the run-store's monotonic `rev`
            // (`debugging/flows/frozen-gw-now-collides-run-ids.md`). Each manual Run is now its own
            // distinct `flow_run`. A caller-supplied `run_id` is still honored verbatim — that is the
            // idempotent-retry / resume / subflow path, which WANTS a stable, re-drivable id.
            let run_id = input
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(lb_store::new_ulid);
            // Optional `entry`/`node`: fire from one specific trigger and run only ITS downstream
            // subgraph (Node-RED "click the inject node"). Absent → a whole-graph run from every root
            // (the back-compat "run all"). The canvas passes the clicked trigger's id here.
            let entry = input
                .get("entry")
                .or_else(|| input.get("node"))
                .and_then(|v| v.as_str());
            // The manual run is a BACKGROUND job (flow-runtime-control-scope): seed synchronously,
            // drive on a detached task, return the run id at once — so the caller is freed before the
            // run is terminal and the canvas can watch/poll/cancel mid-flight. (The cron/boot/inject
            // reactors keep the synchronous `flows_run` — they own their own loop cadence.)
            let id =
                run::flows_run_async(node, principal, ws, flow_id, params, &run_id, now, entry)
                    .await?;
            Ok(json!({ "run_id": id }))
        }
        "flows.resume" => {
            let run_id = str_arg(input, "run_id")?;
            let now = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            run::flows_resume(node, principal, ws, run_id, now).await?;
            Ok(json!({ "ok": true }))
        }
        "flows.suspend" => {
            let run_id = str_arg(input, "run_id")?;
            lifecycle::flows_suspend(node, principal, ws, run_id).await?;
            Ok(json!({ "ok": true }))
        }
        "flows.cancel" => {
            let run_id = str_arg(input, "run_id")?;
            lifecycle::flows_cancel(node, principal, ws, run_id).await?;
            Ok(json!({ "ok": true }))
        }
        "flows.enable" => {
            let id = str_arg(input, "id")?;
            let enabled = input
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let start_on_boot = input
                .get("start_on_boot")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            triggers::flows_enable(node, principal, ws, id, enabled, start_on_boot).await?;
            Ok(json!({ "ok": true }))
        }
        "flows.inject" => {
            let id = str_arg(input, "id")?;
            let node_id = str_arg(input, "node")?;
            let value = input.get("value").cloned().unwrap_or(Value::Null);
            let port = input.get("port").and_then(|v| v.as_str());
            let now = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            let fired =
                triggers::flows_inject(node, principal, ws, id, node_id, value, port, now).await?;
            Ok(json!({ "fired_run": fired }))
        }
        "flows.patch_run" => {
            let run_id = str_arg(input, "run_id")?;
            let node_id = str_arg(input, "node")?;
            let config = input.get("config").cloned().unwrap_or(Value::Null);
            patch_run::flows_patch_run(node, principal, ws, run_id, node_id, config).await?;
            Ok(json!({ "ok": true }))
        }
        "flows.runs.get" => {
            let run_id = str_arg(input, "run_id")?;
            runs::flows_runs_get(&node.store, principal, ws, run_id).await
        }
        "flows.runs.list" => {
            let flow_id = str_arg(input, "flow_id").or_else(|_| str_arg(input, "id"))?;
            let status = input.get("status").and_then(|v| v.as_str());
            runs::flows_runs_list(&node.store, principal, ws, flow_id, status).await
        }
        "flows.node.get" => {
            let flow_id = str_arg(input, "id")?;
            let node_id = str_arg(input, "node")?;
            node_config::flows_node_get(&node.store, principal, ws, flow_id, node_id).await
        }
        "flows.node.update" => {
            let flow_id = str_arg(input, "id")?;
            let node_id = str_arg(input, "node")?;
            let config = input.get("config").cloned().unwrap_or(Value::Null);
            node_config::flows_node_update(&node.store, principal, ws, flow_id, node_id, config)
                .await
        }
        "flows.node_state" => {
            let flow_id = str_arg(input, "id")?;
            node_state::flows_node_state(&node.store, principal, ws, flow_id).await
        }
        // `flows.watch` is a live SSE stream, not a JSON dispatch — the gateway's
        // `/flows/runs/{run}/stream` route calls `watch_flow_run` directly (its `mcp:flows.watch:call`
        // gate runs inside). Mirrors `agent.watch`. A JSON call here is therefore not-found.
        // `flows.debug.watch` is likewise a live SSE stream (the per-flow debug tail the canvas debug
        // panel opens): the gateway's `/flows/{id}/debug/stream` route calls `watch_flow_debug`
        // directly. A JSON call here is not-found. (debug-node-scope)
        _ => Err(FlowsError::BadInput(format!(
            "unknown flows verb: {qualified_tool}"
        ))),
    }
}

impl From<nodes::FlowsNodesError> for FlowsError {
    fn from(e: nodes::FlowsNodesError) -> Self {
        match e {
            nodes::FlowsNodesError::Denied => FlowsError::Denied,
            nodes::FlowsNodesError::Internal(m) => FlowsError::Internal(m),
        }
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, FlowsError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| FlowsError::BadInput(format!("missing/invalid arg: {key}")))
}
