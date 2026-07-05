//! The `AiRequest` — the stable model-access input the agent (the caller) sends the gateway
//! (ai-gateway scope, "the stable internal contract"). The gateway does model access only; the
//! request carries everything a provider adapter needs, but **not** the loop — proposed tool calls
//! come back in the [`AiResponse`](crate::AiResponse) and the *agent* runs them, then sends results
//! back in the next request's `prior_results` (agent scope).
//!
//! S5 ships the fields the mock + the loop need; the rest of the contract (retention mode, budget
//! ceiling, local-only flag, model class) are named in the scope and slot in behind the same shape.

use serde::{Deserialize, Serialize};

use crate::response::ToolResult;

/// One message in the running conversation. `role` is `system` | `user` | `assistant` | `tool`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

/// A tool the model is allowed to propose, by qualified MCP name (`<ext>.<tool>`). The gateway
/// passes the schema through to the provider; the *agent* decides whether a proposed call is
/// actually permitted (caps::check) — the gateway never executes a tool (ai-gateway scope).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    /// The tool's input JSON Schema (`{type:"object", properties, required}`); `None` when the tool
    /// declares none. Passed through to the provider's function `parameters` so the model can form a
    /// valid call — without it every tool looks argument-less.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// A model-access request. Stateless from the gateway's view except for the idempotency cache:
/// two requests with the same `idempotency_key` return the same response, so a resumed agent job
/// does not re-spend budget or diverge (ai-gateway scope, agent scope offline/sync).
// No `Eq`: `ToolSchema.parameters` is a `serde_json::Value` (which is `PartialEq` but not `Eq`).
// Equality is only used in tests as `assert_eq!`, which needs `PartialEq`, not `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRequest {
    /// The workspace this call is scoped to — carried for audit + policy (the hard wall, §7).
    pub ws: String,
    /// The running conversation so far.
    pub messages: Vec<Message>,
    /// The tools the model may propose this turn.
    pub tools: Vec<ToolSchema>,
    /// Results of tool calls proposed in the previous turn, fed back in (the loop's "back" edge).
    #[serde(default)]
    pub prior_results: Vec<ToolResult>,
    /// Pins non-determinism: same key → same cached response (replay-safe resume).
    pub idempotency_key: String,
}

impl AiRequest {
    /// Build a request for `ws` with an idempotency key. Messages/tools/results are added by the
    /// agent as it drives the loop.
    pub fn new(ws: impl Into<String>, idempotency_key: impl Into<String>) -> Self {
        Self {
            ws: ws.into(),
            messages: Vec::new(),
            tools: Vec::new(),
            prior_results: Vec::new(),
            idempotency_key: idempotency_key.into(),
        }
    }
}
