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
use lb_runtime::{CallContext, RuntimeError};

use crate::registry::Target;
use crate::route::{call_key, CallReply, CallRequest};

use super::error::ToolError;

/// Dispatch `qualified_tool`'s call to `target`. Local targets call the instance directly;
/// remote targets route over the bus to the hosting node. `bus` and `ws` are needed only for
/// the remote path (the workspace scopes the routing key).
///
/// `ctx` (the host-callback context) is installed into a **local** instance only — for the duration
/// of this one call, then cleared (`instance.call_tool_with`). A remote target gets none: the guest
/// runs on the other node and its callback identity would have to ride the wire (a separate scope).
pub async fn dispatch(
    target: &Target,
    bus: &Bus,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    ctx: Option<CallContext>,
) -> Result<String, ToolError> {
    match target {
        Target::Local(hosted) => {
            // The guest receives the *unqualified* tool name (the `<ext>.` prefix is the host's
            // routing concern, not the extension's).
            let tool = unqualify(qualified_tool);
            // Borrow discipline (host-callback scope, the re-entrancy hazard): there is ONE instance
            // per ext behind this mutex. A guest whose `host.call-tool` re-enters its OWN ext would
            // try to lock the instance its in-flight call already holds — a deadlock. `try_lock`
            // turns that into a clean error instead of a hang; the caller's depth guard bounds
            // legitimate cross-instance chains. (A concurrent *unrelated* call momentarily contending
            // the lock is rare and also surfaces as this transient error rather than blocking — an
            // acceptable trade for never deadlocking on self-re-entry.)
            let mut instance = hosted
                .instance
                .try_lock()
                .map_err(|_| ToolError::Extension("extension busy (re-entrant call)".into()))?;
            instance
                .call_tool_with(tool, input_json, ctx)
                .await
                .map_err(map_err)
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
