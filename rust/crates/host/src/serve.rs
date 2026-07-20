//! Serve this node's local tools to remote callers — the host side of the routed MCP call
//! (mcp scope, README §6.5; routed-node-dispatch #81). For each extension hosted locally, declare
//! bus **queryables** and answer each routed [`CallRequest`] by running local dispatch
//! (`lb_mcp::serve_call`), replying with the [`CallReply`].
//!
//! Symmetric nodes (§3.1): every node *can* serve — this is not a "cloud-only" path. A solo
//! node simply has no remote callers.
//!
//! ## Two keys, two different walls
//!
//! - **`ws/{ws}/mcp/{ext}/{node}/call` — the node-qualified key, declared PER WORKSPACE.** This is
//!   the key that carries calls. Declaring it per workspace (rather than `ws/*`) makes the
//!   workspace wall a **key-space** wall on this path: a ws-B caller's query physically cannot
//!   reach a node that serves only ws-A, because that node declared nothing matching ws-B's key.
//!   (scope, open question 6 — chosen over the cheaper `ws/*` wildcard + downstream refusal,
//!   because this scope's whole argument is preferring true-by-construction over
//!   true-by-assumption; keeping the wildcard here would contradict the reasoning used to reject
//!   the payload-field design. Cost: N tokens for a hub serving N workspaces, mirroring what
//!   fleet-presence already announces per workspace.)
//!
//! - **`ws/*/mcp/{ext}/call` — the legacy shared key, workspace-WILDCARDED as before.** Every node
//!   hosting `{ext}` declares this one, so it is a fan-in: with a fleet, all of them answer. It no
//!   longer carries calls (`lb_mcp` dispatches on the node key) and survives only so a
//!   mixed-version caller predating #81 still works. Its wall is weaker and unchanged: the call
//!   *is* observed, and the answer loop resolves only the arriving workspace's instance.
//!
//! Note the asymmetry is deliberate and documented rather than tidied away — the strong claim
//! ("ws-A's node never observes a ws-B call") is true of the node key only, which is why
//! `serve_ext` now needs to be told which workspaces it serves.

use std::sync::Arc;

use lb_bus::{declare_queryable, Bus, BusError, NodeId, Responder};
use lb_mcp::{call_key, node_call_key, serve_call, CallReply, CallRequest, Registry};

/// A live tool-serving registration. Holds the queryable tasks; drop it to stop serving. Kept
/// alive by the wiring layer (the `node` binary / a role layer) for the node's lifetime.
pub struct ToolServer {
    _tasks: Vec<tokio::task::JoinHandle<()>>,
}

/// Begin serving extension `ext`'s tools to remote callers over `bus`, as node `node`, for each
/// workspace in `workspaces`. Spawns tasks that answer routed calls against the shared `registry`
/// until the returned [`ToolServer`] drops.
///
/// `registry` is an `Arc<Registry>` — the SAME registry the local call path reads (its
/// instances are already `Arc<Mutex<…>>`), so a tool answers identically whether called locally
/// or routed in.
///
/// `workspaces` are the workspaces this node serves `ext` for. One node-qualified queryable is
/// declared per workspace (see the module doc — that per-workspace declaration IS the workspace
/// wall on this path). Passing an empty slice serves no routed calls on the node key, which is a
/// legitimate posture for a node that hosts an ext purely locally.
pub async fn serve_ext(
    bus: &Bus,
    registry: Arc<Registry>,
    ext: &str,
    node: &NodeId,
    workspaces: &[&str],
) -> Result<ToolServer, BusError> {
    let mut tasks = Vec::with_capacity(workspaces.len() + 1);

    // The node-qualified key, per workspace served — the route, and the key-space wall.
    for ws in workspaces {
        let responder = declare_queryable(bus, ws, &node_call_key(ext, node)).await?;
        tasks.push(tokio::spawn(answer_loop(responder, registry.clone())));
    }

    // The legacy shared key, ws-wildcarded exactly as before. Declared so a pre-#81 caller still
    // reaches this node; `lb_mcp` itself no longer dispatches here.
    let shared = declare_queryable(bus, "*", &call_key(ext)).await?;
    tasks.push(tokio::spawn(answer_loop(shared, registry)));

    Ok(ToolServer { _tasks: tasks })
}

/// The answer loop: await each routed request, run local dispatch, reply. The request's workspace is
/// recovered from the concrete key it arrived on (`ws/{ws}/…`) — a queryable may be ws-wildcarded
/// but each `get` targets a concrete ws — and threaded into `serve_call` so a Tier-agnostic target
/// (a native sidecar keyed `(ws, ext_id)`) resolves THIS workspace's child.
async fn answer_loop(responder: Responder, registry: Arc<Registry>) {
    while let Some(incoming) = responder.recv().await {
        let ws = incoming.ws();
        let reply = match (
            ws,
            serde_json::from_slice::<CallRequest>(&incoming.payload()),
        ) {
            (Some(ws), Ok(req)) => serve_call(&registry, &ws, &req).await,
            (None, _) => CallReply::Err("routed request missing workspace in key".into()),
            (_, Err(e)) => CallReply::Err(format!("malformed routed request: {e}")),
        };
        let bytes = serde_json::to_vec(&reply).unwrap_or_default();
        // Best-effort reply; if the caller went away, it observes a timeout — its concern.
        let _ = incoming.reply(&bytes).await;
    }
}
