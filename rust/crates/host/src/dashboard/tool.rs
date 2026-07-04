//! The MCP bridge for dashboard verbs — host-native tools under the one MCP contract (README §6.5).
//! UI, agents, and extensions reach `dashboard.*` the SAME way they reach any wasm tool: a qualified
//! call with JSON in/out. The MCP gate runs inside each verb FIRST (workspace-first, then
//! `mcp:dashboard.<verb>:call`), so a ws-B caller or one without the grant is refused before the verb
//! runs (the mandatory deny + isolation tests are real here). Host-native — not in the runtime
//! `Registry`; the gateway routes `dashboard.*` here for the routed/agent path.
//!
//! `save`/`delete`/`share` take their logical `now` from the args (the caller's clock — determinism
//! §3, never wall-clock in the verb), exactly as `assets.put_doc` takes `ts`.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::model::{Cell, Visibility};
use super::{
    dashboard_delete, dashboard_get, dashboard_list, dashboard_pin, dashboard_save,
    dashboard_share, DashboardError,
};

/// Dispatch a `dashboard.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_dashboard_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "dashboard.get" => {
            let d = dashboard_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        "dashboard.list" => {
            let rows = dashboard_list(store, principal, ws)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "dashboards": rows }))
        }
        "dashboard.save" => {
            let cells: Vec<Cell> = serde_json::from_value(arg(input, "cells")?.clone())
                .map_err(|e| ToolError::BadInput(format!("cells: {e}")))?;
            // `variables` is additive — a pre-variables caller omits it (defaults to empty).
            let variables = match input.get("variables") {
                Some(v) if !v.is_null() => serde_json::from_value(v.clone())
                    .map_err(|e| ToolError::BadInput(format!("variables: {e}")))?,
                _ => Vec::new(),
            };
            let d = dashboard_save(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                cells,
                variables,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        "dashboard.pin" => {
            // widget-platform scope, Slice B — mint a cell from an `x-lb-render` envelope and upsert it
            // into a dashboard. `envelope` is the opaque render envelope (a descriptor.result or a channel
            // rich_result body minus kind/v); `dashboard` is the target id (idempotent UPSERT,
            // owner-only update). Gated by `mcp:dashboard.pin:call` (its own cap, distinct from .save).
            let envelope = arg(input, "envelope")?.clone();
            let d = dashboard_pin(
                store,
                principal,
                ws,
                str_arg(input, "dashboard")?,
                input.get("title").and_then(Value::as_str).unwrap_or(""),
                &envelope,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        "dashboard.delete" => {
            dashboard_delete(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "dashboard.share" => {
            let visibility = visibility_arg(input)?;
            let team = input.get("team").and_then(|v| v.as_str());
            let d = dashboard_share(
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
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the dashboard gate's outcome onto the MCP tool error (denials opaque).
fn to_tool(e: DashboardError) -> ToolError {
    match e {
        DashboardError::Denied => ToolError::Denied,
        DashboardError::NotFound => ToolError::NotFound,
        DashboardError::BadInput(m) => ToolError::BadInput(m),
        DashboardError::Store(s) => ToolError::Extension(s.to_string()),
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
