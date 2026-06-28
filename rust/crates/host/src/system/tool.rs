//! The MCP bridge for the system-observability verbs — host-native tools under the one MCP contract
//! (README §6.5). UI and agents reach `system.overview`/`system.topology` the same way they reach any
//! tool: a qualified call with JSON in/out. Each verb authorizes first (admin cap); a denial is
//! opaque (`ToolError::Denied`), so an agent reads the same snapshot it shows a human.
//!
//! Host-native (not a wasm extension), so it is NOT in the runtime `Registry`; the gateway/UI route
//! `system.*` here. Neither verb takes an id arg — the workspace is the whole scope.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::{
    system_acp, system_overview, system_subsystem, system_tools, system_topology, SystemError,
};
use crate::boot::Node;

/// Dispatch a system-observability MCP call. `input` is ignored for the two whole-workspace snapshots
/// (`overview`/`topology`); for `system.subsystem` it carries the `{"id": "<subsystem>"}` to detail.
/// The return is the verb's JSON result. Each verb authorizes first; denials are opaque.
pub async fn call_system_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "system.overview" => {
            let ov = system_overview(node, principal, ws)
                .await
                .map_err(system_to_tool)?;
            Ok(json!(ov))
        }
        "system.topology" => {
            let topo = system_topology(node, principal, ws)
                .await
                .map_err(system_to_tool)?;
            Ok(json!(topo))
        }
        "system.subsystem" => {
            // The id selects which subsystem to detail; a missing/blank id is an opaque Denied (the
            // same answer an unknown id gets — no "which ids exist" signal).
            let id = input.get("id").and_then(Value::as_str).unwrap_or("");
            let detail = system_subsystem(node, principal, ws, id)
                .await
                .map_err(system_to_tool)?;
            Ok(json!(detail))
        }
        "system.tools" => {
            let tools = system_tools(node, principal, ws)
                .await
                .map_err(system_to_tool)?;
            Ok(json!(tools))
        }
        "system.acp" => {
            let info = system_acp(principal, ws).await.map_err(system_to_tool)?;
            Ok(json!(info))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the system gate's outcome onto the MCP tool error. `Denied` stays `Denied` (no existence
/// signal); a store error surfaces as `Extension`.
fn system_to_tool(e: SystemError) -> ToolError {
    match e {
        SystemError::Denied => ToolError::Denied,
        SystemError::Store(s) => ToolError::Extension(s.to_string()),
    }
}
