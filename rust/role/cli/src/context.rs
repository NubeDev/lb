//! Resolve the run context: which workspace, which gateway, which transport — the seam between the
//! parsed flags/config/env and a ready [`Transport`](crate::transport::Transport). This is where the
//! `-w` **credential-selector** rule lives (operator-cli scope): `-w` picks which stored token to
//! present; it can NEVER override the token's workspace (there is no ws in the `/mcp/call` body), and a
//! workspace with no stored credential is a LOUD error, not a silent ignore.
//!
//! Precedence (mirrors the node's env-config style): env (`LB_*`) > the config file > the built-in
//! default. Kept here (unit-testable) rather than in `main.rs`.

use crate::config::{self, Config};
use crate::error::{CliError, CliResult};
use crate::transport::{Local, Remote};

/// The default gateway URL when neither env, config, nor `--url` supplies one.
pub const DEFAULT_GATEWAY_URL: &str = "http://127.0.0.1:8080";
/// The default dev user for local mode (parity with the node's `LB_SEED_USER` default).
pub const DEFAULT_LOCAL_USER: &str = "user:ada";

/// The resolved global options a command runs under, independent of transport. Built from the parsed
/// flags + the loaded config + env overrides.
#[derive(Debug, Clone)]
pub struct RunContext {
    /// The selected workspace (the `-w` value, or `LB_WORKSPACE`, or the config default). `None` only
    /// before any login in remote mode, which is then a loud error at credential selection.
    pub workspace: Option<String>,
    /// The gateway base URL (env > config > `--url` > default).
    pub gateway_url: String,
    /// `true` for `lb local …` / `--local`.
    pub local: bool,
    /// The loaded config (holds the per-workspace tokens).
    pub config: Config,
}

impl RunContext {
    /// Resolve the effective workspace: the explicit `-w`, else `LB_WORKSPACE`, else the config's
    /// `default_workspace`.
    pub fn resolve_workspace(&self) -> Option<String> {
        self.workspace
            .clone()
            .or_else(|| non_empty_env(config::WORKSPACE_ENV))
            .or_else(|| self.config.default_workspace.clone())
    }

    /// Build the REMOTE transport for the resolved workspace: select its stored credential (or
    /// `LB_TOKEN`), erroring LOUDLY if none is stored (the credential-selector rule). The workspace the
    /// token was minted for is what reaches the server — `-w` only chose which token, it did not
    /// override the wall.
    pub fn remote(&self) -> CliResult<Remote> {
        let ws = self
            .resolve_workspace()
            .ok_or_else(|| CliError::Other("no workspace selected; run `lb login -w <ws>`".into()))?;
        // `LB_TOKEN` (CI) wins over the file, but is still keyed to the selected workspace's identity —
        // it is the credential for THIS ws, injected out-of-band.
        let token = non_empty_env(config::TOKEN_ENV)
            .or_else(|| self.config.token_for(&ws).map(str::to_string))
            .ok_or_else(|| CliError::NoCredential { workspace: ws.clone() })?;
        Ok(Remote::new(self.gateway_url.clone(), token))
    }

    /// Build the LOCAL transport: boot an in-process node and mint a `dev_claims` principal scoped to
    /// the resolved workspace (or a sensible default). No credential lookup — local mints its own
    /// principal (offline, no gateway). The `-w` scopes the minted principal's workspace, which IS the
    /// wall; it cannot reach outside it.
    pub async fn local(&self) -> CliResult<Local> {
        let ws = self
            .resolve_workspace()
            .unwrap_or_else(|| "acme".to_string());
        let user = non_empty_env("LB_SEED_USER").unwrap_or_else(|| DEFAULT_LOCAL_USER.to_string());
        Local::boot(&user, &ws).await
    }
}

/// Read a non-empty env var, treating unset OR empty as absent (so `LB_TOKEN=` does not select an
/// empty credential).
fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// Compute the gateway URL from precedence env > config > explicit `--url` > default. `--url` is the
/// operator's per-invocation override; env still wins so CI can pin it globally.
pub fn resolve_gateway_url(flag_url: Option<&str>, config: &Config) -> String {
    non_empty_env(config::GATEWAY_URL_ENV)
        .or_else(|| flag_url.map(str::to_string))
        .or_else(|| config.gateway_url.clone())
        .unwrap_or_else(|| DEFAULT_GATEWAY_URL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(ws: Option<&str>, local: bool, config: Config) -> RunContext {
        RunContext {
            workspace: ws.map(str::to_string),
            gateway_url: DEFAULT_GATEWAY_URL.to_string(),
            local,
            config,
        }
    }

    #[test]
    fn unstored_workspace_is_a_loud_error() {
        // `-w gamma` with no stored gamma credential must error loudly, never silently act elsewhere
        // (the load-bearing `-w` trap the scope names).
        let mut config = Config::default();
        config.set_token("acme", "tok-acme");
        let c = ctx(Some("gamma"), false, config);
        match c.remote() {
            Err(CliError::NoCredential { workspace }) => assert_eq!(workspace, "gamma"),
            other => panic!("expected loud NoCredential, got {other:?}"),
        }
    }

    #[test]
    fn dash_w_selects_the_matching_stored_credential() {
        let mut config = Config::default();
        config.set_token("acme", "tok-acme");
        config.set_token("beta", "tok-beta");
        // Selecting beta uses beta's token (its own ws reaches the server — no override).
        let c = ctx(Some("beta"), false, config);
        assert!(c.remote().is_ok());
    }

    #[test]
    fn workspace_falls_back_to_config_default() {
        let mut config = Config::default();
        config.set_token("acme", "tok"); // set_token also sets default_workspace = acme
        let c = ctx(None, false, config);
        assert_eq!(c.resolve_workspace().as_deref(), Some("acme"));
    }

    #[test]
    fn gateway_url_default_when_nothing_set() {
        assert_eq!(
            resolve_gateway_url(None, &Config::default()),
            DEFAULT_GATEWAY_URL
        );
    }

    #[test]
    fn gateway_url_flag_over_default_config_over_flag() {
        let mut config = Config::default();
        config.gateway_url = Some("http://config:1".into());
        // config wins over the flag default fallback order? Precedence: env > flag > config > default.
        assert_eq!(
            resolve_gateway_url(Some("http://flag:2"), &config),
            "http://flag:2"
        );
        assert_eq!(resolve_gateway_url(None, &config), "http://config:1");
    }
}
