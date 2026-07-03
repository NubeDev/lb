//! The MCP bridge for the layout verbs — host-native tools under the one MCP contract (README §7).
//! The gate runs inside each verb FIRST (workspace-first, then `mcp:layout.<verb>:call`), so a ws-B
//! caller or one without the grant is refused before the verb runs. `set` takes its logical `now`
//! from the args (the caller's clock), exactly as `nav.pref.set` does.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::Value;

use super::{layout_get, layout_set, LayoutError};
use lb_store::Store;

/// Dispatch a `layout.<verb>` MCP call. Denials are opaque (`ToolError::Denied`).
pub async fn call_layout_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "layout.get" => {
            let l = layout_get(store, principal, ws, str_arg(input, "surface")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(l).unwrap_or(Value::Null))
        }
        "layout.set" => {
            let model = input.get("model").cloned().unwrap_or(Value::Null);
            let l = layout_set(
                store,
                principal,
                ws,
                str_arg(input, "surface")?,
                model,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(l).unwrap_or(Value::Null))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the layout gate's outcome onto the MCP tool error (denials opaque).
fn to_tool(e: LayoutError) -> ToolError {
    match e {
        LayoutError::Denied => ToolError::Denied,
        LayoutError::BadInput(m) => ToolError::BadInput(m),
        LayoutError::Store(s) => ToolError::Extension(s.to_string()),
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
