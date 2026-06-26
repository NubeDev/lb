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

use super::invoke::{invoke, Invocation};
use super::model_access::{AllowedTool, ModelAccess};
use super::route::{agent_call_key, AgentInvokeReply, AgentInvokeRequest};
use crate::boot::Node;

/// A live agent-serving registration. Holds the queryable task; drop it to stop serving. Kept alive
/// by the wiring layer (the role/`node` binary) for the node's lifetime.
pub struct AgentServer {
    _task: tokio::task::JoinHandle<()>,
}

/// Begin serving the agent on `node` to remote callers, using `model` for model access and
/// `agent_caps` as the agent actor's own capabilities. Spawns a task answering routed invocations
/// until the returned [`AgentServer`] drops.
///
/// `node` and `model` are shared (`Arc`) into the task. `M: 'static` so the task can own its clone.
pub async fn serve_agent<M: ModelAccess + Send + Sync + 'static>(
    node: Arc<Node>,
    model: Arc<M>,
    agent_caps: Vec<String>,
) -> Result<AgentServer, BusError> {
    let responder = declare_queryable(&node.bus, "*", &agent_call_key()).await?;
    let task = tokio::spawn(answer_loop(responder, node, model, agent_caps));
    Ok(AgentServer { _task: task })
}

/// The answer loop: await each routed invocation, run the agent loop, reply.
async fn answer_loop<M: ModelAccess + Send + Sync + 'static>(
    responder: Responder,
    node: Arc<Node>,
    model: Arc<M>,
    agent_caps: Vec<String>,
) {
    while let Some(incoming) = responder.recv().await {
        let reply = match serde_json::from_slice::<AgentInvokeRequest>(&incoming.payload()) {
            Ok(req) => run_one(&node, model.as_ref(), &agent_caps, req).await,
            Err(e) => AgentInvokeReply::Err(format!("malformed agent request: {e}")),
        };
        let bytes = serde_json::to_vec(&reply).unwrap_or_default();
        let _ = incoming.reply(&bytes).await;
    }
}

/// Run one routed invocation: reconstruct the caller, run `invoke`, map the outcome to a reply.
async fn run_one<M: ModelAccess>(
    node: &Node,
    model: &M,
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

    let inv = Invocation {
        job_id: &req.job_id,
        goal: &req.goal,
        skill: req.skill.as_deref(),
        doc: req.doc.as_deref(),
        tools: &tools,
        ts: req.ts,
    };

    match invoke(node, model, &caller, agent_caps, &ws, inv).await {
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
