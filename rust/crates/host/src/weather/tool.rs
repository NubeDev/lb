//! The `weather.*` MCP dispatcher — mirrors `host_tools::tool::call_host_tool`'s shape.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::Value;

pub async fn call_weather_tool(
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "weather.current" => {
            authorize_tool(principal, ws, "weather.current").map_err(|_| ToolError::Denied)?;
            let out = super::current::weather_current(input).await?;
            serde_json::to_value(out).map_err(|e| ToolError::Extension(e.to_string()))
        }
        _ => Err(ToolError::NotFound),
    }
}
