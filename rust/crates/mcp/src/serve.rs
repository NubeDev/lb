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

/// Run `req` against this node's local registry (in workspace `ws`) and produce the reply to send
/// back. A request for a tool this node does not host locally (or any extension error) maps to
/// `CallReply::Err` — the calling node surfaces it as a `ToolError::Extension`.
///
/// `ws` is the workspace the request arrived on (recovered from the routed key). It is threaded into
/// the dispatch target so a Tier-agnostic local target can resolve per-workspace state — a native
/// sidecar keyed `(ws, ext_id)` reaches THIS workspace's child, never another's (the workspace wall
/// stays structural on the routed native path exactly as on the wasm path). A wasm instance ignores
/// it. No routed call re-authorizes here: the calling node ran the gate workspace-first, and the
/// queryable key means a ws-B call physically cannot reach a ws-A target (§7).
pub async fn serve_call(registry: &Registry, ws: &str, req: &CallRequest) -> CallReply {
    let Some((ext_id, tool)) = req.tool.split_once('.') else {
        return CallReply::Err("malformed tool name".into());
    };
    // Only a LOCAL target may answer a routed call. We are the serving node: if this tool is not
    // hosted here, refuse rather than re-route (no routing loops) — a remote/absent entry means the
    // call was misrouted, and forwarding it would let a call bounce between nodes. This holds for
    // node-qualified keys too: a call that reaches the wrong node is refused, never relayed on.
    let local = registry
        .targets(ext_id)
        .into_iter()
        .find_map(|t| match t {
            Target::Local(h) if h.tools.iter().any(|d| d.name == tool) => Some(h),
            _ => None,
        });
    let Some(hosted) = local else {
        return CallReply::Err("tool not hosted on the serving node".into());
    };

    let mut instance = hosted.instance.lock().await;
    // No host-callback context on the routed path: the guest (if any) runs on THIS node and a
    // cross-node callback identity is a separate scope. A native target ignores `ctx` regardless.
    match instance.call_tool(ws, tool, &req.input, None).await {
        Ok(output) => CallReply::Ok(output),
        Err(RuntimeError::Tool(m)) => CallReply::Err(m),
        Err(other) => CallReply::Err(other.to_string()),
    }
}
