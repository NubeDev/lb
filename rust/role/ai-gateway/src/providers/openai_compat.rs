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
//! **Honest-failure contract (load-bearing — scope Risks, upgraded by agent-loop-hardening slice
//! D).** A model call can fail: the network is down, the endpoint returns non-2xx, the body is
//! garbage. This adapter **never** returns a silent empty answer and **never** panics. On any such
//! fault it returns a typed [`ProviderFault`] carrying the *structured* evidence — status code,
//! `Retry-After` delta-seconds, the overflow discriminant (`error.code ==
//! "context_length_exceeded"` or a 413) — so the loop can retry a transient, compact on overflow,
//! and surface a fatal honestly. It never classifies by parsing an error-message string.
//!
//! **Secrets.** The API key arrives already resolved (the caller pulls it from secrets) and is only
//! ever placed in the `Authorization` header — it is never logged, never put in the fault detail.

use serde_json::{json, Value};

use crate::fault::ProviderFault;
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
        // History `role:"tool"` messages (the host loop's per-turn outcome summaries) are re-rolled
        // as plain user text: on this wire a `tool` message is only valid answering an assistant
        // `tool_calls` message, and an orphan one is half-ignored — live GLM lost the thread and
        // guessed datasource names. The summaries are keyed text; user role carries them faithfully.
        let mut messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| {
                if m.role == "tool" {
                    json!({ "role": "user", "content": format!("[tool results]\n{}", m.content) })
                } else {
                    json!({ "role": m.role, "content": m.content })
                }
            })
            .collect();

        // Fold prior tool outcomes back in the CONFORMANT shape: one assistant message echoing the
        // originally-proposed `tool_calls`, then a `role:"tool"` result keyed to each id — the shape
        // the model was trained on (measured live: with the echo GLM kept the call context; without
        // it, identical blind retries). A result without a `name` (a legacy caller) has no call to
        // echo — it degrades to the old orphan message rather than fabricating one.
        let echoes: Vec<Value> = req
            .prior_results
            .iter()
            .filter(|r| !r.name.is_empty())
            .map(|r| {
                json!({
                    "id": r.id,
                    "type": "function",
                    "function": { "name": r.name, "arguments": r.input },
                })
            })
            .collect();
        if !echoes.is_empty() {
            messages.push(json!({ "role": "assistant", "content": "", "tool_calls": echoes }));
        }
        for r in &req.prior_results {
            let content = r.ok.clone().or_else(|| r.error.clone()).unwrap_or_default();
            messages.push(json!({ "role": "tool", "tool_call_id": r.id, "content": content }));
        }

        let mut body = json!({ "model": self.model, "messages": messages });

        if !req.tools.is_empty() {
            let tools: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    // Advertise the tool's REAL input schema so the model can form a valid call. A
                    // tool with no declared schema degrades to an empty object (argument-less), the
                    // OpenAI-required shape. Without a real schema the model cannot know the arguments
                    // and falls back to asking the user in prose.
                    let parameters = t
                        .parameters
                        .clone()
                        .unwrap_or_else(|| json!({ "type": "object" }));
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": parameters,
                        },
                    })
                })
                .collect();
            body["tools"] = Value::Array(tools);
        }

        body
    }

    /// The attributed fault detail prefix — one place so the message shape is exact (scope Risks).
    /// The key is never in the detail.
    fn detail(&self, detail: impl std::fmt::Display) -> String {
        format!("openai-compat/{}: {detail}", self.model)
    }
}

impl Provider for OpenAiCompat {
    async fn complete(&self, req: &AiRequest) -> Result<AiResponse, ProviderFault> {
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
            Err(e) if e.is_timeout() => return Err(ProviderFault::timeout(self.detail(e))),
            Err(e) => return Err(ProviderFault::network(self.detail(e))),
        };

        // Non-2xx is a fault, not a completion. Carry the STRUCTURED evidence: the status, the
        // `Retry-After` header (delta-seconds form only), and — read from the error body's
        // machine field, never its prose — the context-overflow discriminant.
        let status = resp.status();
        if !status.is_success() {
            let retry_after = parse_retry_after(&resp);
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            let fault = if is_overflow_body(&body) {
                ProviderFault::overflow(status.as_u16(), self.detail("context overflow"))
            } else {
                ProviderFault::http(
                    status.as_u16(),
                    retry_after,
                    self.detail(format!("http {}", status.as_u16())),
                )
            };
            return Err(fault);
        }

        let value: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                return Err(ProviderFault::malformed(
                    self.detail(format!("unparseable body: {e}")),
                ))
            }
        };

        parse_completion(&value)
            .ok_or_else(|| ProviderFault::malformed(self.detail("no choices in response")))
    }
}

/// Parse the `Retry-After` header's delta-seconds form. The HTTP-date form is ignored on purpose
/// (it would need a wall clock; the retry lane then just uses its default backoff).
fn parse_retry_after(resp: &reqwest::Response) -> Option<u64> {
    resp.headers()
        .get("retry-after")?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The structured overflow discriminant: `error.code == "context_length_exceeded"` (the OpenAI-
/// compat machine field). A machine code is data, not prose — this is NOT error-string parsing.
fn is_overflow_body(body: &Value) -> bool {
    body.get("error")
        .and_then(|e| e.get("code"))
        .and_then(Value::as_str)
        .map(|c| c == "context_length_exceeded")
        .unwrap_or(false)
}

/// Read one turn out of a chat-completions response. `None` when the shape is missing the pieces a
/// turn needs (no `choices[0]`), which the caller attributes as a failure.
fn parse_completion(value: &Value) -> Option<AiResponse> {
    let choice = value.get("choices")?.as_array()?.first()?;
    let message = choice.get("message")?;

    // Strip any `<think>…</think>` reasoning block some models (GLM) inline in `content` — it is not
    // the answer and reads as broken if it reaches a channel message or an authoring turn.
    let content = super::strip_think::strip_think(
        message.get("content").and_then(Value::as_str).unwrap_or(""),
    );

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
