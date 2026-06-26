//! The serving side of a routed call: answer a [`CallRequest`] that arrived over the bus by
//! running this node's **local** dispatch, then encode a [`CallReply`].
//!
//! This is the mirror of the remote branch in `dispatch`. The serving node only ever reaches
//! here for a query on `ws/{id}/mcp/{ext}/call` — a workspace-scoped key — so it answers only
//! for the workspace the (already-authorized) caller was scoped to. It does NOT re-authorize:
//! the calling node ran `caps::check` workspace-first before routing, and the workspace wall on
//! the queryable key means a call for workspace B physically cannot reach a serving queryable
//! for workspace A (§7). The serving node simply executes the tool it locally hosts.

use lb_runtime::RuntimeError;

use crate::registry::{Registry, Target};
use crate::route::{CallReply, CallRequest};

/// Run `req` against this node's local registry and produce the reply to send back. A request
/// for a tool this node does not host locally (or any extension error) maps to `CallReply::Err`
/// — the calling node surfaces it as a `ToolError::Extension`.
pub async fn serve_call(registry: &Registry, req: &CallRequest) -> CallReply {
    let Some((ext_id, tool)) = req.tool.split_once('.') else {
        return CallReply::Err("malformed tool name".into());
    };
    let hosted = match registry.get(ext_id) {
        Some(Target::Local(h)) if h.tools.iter().any(|t| t == tool) => h,
        // We are the serving node but don't host this tool locally — refuse rather than re-route
        // (no routing loops). A remote/absent entry here is a misroute.
        _ => return CallReply::Err("tool not hosted on the serving node".into()),
    };

    let mut instance = hosted.instance.lock().await;
    match instance.call_tool(tool, &req.input).await {
        Ok(output) => CallReply::Ok(output),
        Err(RuntimeError::Tool(m)) => CallReply::Err(m),
        Err(other) => CallReply::Err(other.to_string()),
    }
}
