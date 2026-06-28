//! The `ModelAccess` seam — the host-owned abstraction of "one model turn", so the host's agent
//! service does **not** build-depend on the AI-gateway *role* crate (roles depend on host, never
//! the reverse — symmetric layering). The role crate provides a blanket impl adapting its
//! `AiGateway` to this trait; the agent loop calls only this.
//!
//! It mirrors the gateway contract at the host's altitude: messages + allowed tools go in, a turn
//! (content + proposed tool calls + done flag) comes back. The gateway does model access only; the
//! **loop** that runs the proposed calls and feeds results back lives in `run.rs` (agent scope).

use std::future::Future;

/// A tool the model is allowed to propose this turn, by qualified MCP name (`<ext>.<tool>`).
#[derive(Debug, Clone)]
pub struct AllowedTool {
    pub name: String,
    pub description: String,
}

/// A tool call the model proposed — the agent must run it (capability-checked) before the next turn.
#[derive(Debug, Clone)]
pub struct ProposedCall {
    pub id: String,
    pub name: String,
    pub input: String,
}

/// The outcome of running a [`ProposedCall`], fed back to the model next turn. A denied call is an
/// `Err` outcome, NOT a crash — the model is told and can react (agent scope deny path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallOutcome {
    pub id: String,
    pub ok: Option<String>,
    pub error: Option<String>,
}

/// One model turn's result.
#[derive(Debug, Clone)]
pub struct Turn {
    /// The model's text this turn.
    pub content: String,
    /// Tool calls to run before the next turn. Empty + `done` means the loop ends.
    pub calls: Vec<ProposedCall>,
    /// True when the model returned no more tool calls (finish reason stop).
    pub done: bool,
}

/// One model turn over the conversation. The host passes the running messages, the allowed tools,
/// the outcomes of the previous turn's calls, and an **idempotency key** (so a resumed turn is
/// replay-safe — the gateway caches by it, agent scope offline/sync).
pub trait ModelAccess {
    fn turn(
        &self,
        ws: &str,
        messages: &[(String, String)],
        tools: &[AllowedTool],
        prior: &[CallOutcome],
        idempotency_key: &str,
    ) -> impl Future<Output = Turn> + Send;
}
