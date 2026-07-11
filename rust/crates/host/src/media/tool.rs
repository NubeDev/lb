//! The MCP bridge for media verbs (media scope). The chunk upload and serve are HTTP routes
//! (the gateway), not MCP verbs — bytes go over HTTP, not MCP payloads.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::error::MediaError;
use super::{media_delete, media_get, media_list, media_upload_begin, media_upload_commit};

/// Dispatch a `media.*` MCP call.
pub async fn call_media_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let now = input.get("now").and_then(|v| v.as_u64()).unwrap_or(0);
    match qualified_tool {
        "media.upload_begin" => {
            let mime = str_arg(input, "mime")?;
            let bytes = input
                .get("bytes")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ToolError::BadInput("missing bytes arg".into()))?;
            let checksum = str_arg(input, "checksum")?;
            let origin = input.get("origin").and_then(|v| v.as_str());
            let result =
                media_upload_begin(store, principal, ws, mime, bytes, checksum, origin, now)
                    .await
                    .map_err(|e| ToolError::from(e))?;
            Ok(result)
        }
        "media.upload_commit" => {
            let id = str_arg(input, "id")?;
            let result = media_upload_commit(store, principal, ws, id, now)
                .await
                .map_err(|e| ToolError::from(e))?;
            Ok(result)
        }
        "media.get" => {
            let id = str_arg(input, "id")?;
            let media = media_get(store, principal, ws, id)
                .await
                .map_err(|e| ToolError::from(e))?;
            Ok(json!({ "media": media }))
        }
        "media.list" => {
            let list = media_list(store, principal, ws)
                .await
                .map_err(|e| ToolError::from(e))?;
            Ok(json!({ "media": list }))
        }
        "media.delete" => {
            let id = str_arg(input, "id")?;
            media_delete(store, principal, ws, id)
                .await
                .map_err(|e| ToolError::from(e))?;
            Ok(json!({ "ok": true }))
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
