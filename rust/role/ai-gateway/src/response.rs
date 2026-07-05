//! The `AiResponse` — the gateway's reply: content and/or proposed tool calls, plus the finish
//! reason and usage (ai-gateway scope). The gateway **carries** tool calls but does not execute
//! them — the agent (the caller) runs the loop, capability-checking each call (agent scope).

use serde::{Deserialize, Serialize};

/// A tool call the model proposes. `name` is the qualified MCP name (`<ext>.<tool>`); `input` is
/// the JSON argument string the agent passes to `lb_mcp::call`. `id` correlates the call with its
/// [`ToolResult`] when fed back next turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: String,
}

/// The outcome of running a [`ToolCall`], fed back to the gateway in the next request. `ok` is the
/// JSON output on success; `error` is a message (e.g. a capability denial) — a denied tool call is
/// **not** a crash: the result is fed back so the model can react (agent scope deny path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub id: String,
    /// The originally-proposed call's tool name + arguments JSON. An OpenAI-compat backend expects
    /// a `role:"tool"` result to answer an assistant message carrying the matching `tool_calls`
    /// entry; the adapter reconstructs that echo from these (an orphan result is half-ignored —
    /// live GLM retried the identical rejected call three turns running). Empty on a legacy caller
    /// — the adapter then degrades to the old orphan shape rather than fabricating a call.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub ok: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl ToolResult {
    pub fn ok(id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            input: String::new(),
            ok: Some(output.into()),
            error: None,
        }
    }
    pub fn err(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            input: String::new(),
            ok: None,
            error: Some(message.into()),
        }
    }
}

/// Why the model stopped this turn. `ToolCalls` means the agent must run them and call again;
/// `Stop` means the loop is done (no more tool calls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
}

/// The gateway's reply for one turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiResponse {
    /// The model's text content this turn (may be empty when it only proposes tool calls).
    pub content: String,
    /// Tool calls the agent must run, capability-checked, before the next turn.
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
    /// Token usage (metering/audit). Mock returns a fixed estimate; a real adapter reports actuals.
    pub tokens: u32,
}

impl AiResponse {
    /// A terminal text reply — no tool calls, the loop ends.
    pub fn stop(content: impl Into<String>, tokens: u32) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            finish_reason: FinishReason::Stop,
            tokens,
        }
    }

    /// A reply proposing tool calls — the agent runs them and calls again.
    pub fn calls(content: impl Into<String>, tool_calls: Vec<ToolCall>, tokens: u32) -> Self {
        Self {
            content: content.into(),
            tool_calls,
            finish_reason: FinishReason::ToolCalls,
            tokens,
        }
    }
}
