//! The MCP bridge for `template.*` verbs — host-native tools under the one MCP contract (README §6.5).
//! The builder (and any extension/agent) reaches the durable scripted-template CRUD the SAME way it
//! reaches any tool: a qualified call with JSON in/out. The MCP gate runs inside each verb FIRST
//! (workspace-first, then `mcp:template.<verb>:call`), so the mandatory deny + isolation tests are
//! real here. Host-native — not in the runtime `Registry`.
//!
//! `save`/`delete` take their logical `now` from the args (the caller's clock — determinism §3).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::model::Engine;
use super::{template_delete, template_get, template_list, template_save, RenderTemplateError};

/// Dispatch a `template.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_template_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "template.get" => {
            let t = template_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(t).unwrap_or(Value::Null))
        }
        "template.list" => {
            let rows = template_list(store, principal, ws).await.map_err(to_tool)?;
            Ok(json!({ "templates": rows }))
        }
        "template.save" => {
            let t = template_save(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                engine_arg(input)?,
                str_arg(input, "code")?,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(t).unwrap_or(Value::Null))
        }
        "template.delete" => {
            template_delete(
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
        _ => Err(ToolError::NotFound),
    }
}

/// Map the template gate's outcome onto the MCP tool error (denials opaque).
fn to_tool(e: RenderTemplateError) -> ToolError {
    match e {
        RenderTemplateError::Denied => ToolError::Denied,
        RenderTemplateError::NotFound => ToolError::NotFound,
        RenderTemplateError::BadInput(m) => ToolError::BadInput(m),
        RenderTemplateError::Store(s) => ToolError::Extension(s.to_string()),
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

/// Parse the `engine` arg (`"template" | "plot" | "d3"`).
fn engine_arg(input: &Value) -> Result<Engine, ToolError> {
    match str_arg(input, "engine")? {
        "template" => Ok(Engine::Template),
        "plot" => Ok(Engine::Plot),
        "d3" => Ok(Engine::D3),
        other => Err(ToolError::BadInput(format!("bad engine: {other}"))),
    }
}
