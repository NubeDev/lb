//! `lb login [--url <gateway>] --email <e> [--password <p>] [-w <ws>]` — the front-door
//! orchestration: POST `/auth/login` (+ `/auth/select` when the person has several workspaces), store
//! the token keyed by the workspace it was minted for (`0600`), persist the gateway URL + default
//! workspace. Prints a header for the freshly-minted session — never the token or the password.
//!
//! Login is remote-only (local mints in-process). It is the one command that mutates the config file,
//! so the persistence lives here, not in `main.rs`.

use crate::config::{self, Config};
use crate::error::CliResult;
use crate::header::Header;
use crate::login::do_login;

use super::Printed;

/// The env var a password may come from instead of `--password`, so a shell history never holds it.
pub const PASSWORD_ENV: &str = "LB_LOGIN_PASSWORD";

/// Log in to `gateway_url` as `email`, into `workspace` when the person belongs to several, storing
/// the result in `config` (which the caller then saves). Returns the header for the new session + a
/// confirmation body. The token is stored but NEVER returned in `Printed`, and the password is never
/// echoed anywhere.
pub async fn run(
    config: &mut Config,
    gateway_url: &str,
    email: &str,
    password: &str,
    workspace: Option<&str>,
) -> CliResult<Printed> {
    let client = reqwest::Client::new();
    let reply = do_login(&client, gateway_url, email, password, workspace).await?;

    // Persist: the token keyed by workspace, the gateway URL, and the default workspace (set inside
    // `set_token`). The config write (0600) is the caller's — it saves after this returns.
    config.set_token(&reply.workspace, reply.token.clone());
    config.gateway_url = Some(gateway_url.trim_end_matches('/').to_string());

    // Build the header from the token we just stored (decoded, unverified) — the same source every
    // later command uses, so `login` and the next command agree.
    let header = crate::header::header_from_token(&reply.token, false).unwrap_or_else(|| {
        Header::new(
            reply.workspace.clone(),
            reply.principal.clone(),
            lb_auth::Role::Member,
            false,
        )
    });
    let body = format!(
        "logged in to {} as {} (workspace {}); credential stored in {:?}",
        gateway_url,
        reply.principal,
        reply.workspace,
        config::config_path()
    );
    Ok(Printed::new(&header, body))
}
