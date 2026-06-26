//! Serve this node's local tools to remote callers — the host side of the routed MCP call
//! (mcp scope, README §6.5). For each extension hosted locally, declare a bus **queryable** on
//! `ws/*/mcp/{ext}/call` and answer each routed [`CallRequest`] by running local dispatch
//! (`lb_mcp::serve_call`), replying with the [`CallReply`].
//!
//! Symmetric nodes (§3.1): every node *can* serve — this is not a "cloud-only" path. A solo
//! node simply has no remote callers. The queryable key is workspace-wildcarded so one
//! declaration serves every workspace, but each request only ever arrives on the key the
//! *calling* node emitted — and the calling node emits `ws/{principal.ws}/…`, authorized
//! workspace-first. So a node-B caller can never produce a request on `ws/A/…`; the workspace
//! wall holds on the routed path exactly as on pub/sub (§7).

use std::sync::Arc;

use lb_bus::{declare_queryable, Bus, BusError, Responder};
use lb_mcp::{call_key, serve_call, CallReply, CallRequest, Registry};

/// A live tool-serving registration. Holds the queryable task; drop it to stop serving. Kept
/// alive by the wiring layer (the `node` binary / a role layer) for the node's lifetime.
pub struct ToolServer {
    _task: tokio::task::JoinHandle<()>,
}

/// Begin serving extension `ext`'s tools to remote callers over `bus`. Spawns a task that
/// answers routed calls against the shared `registry` until the returned [`ToolServer`] drops.
///
/// `registry` is an `Arc<Registry>` — the SAME registry the local call path reads (its
/// instances are already `Arc<Mutex<…>>`), so a tool answers identically whether called locally
/// or routed in.
pub async fn serve_ext(
    bus: &Bus,
    registry: Arc<Registry>,
    ext: &str,
) -> Result<ToolServer, BusError> {
    // Wildcard the workspace: one queryable serves every workspace's calls for this ext. `*` is
    // a single segment, matching exactly the `{id}` in `ws/{id}/mcp/{ext}/call`.
    let responder = declare_queryable(bus, "*", &call_key(ext)).await?;
    let task = tokio::spawn(answer_loop(responder, registry));
    Ok(ToolServer { _task: task })
}

/// The answer loop: await each routed request, run local dispatch, reply.
async fn answer_loop(responder: Responder, registry: Arc<Registry>) {
    while let Some(incoming) = responder.recv().await {
        let reply = match serde_json::from_slice::<CallRequest>(&incoming.payload()) {
            Ok(req) => serve_call(&registry, &req).await,
            Err(e) => CallReply::Err(format!("malformed routed request: {e}")),
        };
        let bytes = serde_json::to_vec(&reply).unwrap_or_default();
        // Best-effort reply; if the caller went away, it observes a timeout — its concern.
        let _ = incoming.reply(&bytes).await;
    }
}
