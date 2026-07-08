//! Forward one MCP `tools/call` to the gateway's `POST /mcp/call`. The gateway authenticates the
//! bearer, re-checks workspace-first + `mcp:<tool>:call`, and dispatches — the shim is transport.
//!
//! The MCP `tools/call` result is `{content: [{type:"text", text:"..."}], isError?: bool}`. The
//! gateway's `/mcp/call` returns the tool's JSON output (or a `403`/error string). We wrap that
//! into the MCP content-block shape so a Codex/Open-Interpreter agent renders it natively: a
//! JSON reply becomes one text block (the JSON stringified), a `403`/error becomes one text
//! block with `isError: true`. We do NOT introspect or rewrite the body — the agent sees exactly
//! what the host returned, in the shape the MCP spec mandates.

use serde::Serialize;
use serde_json::Value;

/// The `POST /mcp/call` body — the same shape the UI bridge sends (`{tool, args}`).
#[derive(Debug, Serialize)]
pub struct McpCallBody<'a> {
    pub tool: &'a str,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub args: &'a Value,
}

/// The forward outcome — what the shim turns into the MCP `tools/call` result.
#[derive(Debug, Clone, Serialize)]
pub struct CallOutcome {
    /// The MCP content blocks (one text block carrying the gateway's reply).
    pub content: Vec<ContentBlock>,
    /// `true` when the gateway refused (caps deny / unknown tool / bad input). Drives the
    /// `isError` flag on the MCP result so the agent surfaces it instead of treating it as data.
    #[serde(rename = "isError", skip_serializing_if = "is_false")]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub text: String,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// What can go wrong forwarding the call — surfaced to [`crate::serve`] which decides whether to
/// retry after a refresh (401) or fail the call.
#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    /// The gateway returned `401` — the run token is expired/revoked. The shim attempts ONE
    /// refresh-and-retry (D2 self-heal) before treating this as terminal.
    #[error("gateway returned 401 (token expired/revoked)")]
    Unauthorized,
    /// A network/transport error (gateway unreachable).
    #[error("transport: {0}")]
    Transport(String),
    /// A non-401 HTTP failure (gateway internal error, etc.).
    #[error("gateway status {status}: {body}")]
    Status { status: u16, body: String },
}

impl CallOutcome {
    fn ok(text: String) -> Self {
        Self {
            content: vec![ContentBlock { kind: "text", text }],
            is_error: false,
        }
    }

    fn err(text: String) -> Self {
        Self {
            content: vec![ContentBlock { kind: "text", text }],
            is_error: true,
        }
    }
}

/// Forward one tool call. `gateway_url` is the base (no trailing `/`); `token` is the current
/// bearer. The body is the MCP `tools/call` arguments verbatim (a JSON object) — the gateway's
/// `/mcp/call` accepts `args` as any JSON value (null → `"{}"`).
pub async fn call_gateway(
    client: &reqwest::Client,
    gateway_url: &str,
    token: &str,
    tool: &str,
    args: &Value,
) -> Result<CallOutcome, ForwardError> {
    let url = format!("{gateway_url}/mcp/call");
    let body = McpCallBody { tool, args };
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| ForwardError::Transport(e.to_string()))?;
    let status = resp.status().as_u16();
    if status == 401 {
        return Err(ForwardError::Unauthorized);
    }
    let text = resp
        .text()
        .await
        .map_err(|e| ForwardError::Transport(e.to_string()))?;
    if !(200..300).contains(&status) {
        // A 403 (caps deny), a 400 (bad input), or a 5xx — all surface as an MCP `isError` block
        // carrying the gateway's honest message. The agent sees the deny reason (the gateway
        // returns an opaque string per the MCP deny contract) and reports it.
        return Ok(CallOutcome::err(text));
    }
    // Success: the gateway returns the tool's JSON output (a string, number, object, …). Wrap it
    // in ONE text block. A JSON object is stringified (MCP text blocks are strings); a bare
    // string is passed through unchanged.
    let display = if text.starts_with('"') {
        // The gateway already returned a JSON string literal — unwrap one level for readability.
        serde_json::from_str::<String>(&text).unwrap_or(text)
    } else {
        text
    };
    Ok(CallOutcome::ok(display))
}
