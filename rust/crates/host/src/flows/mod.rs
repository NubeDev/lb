//! The `flows.*` host service — the visual node-graph engine over `lb-jobs` + SurrealDB (flows
//! scope). The pure node model + descriptor contract + DAG math lives in `lb-flows`; the durable
//! run engine, the merged registry, and the `flows.*` MCP surface live HERE.
//!
//! This slice (node-descriptor-scope) ships the **keystone contract surface**: the read-only
//! `flows.nodes` verb returning the **merged registry** = built-ins ∪ every installed extension's
//! validated `[[node]]` descriptors for the calling workspace. Derived, not stored — a read-time
//! union over the workspace's `install` records (each carrying its parsed node blocks). The editor
//! palette renders entirely from this response.
//!
//! Verbs land one file each (FILE-LAYOUT). The run engine (`run`/`runs.get`/`watch`/`suspend`/
//! `resume`/`cancel`/`patch_run`) + flow CRUD (`save`/`get`/`list`/`delete`) arrive in the flow-run
//! slice; the triggers (`enable`/`inject`) in the triggers slice. Host-native; gated
//! `mcp:flows.<verb>:call` at the bridge, then each verb's store surface.

pub mod nodes;

use std::sync::Arc;

use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;

/// Dispatch a `flows.*` MCP call (the cap gate already ran in `tool_call`). Each verb re-authorizes
/// its own store surface where it touches data.
pub async fn call_flows_tool(
    node: &Arc<Node>,
    principal: &lb_auth::Principal,
    ws: &str,
    qualified_tool: &str,
    _input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "flows.nodes" => {
            let registry = nodes::flows_nodes(&node.store, principal, ws)
                .await
                .map_err(flows_to_tool)?;
            Ok(json!({ "nodes": registry }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn flows_to_tool(e: nodes::FlowsNodesError) -> ToolError {
    match e {
        nodes::FlowsNodesError::Denied => ToolError::Denied,
        nodes::FlowsNodesError::Internal(m) => ToolError::Extension(m),
    }
}
