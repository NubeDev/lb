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

use super::{system_overview, system_topology, SystemError};
use crate::boot::Node;

/// Dispatch a system-observability MCP call. `input` is ignored (both verbs are whole-workspace
/// snapshots with no arguments); the return is the verb's JSON result. Each verb authorizes first;
/// denials are opaque.
pub async fn call_system_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    _input: &Value,
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
