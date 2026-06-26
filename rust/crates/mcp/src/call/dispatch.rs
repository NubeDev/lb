//! The dispatch phase — invoke the tool on the hosting extension's instance.
//!
//! This is the routing seam (mcp scope): local in S1 (lock the in-process instance and call
//! through the WIT `tool.call`); in S3 a remote `Hosted` variant would route over a Zenoh
//! queryable here, with callers and `authorize` unchanged.

use lb_runtime::RuntimeError;

use crate::registry::Hosted;

use super::error::ToolError;

/// Dispatch `qualified_tool`'s call to `target`, passing the JSON input through the WIT
/// boundary. The guest receives the *unqualified* tool name (the `<ext>.` prefix is the
/// host's routing concern, not the extension's).
pub async fn dispatch(
    target: &Hosted,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    let tool = qualified_tool
        .split_once('.')
        .map(|(_, t)| t)
        .unwrap_or(qualified_tool);
    let mut instance = target.instance.lock().await;
    instance.call_tool(tool, input_json).await.map_err(map_err)
}

fn map_err(e: RuntimeError) -> ToolError {
    match e {
        RuntimeError::Tool(m) => ToolError::Extension(m),
        other => ToolError::Extension(other.to_string()),
    }
}
