//! The CLI's ONLY persistence: its own config file (operator-cli scope, Non-goals: "no new
//! persistence layer"). One TOML file at `$LB_DIR/config` holding the gateway URL, the default
//! workspace, and the session token **keyed by the workspace it was minted for** — so `-w acme` loads
//! the `acme` credential and a ws-A token can never address ws-B (the wall holds by construction; `-w`
//! is a credential selector, never an override).
//!
//! The token is secret material: the file is written `0600`, the token is NEVER logged and NEVER
//! echoed in output (the token-custody risk + the "no command emits the token" test). `keyring` is the
//! documented phase-2 upgrade off the plaintext file — same seam api-keys names for its hash.
//!
//! Env overrides mirror the node's env-config style: `LB_GATEWAY_URL`, `LB_TOKEN`, `LB_WORKSPACE`
//! win over the file (so CI can inject a credential without writing one to disk).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CliError, CliResult};

/// The env var naming the `.lazybones` root the Makefile already uses. Defaults to `~/.lazybones`.
pub const LB_DIR_ENV: &str = "LB_DIR";
/// Env override: the gateway base URL (`http://127.0.0.1:8080`).
pub const GATEWAY_URL_ENV: &str = "LB_GATEWAY_URL";
/// Env override: a raw session token, bypassing the file's per-workspace slot (CI injects here).
pub const TOKEN_ENV: &str = "LB_TOKEN";
/// Env override: the default workspace (the credential to select).
pub const WORKSPACE_ENV: &str = "LB_WORKSPACE";

/// The persisted CLI config. Small on purpose: a gateway URL, a default workspace, and the
/// per-workspace token map. Nothing platform (no SurrealDB, no Zenoh) — that is the host's.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// The gateway base URL the remote transport POSTs to. `None` until the first `lb login --url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway_url: Option<String>,
    /// The workspace `-w` defaults to when the flag is omitted. Set by the last `lb login`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_workspace: Option<String>,
    /// `workspace -> token`. Keyed by workspace so a stored credential IS scoped to its workspace;
    /// selecting a workspace with no entry is a loud error, never a silent cross-workspace hop.
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
}

impl Config {
    /// The stored token for `workspace`, if any. Read by the remote transport to authenticate.
    pub fn token_for(&self, workspace: &str) -> Option<&str> {
        self.tokens.get(workspace).map(String::as_str)
    }

    /// Store (or replace) the token for `workspace` and make it the default. Called by `lb login`.
    pub fn set_token(&mut self, workspace: &str, token: impl Into<String>) {
        self.tokens.insert(workspace.to_string(), token.into());
        self.default_workspace = Some(workspace.to_string());
    }
}

/// The config-file path: `$LB_DIR/config` (`LB_DIR` env, default `~/.lazybones`). The Makefile's
/// `.lazybones/` root is the same directory, so the CLI and the dev flow share one home.
pub fn config_path() -> PathBuf {
    lb_dir().join("config")
}

/// The `.lazybones` root — `$LB_DIR` if set, else `~/.lazybones`, else `.lazybones` in the cwd (a
/// home-less environment like a minimal container).
pub fn lb_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(LB_DIR_ENV) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .map(|h| h.join(".lazybones"))
        .unwrap_or_else(|| PathBuf::from(".lazybones"))
}

/// Load the config from `path`. A missing file is an empty default (first run) — not an error, so a
/// brand-new operator can `lb login` without a pre-existing file.
pub fn load_from(path: &Path) -> CliResult<Config> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            toml::from_str(&text).map_err(|e| CliError::Other(format!("parse config {path:?}: {e}")))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(CliError::Other(format!("read config {path:?}: {e}"))),
    }
}

/// Load the config from the default [`config_path`].
pub fn load() -> CliResult<Config> {
    load_from(&config_path())
}

/// Persist `config` to `path`, creating the parent dir and enforcing `0600` on the file (owner
/// read/write only) — the token is secret material at rest (the token-custody risk). On unix the mode
/// is set explicitly; other platforms rely on the created-file default (documented gap).
pub fn save_to(config: &Config, path: &Path) -> CliResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError::Other(format!("create {parent:?}: {e}")))?;
    }
    let text = toml::to_string_pretty(config)
        .map_err(|e| CliError::Other(format!("serialize config: {e}")))?;
    std::fs::write(path, text).map_err(|e| CliError::Other(format!("write config {path:?}: {e}")))?;
    set_owner_only(path)?;
    Ok(())
}

/// Persist `config` to the default [`config_path`].
pub fn save(config: &Config) -> CliResult<()> {
    save_to(config, &config_path())
}

/// Set `0600` on the config file (unix). A best-effort no-op elsewhere.
#[cfg(unix)]
fn set_owner_only(path: &Path) -> CliResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)
        .map_err(|e| CliError::Other(format!("chmod 0600 {path:?}: {e}")))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> CliResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_loads_empty_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let cfg = load_from(&path).unwrap();
        assert_eq!(cfg, Config::default());
        assert!(cfg.tokens.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let mut cfg = Config::default();
        cfg.gateway_url = Some("http://127.0.0.1:8080".into());
        cfg.set_token("acme", "tok-acme");
        cfg.set_token("beta", "tok-beta");
        save_to(&cfg, &path).unwrap();

        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded, cfg);
        assert_eq!(loaded.token_for("acme"), Some("tok-acme"));
        assert_eq!(loaded.token_for("beta"), Some("tok-beta"));
        // login set the default to the last workspace.
        assert_eq!(loaded.default_workspace.as_deref(), Some("beta"));
    }

    #[test]
    fn saved_config_is_0600() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let mut cfg = Config::default();
        cfg.set_token("acme", "secret-token");
        save_to(&cfg, &path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            // Only the owner bits may be set — no group/other read on secret material.
            assert_eq!(mode & 0o777, 0o600, "config must be 0600, got {:o}", mode & 0o777);
        }
    }

    #[test]
    fn token_for_unknown_workspace_is_none() {
        let cfg = Config::default();
        assert_eq!(cfg.token_for("nobody"), None);
    }
}
