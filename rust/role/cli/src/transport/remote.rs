//! The **remote** transport (operator-cli scope, decision #1): route every command through the one
//! universal endpoint, `POST /mcp/call`. The gateway authenticates the session token this holds,
//! re-checks the workspace + `mcp:<tool>:call`, and returns the tool's JSON — so one client code path
//! reaches EVERY verb with zero per-verb HTTP wiring, and a new host verb is free the moment it ships.
//!
//! The workspace is NEVER in the body — `POST /mcp/call` carries `{tool, args}` only, so the server
//! reads the workspace from the verified token (the hard wall holds at the front door). The token is
//! held here to set the `Authorization: Bearer` header; it is never logged and never rendered.

use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::error::{CliError, CliResult};
use crate::header::{header_from_token, Header};

use super::Transport;

/// A reqwest client bound to one gateway URL + one session token. Constructed by the command layer
/// after it selected the credential for the `-w` workspace (a missing credential errored before we got
/// here — the credential-selector rule).
///
/// `Debug` is hand-written to REDACT the token — a derived `Debug` would print the bearer, defeating
/// the never-log discipline. Only the URL is shown.
pub struct Remote {
    client: reqwest::Client,
    base_url: String,
    token: String,
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the token — a redacted placeholder keeps `{:?}` (used in test panics) from
        // leaking the secret.
        f.debug_struct("Remote")
            .field("base_url", &self.base_url)
            .field("token", &"<redacted>")
            .finish()
    }
}

impl Remote {
    /// Build a remote transport for `base_url` authenticating with `token`. The base URL is the gateway
    /// root (`http://127.0.0.1:8080`); routes are appended.
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
        }
    }

    /// The gateway base URL (for the ext-publish path, which POSTs `/extensions`, not `/mcp/call`).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The session token (for the ext-publish path's `Authorization` header). Crate-internal; never
    /// rendered to the operator.
    pub(crate) fn token(&self) -> &str {
        &self.token
    }

    /// The shared reqwest client.
    pub(crate) fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

impl Transport for Remote {
    fn header(&self) -> Header {
        // Decode the token we hold; if it is somehow unreadable, fall back to unknowns rather than
        // panicking — the call itself will still authenticate (or fail) at the server.
        header_from_token(&self.token, false)
            .unwrap_or_else(|| Header::new("?", "?", lb_auth::Role::Member, false))
    }

    fn caps(&self) -> Vec<String> {
        lb_auth::claims_unverified(&self.token)
            .map(|c| c.caps)
            .unwrap_or_default()
    }

    async fn call(&self, tool: &str, args: Value) -> CliResult<Value> {
        let url = format!("{}/mcp/call", self.base_url);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&json!({ "tool": tool, "args": args }))
            .send()
            .await
            // A transport-layer failure (refused connection, DNS, timeout) is a DOWN gateway — a clear
            // error, never a hang and never a fake success (the mandatory offline test for remote mode).
            .map_err(|e| CliError::Transport(e.to_string()))?;

        map_response(tool, resp).await
    }
}

/// Map a `/mcp/call` HTTP response to the CLI result. A `403` is the server's opaque authorization
/// deny → surface it as `DENIED  mcp:<tool>:call` (never a fabricated ok). A `401` is an auth failure
/// (bad/expired token). Any other non-2xx carries the server's message verbatim. A `2xx` is the tool's
/// JSON result.
async fn map_response(tool: &str, resp: reqwest::Response) -> CliResult<Value> {
    let status = resp.status();
    if status.is_success() {
        return resp
            .json::<Value>()
            .await
            .map_err(|e| CliError::Transport(format!("decode result: {e}")));
    }
    // Read the server's body for the verbatim message (the scope: "surface the server's deny string
    // verbatim"). Failing to read it still yields an honest error, not a success.
    let body = resp.text().await.unwrap_or_default();
    match status {
        StatusCode::FORBIDDEN => Err(CliError::Denied {
            tool: tool.to_string(),
        }),
        StatusCode::UNAUTHORIZED => Err(CliError::Transport(format!(
            "unauthorized (401): {}",
            body.trim()
        ))),
        other => Err(CliError::Transport(format!(
            "gateway returned {other}: {}",
            body.trim()
        ))),
    }
}
