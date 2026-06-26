//! The edge side of a routed agent invocation — authorize locally, then `query` the hub's agent
//! queryable (agent scope, reusing the S3 routing seam). The call site mirrors a routed tool call:
//! `caps::check` runs HERE first (workspace-first, `mcp:agent.invoke:call`), so an unauthorized or
//! cross-workspace invocation never leaves the edge.
//!
//! The edge carries its caller identity + grant + ws in the request so the hub can run the loop
//! under `agent ∩ caller` (route.rs, the S5 co-trust path). Isolation is structural: the request is
//! emitted on `ws/{caller.ws}/agent/invoke`, so it can only ever reach the agent serving that ws.

use lb_auth::Principal;
use lb_bus::{query, Bus};

use super::authorize::authorize_invoke;
use super::error::AgentError;
use super::model_access::AllowedTool;
use super::route::{agent_call_key, AgentInvokeReply, AgentInvokeRequest};

/// Invoke the (remote, hub-hosted) agent from this edge node. Authorizes on the edge, routes over
/// the bus, and returns the agent's final answer. `None`-style failures (no hub answered) surface
/// as [`AgentError::NotFound`]; a hub-side error surfaces as [`AgentError::BadInput`] carrying the
/// message (the agent's own gate denials already returned before any routing).
#[allow(clippy::too_many_arguments)]
pub async fn invoke_remote(
    bus: &Bus,
    caller: &Principal,
    ws: &str,
    job_id: &str,
    goal: &str,
    skill: Option<&str>,
    doc: Option<&str>,
    tools: &[AllowedTool],
    ts: u64,
) -> Result<String, AgentError> {
    // Gate 1 on the EDGE: workspace-first, then mcp:agent.invoke:call. An ungranted or cross-ws
    // invocation is refused here — it never routes to the hub.
    authorize_invoke(caller, ws)?;

    let req = AgentInvokeRequest {
        caller_sub: caller.sub().to_string(),
        caller_ws: ws.to_string(),
        caller_caps: caller.caps().to_vec(),
        job_id: job_id.to_string(),
        goal: goal.to_string(),
        skill: skill.map(|s| s.to_string()),
        doc: doc.map(|s| s.to_string()),
        tools: tools
            .iter()
            .map(|t| (t.name.clone(), t.description.clone()))
            .collect(),
        ts,
    };
    let payload = serde_json::to_vec(&req).map_err(|e| AgentError::BadInput(e.to_string()))?;

    match query(bus, ws, &agent_call_key(), &payload).await {
        Ok(Some(bytes)) => match serde_json::from_slice::<AgentInvokeReply>(&bytes) {
            Ok(AgentInvokeReply::Ok(answer)) => Ok(answer),
            Ok(AgentInvokeReply::Err(msg)) => Err(AgentError::BadInput(msg)),
            Err(e) => Err(AgentError::BadInput(format!("malformed agent reply: {e}"))),
        },
        // No hub answered (offline / not serving): the caller decides what that means.
        Ok(None) => Err(AgentError::NotFound),
        Err(e) => Err(AgentError::BadInput(format!("agent routing failed: {e}"))),
    }
}
