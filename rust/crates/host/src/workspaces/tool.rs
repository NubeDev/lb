//! The MCP bridge for the destructive workspace verbs — host-native tools under the one MCP contract
//! (admin-crud scope). `workspace.rename` / `workspace.delete` (soft) / `workspace.purge` (hard).
//! `create`/`list` keep their existing dedicated routes; this bridges the lifecycle verbs an admin
//! agent / the console call. Each authorizes inside the verb; denials are opaque.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{workspace_delete, workspace_purge, workspace_rename, WorkspacesError};

/// Dispatch a destructive `workspace.*` MCP call. `ts` is caller-injected for rename (no wall-clock).
pub async fn call_workspaces_tool(
    store: &Store,
    principal: &Principal,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "workspace.rename" => {
            workspace_rename(
                store,
                principal,
                str_arg(input, "ws")?,
                str_arg(input, "name")?,
                input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0),
            )
            .await
            .map_err(ws_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "workspace.delete" => {
            workspace_delete(store, principal, str_arg(input, "ws")?)
                .await
                .map_err(ws_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "workspace.purge" => {
            workspace_purge(
                store,
                principal,
                str_arg(input, "ws")?,
                str_arg(input, "confirm")?,
            )
            .await
            .map_err(ws_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn ws_to_tool(e: WorkspacesError) -> ToolError {
    match e {
        WorkspacesError::Denied => ToolError::Denied,
        WorkspacesError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
