//! The `tools.*` MCP bridge dispatch (channels-command-palette scope, README §6.5: host-native
//! verbs reached under the one MCP contract). UI and agents reach `tools.catalog` the same way they
//! reach any tool: a qualified call with JSON in/out. The verb authorizes first (the
//! `mcp:tools.<verb>:call` gate runs inside the service function via `authorize_tool`); a denial is
//! opaque (`ToolError::Denied`), so a caller without the gate learns nothing about which tools exist.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::catalog::tools_catalog;
use crate::boot::Node;

/// Dispatch a `tools.*` MCP call. `input` is ignored for the read-only `tools.catalog` (the
/// workspace is the whole scope, taken from the caller). The return is the verb's JSON result.
pub async fn call_tools_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    _input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "tools.catalog" => {
            let catalog = tools_catalog(node, principal, ws).await?;
            Ok(json!(catalog))
        }
        _ => Err(ToolError::NotFound),
    }
}
