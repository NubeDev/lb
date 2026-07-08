//! The `host.*` MCP dispatcher.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::Value;

use crate::boot::Node;

pub async fn call_host_tool(
    _node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "host.net.info" => {
            authorize_tool(principal, ws, "host.net.info").map_err(|_| ToolError::Denied)?;
            serde_json::to_value(super::net::host_net_info())
                .map_err(|e| ToolError::Extension(e.to_string()))
        }
        "host.net.reach" => {
            authorize_tool(principal, ws, "host.net.reach").map_err(|_| ToolError::Denied)?;
            let out = super::net::host_net_reach(input).await?;
            serde_json::to_value(out).map_err(|e| ToolError::Extension(e.to_string()))
        }
        "host.time.now" => {
            authorize_tool(principal, ws, "host.time.now").map_err(|_| ToolError::Denied)?;
            serde_json::to_value(super::time::host_time_now())
                .map_err(|e| ToolError::Extension(e.to_string()))
        }
        "host.time.zones" => {
            authorize_tool(principal, ws, "host.time.zones").map_err(|_| ToolError::Denied)?;
            serde_json::to_value(super::time::host_time_zones())
                .map_err(|e| ToolError::Extension(e.to_string()))
        }
        "host.fs.stat" => {
            authorize_tool(principal, ws, "host.fs.stat").map_err(|_| ToolError::Denied)?;
            let out = super::fs::host_fs_stat(input)?;
            serde_json::to_value(out).map_err(|e| ToolError::Extension(e.to_string()))
        }
        "host.fs.list" => {
            authorize_tool(principal, ws, "host.fs.list").map_err(|_| ToolError::Denied)?;
            let out = super::fs::host_fs_list(input)?;
            serde_json::to_value(out).map_err(|e| ToolError::Extension(e.to_string()))
        }
        "host.fs.home" => {
            authorize_tool(principal, ws, "host.fs.home").map_err(|_| ToolError::Denied)?;
            let out = super::fs::host_fs_home()?;
            serde_json::to_value(out).map_err(|e| ToolError::Extension(e.to_string()))
        }
        _ => Err(ToolError::NotFound),
    }
}
