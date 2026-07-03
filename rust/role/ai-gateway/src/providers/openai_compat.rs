//! The one OpenAI-compatible adapter — a real [`Provider`] that speaks the OpenAI
//! **chat-completions** wire shape (active-agent-wiring scope, Slice 1). It covers OpenAI itself,
//! Z.AI `zaicoding`, and any endpoint that speaks the same shape: point it at a `base_url` and it
//! is that backend. There is exactly one such adapter — the differences are all `base_url` + key +
//! model, never code branches (§1 symmetric: no `if provider { … }`).
//!
//! **Model access only.** Like every provider it answers one turn: request in, one [`AiResponse`]
//! out (with any proposed tool calls carried, not run). It holds no loop and never touches the
//! store — the agent owns those (agent scope).
//!
//! **Honest-failure contract (load-bearing — scope Risks).** A model call can fail: the network is
//! down, the endpoint returns non-2xx, the body is garbage. This adapter **never** returns a silent
//! empty answer and **never** panics. On any such fault it returns a *terminal* [`AiResponse::stop`]
//! whose content is an attributed error — `"model call failed: openai-compat/{model}: {detail}"` —
//! with zero tokens. The gateway/agent loop treats a `stop` as terminal, so the fault surfaces as
//! the turn's answer (attributed to the model) rather than looking like a real, empty completion.
//!
//! **Secrets.** The API key arrives already resolved (the caller pulls it from secrets) and is only
//! ever placed in the `Authorization` header — it is never logged, never put in the error string.

use serde_json::{json, Value};

use crate::provider::Provider;
use crate::request::AiRequest;
use crate::response::{AiResponse, ToolCall};

/// The public OpenAI base URL — used when the caller passes no `base_url` override.
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// An OpenAI-compatible chat-completions provider. Cheap to construct; holds one `reqwest::Client`
/// so connections are pooled across turns. `Send + Sync` (the [`Provider`] requirement) so the
/// gateway holding it can be shared across the node's tasks.
pub struct OpenAiCompat {
    api_key: String,
    model: String,
    /// `{base_url}/chat/completions` is the endpoint; `base_url` defaults to OpenAI's.
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiCompat {
    /// Build an adapter for `model`, authenticating with `api_key`. `base_url` selects the backend
    /// (`None` → the public OpenAI API); it is the `{base_url}/v1`-style prefix, e.g.
    /// `https://api.z.ai/api/paas/v4` for Z.AI. The endpoint hit is `{base_url}/chat/completions`.
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            client: reqwest::Client::new(),
        }
    }

    /// The chat-completions endpoint for this backend.
    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    /// Build the request body: `model`, the conversation `messages` (with `prior_results` folded in
    /// as `role:"tool"` messages so a multi-turn loop feeds outcomes back), and — only when
    /// non-empty — the `tools` array in OpenAI's function shape.
    fn body(&self, req: &AiRequest) -> Value {
        let mut messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();

        // Fold prior tool outcomes back in as `role:"tool"` messages keyed by the call id, so the
        // model sees what its proposed calls returned on the next turn (the loop's "back" edge).
        for r in &req.prior_results {
            let content = r.ok.clone().or_else(|| r.error.clone()).unwrap_or_default();
            messages.push(json!({
                "role": "tool",
                "tool_call_id": r.id,
                "content": content,
            }));
        }

        let mut body = json!({ "model": self.model, "messages": messages });

        if !req.tools.is_empty() {
            let tools: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": { "type": "object" },
                        },
                    })
                })
                .collect();
            body["tools"] = Value::Array(tools);
        }

        body
    }

    /// The attributed terminal failure — one place so the message shape is exact (scope Risks). The
    /// key is never in `detail`.
    fn failed(&self, detail: impl std::fmt::Display) -> AiResponse {
        AiResponse::stop(
            format!("model call failed: openai-compat/{}: {detail}", self.model),
            0,
        )
    }
}

impl Provider for OpenAiCompat {
    async fn complete(&self, req: &AiRequest) -> AiResponse {
        let resp = match self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .header("content-type", "application/json")
            .json(&self.body(req))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => return self.failed(e),
        };

        // Non-2xx is a fault, not a completion — attribute the status rather than trusting the body.
        let status = resp.status();
        if !status.is_success() {
            return self.failed(format!("http {}", status.as_u16()));
        }

        let value: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => return self.failed(format!("unparseable body: {e}")),
        };

        parse_completion(&value).unwrap_or_else(|| self.failed("no choices in response"))
    }
}

/// Read one turn out of a chat-completions response. `None` when the shape is missing the pieces a
/// turn needs (no `choices[0]`), which the caller attributes as a failure.
fn parse_completion(value: &Value) -> Option<AiResponse> {
    let choice = value.get("choices")?.as_array()?.first()?;
    let message = choice.get("message")?;

    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let tokens = value
        .get("usage")
        .and_then(|u| u.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;

    let tool_calls = parse_tool_calls(message);

    let finish_is_tools = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(|r| r == "tool_calls")
        .unwrap_or(false);

    if finish_is_tools || !tool_calls.is_empty() {
        Some(AiResponse::calls(content, tool_calls, tokens))
    } else {
        Some(AiResponse::stop(content, tokens))
    }
}

/// Map `message.tool_calls[]` (`{ id, function: { name, arguments } }`) to our [`ToolCall`]s. The
/// `arguments` are a JSON *string* on the wire — carried through verbatim as [`ToolCall::input`].
fn parse_tool_calls(message: &Value) -> Vec<ToolCall> {
    let Some(calls) = message.get("tool_calls").and_then(Value::as_array) else {
        return Vec::new();
    };
    calls
        .iter()
        .filter_map(|c| {
            let function = c.get("function")?;
            Some(ToolCall {
                id: c
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                name: function
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                input: function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}
