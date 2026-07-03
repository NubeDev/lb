//! Serve the central agent to remote (edge) callers — the hub side of the routed invocation
//! (agent scope, reusing the S3 routing seam in `host/serve.rs`). The hub declares a bus
//! **queryable** on `ws/*/agent/invoke` and answers each [`AgentInvokeRequest`] by running the
//! agent loop locally, replying with the [`AgentInvokeReply`].
//!
//! Symmetric nodes (§3.1): any node *can* host the agent — this is not a cloud-only path; a solo
//! node simply has no remote callers. The queryable key is workspace-wildcarded so one declaration
//! serves every workspace, but each request only ever arrives on the key the *calling* node
//! emitted — and the edge emits `ws/{caller.ws}/...`, authorized workspace-first on its side. So an
//! edge in workspace B can never produce a request on `ws/A/...`; the workspace wall holds (§7).
//!
//! The model lives on the hub (the role layer supplies it); the loop runs under the caller's grant
//! reconstructed from the routed request (`Principal::routed` — the S5 co-trust path, route.rs).

use std::sync::Arc;

use lb_auth::Principal;
use lb_bus::{declare_queryable, BusError, Responder};

use super::dispatch::invoke_via_runtime;
use super::model_access::AllowedTool;
use super::registry::RuntimeRegistry;
use super::route::{agent_call_key, AgentInvokeReply, AgentInvokeRequest};
use crate::boot::Node;

/// A live agent-serving registration. Holds the queryable task; drop it to stop serving. Kept alive
/// by the wiring layer (the role/`node` binary) for the node's lifetime.
pub struct AgentServer {
    _task: tokio::task::JoinHandle<()>,
}

/// Begin serving the agent on `node` to remote callers, resolving each invocation's `runtime` against
/// `registry` and using `agent_caps` as the agent actor's own capabilities. Spawns a task answering
/// routed invocations until the returned [`AgentServer`] drops.
///
/// **The registry carries the runtime seam (#1).** It always holds the in-house `default` (built by
/// the wiring layer over its `ModelAccess`); a node built with the `external-agent` feature also
/// registers the external `AcpRuntime` entries. So whether a routed `agent.invoke { runtime }`
/// reaches an external agent is the registry's *contents* (feature + config), never a branch here —
/// this loop dispatches through the trait object identically for every runtime.
///
/// `node` and `registry` are shared (`Arc`) into the task.
pub async fn serve_agent(
    node: Arc<Node>,
    registry: Arc<RuntimeRegistry>,
    agent_caps: Vec<String>,
) -> Result<AgentServer, BusError> {
    let responder = declare_queryable(&node.bus, "*", &agent_call_key()).await?;
    let task = tokio::spawn(answer_loop(responder, node, registry, agent_caps));
    Ok(AgentServer { _task: task })
}

/// The answer loop: await each routed invocation, run the resolved runtime, reply.
async fn answer_loop(
    responder: Responder,
    node: Arc<Node>,
    registry: Arc<RuntimeRegistry>,
    agent_caps: Vec<String>,
) {
    while let Some(incoming) = responder.recv().await {
        let reply = match serde_json::from_slice::<AgentInvokeRequest>(&incoming.payload()) {
            Ok(req) => run_one(&node, &registry, &agent_caps, req).await,
            Err(e) => AgentInvokeReply::Err(format!("malformed agent request: {e}")),
        };
        let bytes = serde_json::to_vec(&reply).unwrap_or_default();
        let _ = incoming.reply(&bytes).await;
    }
}

/// Run one routed invocation: reconstruct the caller, dispatch through the resolved runtime, map the
/// outcome to a reply. The `runtime` field selects the runtime (absent → default; unknown → error).
async fn run_one(
    node: &Arc<Node>,
    registry: &RuntimeRegistry,
    agent_caps: &[String],
    req: AgentInvokeRequest,
) -> AgentInvokeReply {
    // Reconstruct the caller from the routed request (S5 co-trust path; route.rs). The workspace is
    // the caller's own — the queryable key it arrived on was `ws/{caller.ws}/...`.
    let ws = caller_ws(&req);
    let caller = Principal::routed(&req.caller_sub, &ws, req.caller_caps.clone());

    let tools: Vec<AllowedTool> = req
        .tools
        .iter()
        .map(|(name, description)| AllowedTool {
            name: name.clone(),
            description: description.clone(),
        })
        .collect();

    let substrate = super::dispatch::Substrate {
        skill: req.skill.as_deref(),
        doc: req.doc.as_deref(),
    };

    match invoke_via_runtime(
        node,
        registry,
        req.runtime.as_deref(),
        &caller,
        agent_caps,
        &ws,
        &req.job_id,
        &req.goal,
        substrate,
        &tools,
        req.ts,
    )
    .await
    {
        Ok(answer) => AgentInvokeReply::Ok(answer),
        Err(e) => AgentInvokeReply::Err(e.to_string()),
    }
}

/// The caller's workspace is implied by the bus key the request arrived on. The edge `query` helper
/// passes `ws` explicitly there; on this side we read it from the request's caller (which the edge
/// set to its own ws). Kept as a one-liner seam so a key-derived ws can replace it later.
fn caller_ws(req: &AgentInvokeRequest) -> String {
    req.caller_ws.clone()
}
