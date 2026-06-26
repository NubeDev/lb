//! The dispatch phase — invoke the tool, locally or across the bus (the S3 routing seam).
//!
//! This is where "edge↔hub becomes real" for tool calls. The target resolved on THIS node is
//! either:
//!   - [`Target::Local`] — lock the in-process instance and call through the WIT `tool.call`;
//!   - [`Target::Remote`] — `query` the hosting node over the workspace-scoped queryable
//!     (`route::call_key`), passing the qualified tool + input, and unwrap its reply.
//!
//! Callers and `authorize` are unchanged from S1 — authorization already ran on this node,
//! workspace-first, before we ever reach here. The remote node re-runs the *local* dispatch
//! when it answers (it never trusts an unauthorized call: the query only reaches its queryable
//! for the workspace in the key, which is the authorized principal's own workspace).

use lb_bus::{query, Bus};
use lb_runtime::RuntimeError;

use crate::registry::Target;
use crate::route::{call_key, CallReply, CallRequest};

use super::error::ToolError;

/// Dispatch `qualified_tool`'s call to `target`. Local targets call the instance directly;
/// remote targets route over the bus to the hosting node. `bus` and `ws` are needed only for
/// the remote path (the workspace scopes the routing key).
pub async fn dispatch(
    target: &Target,
    bus: &Bus,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    match target {
        Target::Local(hosted) => {
            // The guest receives the *unqualified* tool name (the `<ext>.` prefix is the host's
            // routing concern, not the extension's).
            let tool = unqualify(qualified_tool);
            let mut instance = hosted.instance.lock().await;
            instance.call_tool(tool, input_json).await.map_err(map_err)
        }
        Target::Remote { .. } => route(bus, ws, qualified_tool, input_json).await,
    }
}

/// Route a call to the node hosting `qualified_tool`'s extension over the bus queryable.
async fn route(
    bus: &Bus,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    let ext = qualified_tool
        .split_once('.')
        .map(|(e, _)| e)
        .unwrap_or(qualified_tool);
    let req = CallRequest {
        tool: qualified_tool.to_string(),
        input: input_json.to_string(),
    };
    let bytes = serde_json::to_vec(&req).map_err(|e| ToolError::BadInput(e.to_string()))?;

    let reply = query(bus, ws, &call_key(ext), &bytes)
        .await
        .map_err(|e| ToolError::Extension(format!("route: {e}")))?
        .ok_or_else(|| ToolError::Extension("no node answered the routed call".into()))?;

    match serde_json::from_slice::<CallReply>(&reply)
        .map_err(|e| ToolError::Extension(format!("bad routed reply: {e}")))?
    {
        CallReply::Ok(output) => Ok(output),
        CallReply::Err(msg) => Err(ToolError::Extension(msg)),
    }
}

/// Strip the `<ext>.` prefix to the unqualified tool name the guest expects.
fn unqualify(qualified_tool: &str) -> &str {
    qualified_tool
        .split_once('.')
        .map(|(_, t)| t)
        .unwrap_or(qualified_tool)
}

fn map_err(e: RuntimeError) -> ToolError {
    match e {
        RuntimeError::Tool(m) => ToolError::Extension(m),
        other => ToolError::Extension(other.to_string()),
    }
}
