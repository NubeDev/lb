//! The MCP bridge for the destructive team verbs — host-native tools under the one MCP contract
//! (admin-crud scope). `teams.delete` (cascade) / `teams.rename`. `teams.create`/`list` keep their
//! authz-service bridge; this adds the destructive half. Each authorizes inside the verb.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{teams_delete, teams_rename, TeamsError};

/// Dispatch a destructive `teams.*` MCP call.
pub async fn call_teams_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "teams.delete" => {
            let removed = teams_delete(store, principal, ws, str_arg(input, "team")?)
                .await
                .map_err(teams_to_tool)?;
            Ok(json!({ "ok": true, "members_removed": removed }))
        }
        "teams.rename" => {
            teams_rename(
                store,
                principal,
                ws,
                str_arg(input, "team")?,
                str_arg(input, "name")?,
            )
            .await
            .map_err(teams_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn teams_to_tool(e: TeamsError) -> ToolError {
    match e {
        TeamsError::Denied => ToolError::Denied,
        TeamsError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
