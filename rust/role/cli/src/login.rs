//! `lb login` — the front door (operator-cli scope, decision #3): POST the dev-login `{user,
//! workspace}` to the existing `/login` (the same the browser uses), and store the signed token
//! **keyed by the workspace it was minted for** (`0600`, never logged). The token already carries the
//! workspace + caps, verified per request by `session::authenticate`, so the wall holds at the front
//! door with no new auth code. `-w` on later commands selects this stored credential.
//!
//! Login is REMOTE-only: local mode has no login (it mints a `dev_claims` principal in-process). The
//! `/login` route is not `/mcp/call` (it issues the token that later `/mcp/call`s present), so this is
//! the one command that reaches a typed gateway route directly.

use serde::{Deserialize, Serialize};

use crate::error::{CliError, CliResult};

/// The `/login` request body — who, and into which workspace (mirrors the gateway's `LoginRequest`).
#[derive(Debug, Serialize)]
struct LoginRequest<'a> {
    user: &'a str,
    workspace: &'a str,
}

/// The `/login` reply — the signed token plus the resolved principal/workspace/caps (mirrors the
/// gateway's `LoginReply`). We keep only the token (secret) + the facts the header needs.
#[derive(Debug, Deserialize)]
pub struct LoginReply {
    pub token: String,
    pub principal: String,
    pub workspace: String,
    #[serde(default)]
    pub caps: Vec<String>,
}

/// Post `{user, workspace}` to `{base_url}/login` and return the reply. A transport failure is a DOWN
/// gateway (clear error, never a hang); a non-2xx is the server's verbatim message (a real deployment
/// would `401` a bad credential here — the dev-login accepts any user).
pub async fn do_login(
    client: &reqwest::Client,
    base_url: &str,
    user: &str,
    workspace: &str,
) -> CliResult<LoginReply> {
    let url = format!("{}/login", base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&LoginRequest { user, workspace })
        .send()
        .await
        .map_err(|e| CliError::Transport(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(CliError::Transport(format!(
            "login failed ({status}): {}",
            body.trim()
        )));
    }
    resp.json::<LoginReply>()
        .await
        .map_err(|e| CliError::Transport(format!("decode login reply: {e}")))
}
