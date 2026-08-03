//! `lb login` — the front door (operator-cli scope, decision #3): POST `{email, password}` to
//! `/auth/login` (the same door the browser uses — the ONLY human door since the legacy
//! `POST /login {user, workspace}` was deleted in the pre-production legacy sweep) and store the
//! signed token **keyed by the workspace it was minted for** (`0600`, never logged). The token
//! already carries the workspace + caps, verified per request by `session::authenticate`, so the wall
//! holds at the front door with no new auth code. `-w` on later commands selects this stored
//! credential.
//!
//! `/auth/login` is authenticate-then-choose, so this mirrors the browser's 0/1/N branch: one
//! workspace mints immediately; several return a short-lived select-token and this posts the caller's
//! `-w <ws>` to `/auth/select`. With several workspaces and no `-w`, the error NAMES them rather than
//! guessing.
//!
//! Login is REMOTE-only: local mode has no login (it mints a `dev_claims` principal in-process). The
//! `/auth/*` routes are not `/mcp/call` (they issue the token that later `/mcp/call`s present), so
//! this is the one command that reaches a typed gateway route directly.
//!
//! Machines do not use this at all — an agent/appliance/raw API caller carries an **lb API key**.

use serde::{Deserialize, Serialize};

use crate::error::{CliError, CliResult};

/// The `/auth/login` request body — the human's email + password. No workspace and no `user:`
/// principal ever cross this wire (mirrors the gateway's `AuthLoginRequest`).
#[derive(Debug, Serialize)]
struct AuthLoginRequest<'a> {
    email: &'a str,
    password: &'a str,
}

/// The `/auth/select` request body — the workspace picked out of the roster.
#[derive(Debug, Serialize)]
struct AuthSelectRequest<'a> {
    workspace: &'a str,
}

/// One workspace in the login roster (mirrors the gateway's `WorkspaceRow`).
#[derive(Debug, Deserialize)]
pub struct WorkspaceRow {
    pub ws: String,
    #[serde(default)]
    pub name: String,
}

/// The `/auth/*` reply — either a full session (`token` set) or a select-needed branch
/// (`select_token` set), plus the roster in both (mirrors the gateway's `AuthReply`).
#[derive(Debug, Deserialize)]
pub struct AuthReply {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub principal: Option<String>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub caps: Vec<String>,
    #[serde(default)]
    pub select_token: Option<String>,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRow>,
}

/// The resolved session the command stores: the signed token plus the facts the header needs.
#[derive(Debug)]
pub struct LoginReply {
    pub token: String,
    pub principal: String,
    pub workspace: String,
    pub caps: Vec<String>,
}

/// Authenticate `{email, password}` at `{base_url}/auth/login`, then resolve to ONE workspace: the
/// auto-skip when the person belongs to exactly one, or `/auth/select` with `workspace` when they
/// belong to several. A transport failure is a DOWN gateway (clear error, never a hang); a non-2xx is
/// the server's verbatim message (a wrong password is an opaque `401 invalid credentials`).
pub async fn do_login(
    client: &reqwest::Client,
    base_url: &str,
    email: &str,
    password: &str,
    workspace: Option<&str>,
) -> CliResult<LoginReply> {
    let base = base_url.trim_end_matches('/');
    let reply: AuthReply = post(
        client,
        &format!("{base}/auth/login"),
        None,
        &AuthLoginRequest { email, password },
    )
    .await?;

    // The 1-branch: the gateway already minted the full token (the person has exactly one workspace).
    if let Some(session) = into_session(&reply) {
        return Ok(session);
    }

    // The N-branch: pick with the select-token. `-w` is the pick — the CLI never guesses.
    let select_token = reply.select_token.ok_or_else(|| {
        CliError::Transport("login reply carried neither a token nor a select-token".into())
    })?;
    let ws = workspace.ok_or_else(|| {
        let names: Vec<&str> = reply.workspaces.iter().map(|w| w.ws.as_str()).collect();
        CliError::BadInput(format!(
            "you belong to several workspaces ({}); pick one: `lb login -w <ws>`",
            names.join(", ")
        ))
    })?;
    let selected: AuthReply = post(
        client,
        &format!("{base}/auth/select"),
        Some(&select_token),
        &AuthSelectRequest { workspace: ws },
    )
    .await?;
    into_session(&selected)
        .ok_or_else(|| CliError::Transport("select reply carried no session token".into()))
}

/// Read the full-session branch out of a reply, if this is one.
fn into_session(reply: &AuthReply) -> Option<LoginReply> {
    let token = reply.token.clone()?;
    Some(LoginReply {
        token,
        principal: reply.principal.clone().unwrap_or_default(),
        workspace: reply.workspace.clone().unwrap_or_default(),
        caps: reply.caps.clone(),
    })
}

/// POST `body` to `url` (optionally bearing `token`) and decode the `AuthReply`. Non-2xx is relayed
/// verbatim — the gateway's uniform `401 invalid credentials` is deliberately not interpreted here.
async fn post<B: Serialize>(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
    body: &B,
) -> CliResult<AuthReply> {
    let mut req = client.post(url).json(body);
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    let resp = req
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
    resp.json::<AuthReply>()
        .await
        .map_err(|e| CliError::Transport(format!("decode login reply: {e}")))
}
