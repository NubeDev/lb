//! The cross-node wire for invoking the agent — the bus key a routed invocation rides on, and the
//! request/reply envelope (mirrors `mcp/src/route.rs`). The edge `query`s; the hub's agent
//! queryable answers (`serve.rs`).
//!
//! - bus key (workspace-relative): `agent/invoke` → workspace-prefixed by `lb_bus` to
//!   `ws/{id}/agent/invoke`. Only the hub hosting the agent declares a queryable here, and the
//!   `ws/{id}/` prefix means an invocation authorized for workspace B can never reach a queryable
//!   serving workspace A (the workspace wall on the routed path, §7).
//!
//! **Caller identity on the wire (S5).** Unlike a routed *tool* call (where the loop is the serving
//! node's and `caps::check` ran on the caller), the agent's loop runs on the HUB and must re-check
//! each tool call under the caller's grant. So the request carries the caller's `sub`/`caps`: the
//! hub reconstructs the principal and bounds the loop to `agent ∩ caller`. The workspace-scoped key
//! still enforces isolation (a ws-B caller can only emit on `ws/B/...`). Signing the carried caps
//! (token-on-the-bus) is the mcp-scope "serve-side authorization" open question — recorded, not
//! built at S5 (the edge and hub are co-trusted in-process here).

use serde::{Deserialize, Serialize};

/// The workspace-relative bus key for routing an agent invocation. Only the hub hosting the agent
/// declares a queryable here, so the invocation lands on exactly one node.
pub fn agent_call_key() -> String {
    "agent/invoke".to_string()
}

/// A routed agent invocation: the caller's identity + grant, the goal, optional substrate refs, the
/// allowed tools, and the durable job id. Serialized as bytes on the bus.
#[derive(Serialize, Deserialize)]
pub struct AgentInvokeRequest {
    /// The caller's global identity (`user:…`) — recorded for audit, carried to derive the actor.
    pub caller_sub: String,
    /// The caller's workspace — set by the edge to its own ws (the same ws the bus key is scoped
    /// to). The hub runs the loop in this workspace; the workspace-scoped key already guarantees a
    /// ws-B caller can only reach `ws/B/...`.
    pub caller_ws: String,
    /// The caller's held capabilities — the upper bound of the agent's effective grant (the
    /// intersection). The hub reconstructs the caller principal from these.
    pub caller_caps: Vec<String>,
    pub job_id: String,
    pub goal: String,
    pub skill: Option<String>,
    pub doc: Option<String>,
    /// The **runtime selector** (runtime-seam #1). Resolved against the serving node's registry:
    /// `None`/absent → the in-house default; a known profile id (`"open-interpreter-default"`, …) →
    /// that external runtime; an unknown named id → error. `#[serde(default)]` so an older edge that
    /// omits the field decodes as `None` (default runtime) — the wire stays backward-compatible.
    #[serde(default)]
    pub runtime: Option<String>,
    /// Optional **page context** (agent-dock scope) — the client-reported `{ surface, path, search }`
    /// object the run fences into its goal as untrusted context. `#[serde(default)]` so an older edge
    /// that omits it decodes as `None` (behavior byte-identical to today). Opaque `Value`: the host
    /// never branches on a surface id (rule 10), it only serializes + fences it.
    #[serde(default)]
    pub context: Option<serde_json::Value>,
    /// Allowed tools, as `(name, description)` pairs (the host `AllowedTool` shape on the wire).
    pub tools: Vec<(String, String)>,
    pub ts: u64,
}

/// The routed invocation reply: the agent's final answer, or an error message.
#[derive(Serialize, Deserialize)]
pub enum AgentInvokeReply {
    Ok(String),
    Err(String),
}
