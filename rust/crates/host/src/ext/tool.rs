//! The MCP bridge for the unified ext lifecycle verbs — host-native tools under the one MCP contract
//! (lifecycle-management scope). `ext.list` / `ext.enable` / `ext.disable` /
//! `ext.uninstall`. These dispatch by the `Install.tier` inside each verb (the host lifecycle
//! surface), so the caller sees one consistent verb set across both tiers. Each authorizes inside the
//! verb; denials are opaque.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::{ext_disable, ext_enable, ext_list, ext_uninstall, ExtError};
use crate::boot::Node;

/// Dispatch an `ext.*` lifecycle MCP call. `ts` is caller-injected (no wall-clock — testing §3).
pub async fn call_ext_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "ext.list" => {
            let rows = ext_list(node, principal, ws).await.map_err(ext_to_tool)?;
            Ok(json!({ "extensions": rows }))
        }
        "ext.enable" => {
            ext_enable(node, principal, ws, str_arg(input, "ext")?, ts(input))
                .await
                .map_err(ext_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "ext.disable" => {
            ext_disable(node, principal, ws, str_arg(input, "ext")?, ts(input))
                .await
                .map_err(ext_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "ext.uninstall" => {
            ext_uninstall(node, principal, ws, str_arg(input, "ext")?, ts(input))
                .await
                .map_err(ext_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn ext_to_tool(e: ExtError) -> ToolError {
    match e {
        ExtError::Denied => ToolError::Denied,
        ExtError::Unverified => ToolError::BadInput("artifact failed verification".into()),
        ExtError::Store(s) => ToolError::Extension(s.to_string()),
        ExtError::Native(m) => ToolError::Extension(m),
        ExtError::Manifest(m) => ToolError::Extension(m),
    }
}

fn ts(input: &Value) -> u64 {
    input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0)
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
