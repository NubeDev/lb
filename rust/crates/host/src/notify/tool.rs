//! The MCP bridge for notify/device verbs (push-target scope).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::verbs::{device_list, device_register, device_remove, notify_send};

/// Dispatch a `device.*` / `notify.*` MCP call.
pub async fn call_notify_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let now = input.get("now").and_then(|v| v.as_u64()).unwrap_or(0);
    match qualified_tool {
        "device.register" => {
            let platform = str_arg(input, "platform")?;
            let token = str_arg(input, "token")?;
            let app_id = input.get("app_id").and_then(|v| v.as_str());
            device_register(store, principal, ws, platform, token, app_id, now)
                .await
                .map_err(|e| ToolError::from(e))?;
            Ok(json!({ "ok": true }))
        }
        "device.list" => {
            let devices = device_list(store, principal, ws)
                .await
                .map_err(|e| ToolError::from(e))?;
            Ok(json!({ "devices": devices }))
        }
        "device.remove" => {
            let id = str_arg(input, "id")?;
            let removed = device_remove(store, principal, ws, id)
                .await
                .map_err(|e| ToolError::from(e))?;
            Ok(json!({ "removed": removed }))
        }
        "notify.send" => {
            let to = input
                .get("to")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ToolError::BadInput("missing to array".into()))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| ToolError::BadInput("to entry not a string".into()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            // Literal title/body are optional when a catalog key is supplied (i18n gap c).
            let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let body = input.get("body").and_then(|v| v.as_str()).unwrap_or("");
            let title_key = input
                .get("title_key")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let body_key = input.get("body_key").and_then(|v| v.as_str()).unwrap_or("");
            let catalog = (!title_key.is_empty() || !body_key.is_empty()).then(|| {
                super::verbs::NotifyCatalogRef {
                    title_key,
                    body_key,
                    args: input.get("args").cloned().unwrap_or(Value::Null),
                }
            });
            let deep_link = input.get("deep_link").and_then(|v| v.as_str());
            let collapse_key = input.get("collapse_key").and_then(|v| v.as_str());
            let priority = input.get("priority").and_then(|v| v.as_str());
            let effect_id = notify_send(
                store,
                principal,
                ws,
                &to,
                title,
                body,
                catalog,
                deep_link,
                collapse_key,
                priority,
                now,
            )
            .await
            .map_err(|e| ToolError::from(e))?;
            Ok(json!({ "effect_id": effect_id }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
