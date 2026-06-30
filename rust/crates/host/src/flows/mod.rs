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

pub mod coordinator;
pub mod error;
pub mod execute_node;
pub mod lifecycle;
pub mod nodes;
pub mod patch_run;
pub mod record;
pub mod run;
pub mod run_store;
pub mod runs;
pub mod save;
pub mod source;

pub use source::{arm_source, disarm_source, source_series};

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;

use error::FlowsError;
use run::{default_run_id, params_map};

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
            let run_id = input
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| default_run_id(flow_id, now));
            let id = run::flows_run(node, principal, ws, flow_id, params, &run_id, now).await?;
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
