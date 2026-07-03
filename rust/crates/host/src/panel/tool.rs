//! The MCP bridge for panel verbs — host-native tools under the one MCP contract (library-panels
//! scope). UI, agents, and extensions reach `panel.*` the SAME way they reach any wasm tool: a
//! qualified call with JSON in/out. The MCP gate runs inside each verb FIRST (workspace-first, then
//! `mcp:panel.<verb>:call`), so a ws-B caller or one without the grant is refused before the verb runs
//! (the mandatory deny + isolation tests are real here). Host-native — not in the runtime `Registry`;
//! the gateway routes `panel.*` here for the routed/agent path.
//!
//! `save`/`delete`/`share` take their logical `now` from the args (the caller's clock — determinism
//! §3), exactly as `dashboard.*` does.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::model::{PanelSpec, Visibility};
use super::{
    panel_delete, panel_get, panel_list, panel_save, panel_share, panel_usage, PanelError,
};

/// Dispatch a `panel.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the verb's
/// JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_panel_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "panel.get" => {
            let p = panel_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(p).unwrap_or(Value::Null))
        }
        "panel.list" => {
            let rows = panel_list(store, principal, ws).await.map_err(to_tool)?;
            Ok(json!({ "panels": rows }))
        }
        "panel.save" => {
            let spec: PanelSpec = serde_json::from_value(arg(input, "spec")?.clone())
                .map_err(|e| ToolError::BadInput(format!("spec: {e}")))?;
            let p = panel_save(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                spec,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(p).unwrap_or(Value::Null))
        }
        "panel.delete" => {
            let force = input.get("force").and_then(Value::as_bool).unwrap_or(false);
            panel_delete(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                force,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "panel.share" => {
            let visibility = visibility_arg(input)?;
            let team = input.get("team").and_then(|v| v.as_str());
            let p = panel_share(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                visibility,
                team,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(p).unwrap_or(Value::Null))
        }
        "panel.usage" => {
            let rows = panel_usage(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "usage": rows }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the panel gate's outcome onto the MCP tool error (denials opaque). `InUse` becomes a `BadInput`
/// carrying the referencing dashboards, so a headless caller sees why the delete refused.
fn to_tool(e: PanelError) -> ToolError {
    match e {
        PanelError::Denied => ToolError::Denied,
        PanelError::NotFound => ToolError::NotFound,
        PanelError::BadInput(m) => ToolError::BadInput(m),
        PanelError::InUse(rows) => ToolError::BadInput(format!(
            "panel in use by {} dashboard(s): {}",
            rows.len(),
            rows.iter()
                .map(|r| r.dashboard.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        PanelError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn arg<'a>(input: &'a Value, key: &str) -> Result<&'a Value, ToolError> {
    input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arg(input, key)?
        .as_str()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a string: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    arg(input, key)?
        .as_u64()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a u64: {key}")))
}

/// Parse the `visibility` arg (`"private" | "team" | "workspace"`).
fn visibility_arg(input: &Value) -> Result<Visibility, ToolError> {
    match str_arg(input, "visibility")? {
        "private" => Ok(Visibility::Private),
        "team" => Ok(Visibility::Team),
        "workspace" => Ok(Visibility::Workspace),
        other => Err(ToolError::BadInput(format!("bad visibility: {other}"))),
    }
}
