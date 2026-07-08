//! The MCP-shim bridge wiring for one external-agent run (external-agent-authoring scope S1d).
//! Builds the two things the shim + the wrapper need from the role crate:
//!
//! 1. **The menu JSON** — the `tools/list` answer, pre-baked from the narrowed `&[AllowedTool]`
//!    (advertisement only; `caps::check` on every `tools/call` is the wall). Written into the
//!    scratch dir so the shim reads it by path (no gateway round-trip for the menu).
//! 2. **The bridge env vars** — `LB_MCP_GATEWAY_URL`, `LB_MCP_RUN_TOKEN`, `LB_MCP_RUN_ID`,
//!    `LB_MCP_MENU_PATH`, `LB_MCP_REFRESH_AT_SEC`. These are set on the agent child's env; the
//!    shim (the agent's own MCP-server child) inherits them.
//!
//! 3. **The per-wrapper MCP config** — each wrapper (codex, vtcode, …) names its MCP-server
//!    config differently. [`AgentWrapper::mcp_config`](wrapper) returns the config-file contents
//!    the wrapper expects (pointing its MCP stanza at the shim binary); the role crate writes it
//!    into the scratch dir and the wrapper's argv points the agent at it.
//!
//! The token + the gateway URL NEVER appear in the goal text, a record, or a log — they live only
//! in the per-child env map + the per-run config file inside the scratch dir (mode 0600, deleted
//! with the run). Byte-asserted in the bridge integration test.

use std::fs;
use std::io;
use std::path::Path;

use lb_external_agent::AgentWrapper;
use lb_host::AllowedTool;
use serde::Serialize;

use crate::token::RunToken;

/// The env-name constants the shim reads. Mirrors `lb_mcp_shim::config` — kept here so the role
/// crate is the one source of truth for what it sets (a rename is a two-file change: here + the
/// shim crate).
pub const ENV_GATEWAY_URL: &str = "LB_MCP_GATEWAY_URL";
pub const ENV_RUN_TOKEN: &str = "LB_MCP_RUN_TOKEN";
pub const ENV_RUN_ID: &str = "LB_MCP_RUN_ID";
pub const ENV_MENU_PATH: &str = "LB_MCP_MENU_PATH";
pub const ENV_REFRESH_AT_SEC: &str = "LB_MCP_REFRESH_AT_SEC";

/// The default gateway URL when the node config / env supplies none. Matches the dev node's
/// `make dev` bind (`127.0.0.1:8080`). A production node sets `LB_GATEWAY_URL`.
const DEFAULT_GATEWAY_URL: &str = "http://127.0.0.1:8080";

/// One menu entry — mirrors the shim's `MenuEntry` shape (`{name, description?, inputSchema?}`).
/// Kept as a local struct (not a dep on the shim crate) so the role crate → shim boundary is the
/// JSON file, not a shared type (the shim is a standalone leaf with no shared dep on host/auth).
#[derive(Debug, Serialize)]
struct MenuEntry<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(rename = "inputSchema", skip_serializing_if = "Option::is_none")]
    input_schema: Option<&'a serde_json::Value>,
}

/// The bridge artifacts for one run — the env vars to inject + the files written into the scratch
/// dir. The role crate writes the files and passes `env` to [`lb_external_agent::drive`] as
/// `extra_env`.
pub struct Bridge {
    pub env: Vec<(String, String)>,
}

/// Read the gateway URL the node is configured with. `LB_GATEWAY_URL` env → default. The role
/// crate is placement-agnostic (config, never an `if cloud`) — the operator sets this per node.
pub fn gateway_url() -> String {
    std::env::var("LB_GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY_URL.to_string())
}

/// Build the bridge for one run: write the menu JSON + the wrapper's MCP config into `scratch`,
/// and return the env vars to inject into the child. `tools` is the NARROWED menu (already
/// `reachable ∩ persona.granted_tools`); `shim_bin` is the shim binary path/name the wrapper's
/// MCP config points at (e.g. `"lb-mcp-shim"` if it's on PATH).
///
/// Returns `None` when the wrapper provides no MCP config (the pre-bridge path — the agent runs
/// with no host-tool surface, same as today). The caller skips the env injection in that case.
pub fn build(
    wrapper: &dyn AgentWrapper,
    scratch: &Path,
    run_id: &str,
    tools: &[AllowedTool],
    run_token: &RunToken,
    shim_bin: &str,
) -> Result<Option<Bridge>, io::Error> {
    // 1. The menu JSON — advertisement only; `caps::check` on every call is the wall.
    let menu_path = scratch.join("menu.json");
    let entries: Vec<MenuEntry<'_>> = tools
        .iter()
        .map(|t| MenuEntry {
            name: &t.name,
            description: if t.description.is_empty() {
                None
            } else {
                Some(&t.description)
            },
            input_schema: t.input_schema.as_ref(),
        })
        .collect();
    let menu_json = serde_json::to_vec(&entries).expect("menu always serializes");
    fs::write(&menu_path, &menu_json)?;
    // The menu path is passed as a string env var — the shim reads it at startup.
    let menu_path_str = menu_path.to_string_lossy().into_owned();

    // 2. The per-wrapper MCP config (e.g. codex's `config.toml` with `[mcp_servers.lb]`). The
    //    wrapper owns the format; the role crate writes whatever it returns into the scratch dir.
    //    `None` ⇒ this wrapper has no MCP bridge (pre-bridge behavior); the env is still built so
    //    the shim, if spawned by other means, still works. In practice `None` means the wrapper
    //    cannot drive the bridge at all.
    let Some(config) = wrapper.mcp_config(shim_bin, scratch) else {
        return Ok(None);
    };
    let config_path = scratch.join(&config.file_name);
    fs::write(&config_path, &config.contents)?;
    // Tighten perms — the config file contains the run token in env stanzas. Best-effort on
    // non-Unix; on Unix, 0600 so only the node user can read it. Deleted with the scratch dir.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&config_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&config_path, perms)?;
    }

    // 3. The bridge env vars — set on the agent child; the shim (its MCP child) inherits them.
    let gw = gateway_url();
    let env = vec![
        (ENV_GATEWAY_URL.to_string(), gw),
        (ENV_RUN_TOKEN.to_string(), run_token.token.clone()),
        (ENV_RUN_ID.to_string(), run_id.to_string()),
        (ENV_MENU_PATH.to_string(), menu_path_str),
        (
            ENV_REFRESH_AT_SEC.to_string(),
            run_token.refresh_at_sec.to_string(),
        ),
    ];
    Ok(Some(Bridge { env }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_external_agent::wrapper::{AgentWrapper, Decoded};
    use lb_external_agent::AgentProfile;

    /// A test wrapper that returns a minimal MCP config pointing at a fake shim binary.
    struct TestWrapper;
    impl AgentWrapper for TestWrapper {
        fn id(&self) -> &'static str {
            "test"
        }
        fn command_args(&self, _: &AgentProfile, _: &str, _: &str) -> Vec<String> {
            vec![]
        }
        fn decode_line(&self, _: &str, _: u32) -> Decoded {
            Decoded::Ignore
        }
        fn mcp_config(
            &self,
            shim_bin: &str,
            _scratch: &Path,
        ) -> Option<lb_external_agent::wrapper::McpConfig> {
            Some(lb_external_agent::wrapper::McpConfig {
                file_name: "config.toml".into(),
                contents: format!("[mcp_servers.lb]\ncommand = \"{shim_bin}\"\n"),
            })
        }
    }

    #[test]
    fn build_writes_menu_and_config_and_env() {
        let dir = std::env::temp_dir().join("lb-bridge-test");
        std::fs::create_dir_all(&dir).unwrap();
        let tools = vec![
            AllowedTool {
                name: "tools.catalog".into(),
                description: "List tools".into(),
                input_schema: None,
            },
            AllowedTool {
                name: "devkit.scaffold".into(),
                description: "".into(),
                input_schema: Some(serde_json::json!({"type": "object"})),
            },
        ];
        let rt = RunToken {
            token: "tok-123".into(),
            refresh_at_sec: 999,
            exp_sec: 1000,
        };
        let bridge = build(&TestWrapper, &dir, "run-1", &tools, &rt, "lb-mcp-shim")
            .unwrap()
            .expect("wrapper provides a config");
        // The menu JSON was written.
        let menu = std::fs::read(dir.join("menu.json")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_slice(&menu).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["name"], "tools.catalog");
        assert_eq!(parsed[0]["description"], "List tools");
        assert!(parsed[0].get("inputSchema").is_none()); // skip_serializing_if None
        assert_eq!(parsed[1]["name"], "devkit.scaffold");
        assert!(parsed[1].get("description").is_none()); // empty description → None
        assert!(parsed[1]["inputSchema"]["type"] == "object");
        // The wrapper config was written.
        let cfg = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(cfg.contains("[mcp_servers.lb]"));
        assert!(cfg.contains("lb-mcp-shim"));
        // The env carries the 5 bridge vars.
        let names: Vec<&str> = bridge.env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "LB_MCP_GATEWAY_URL",
                "LB_MCP_RUN_TOKEN",
                "LB_MCP_RUN_ID",
                "LB_MCP_MENU_PATH",
                "LB_MCP_REFRESH_AT_SEC"
            ]
        );
        // The token value is in the env (never in the goal/record — only the child env map).
        let token_val = bridge
            .env
            .iter()
            .find(|(k, _)| k == "LB_MCP_RUN_TOKEN")
            .unwrap();
        assert_eq!(token_val.1, "tok-123");
    }

    #[test]
    fn build_returns_none_when_wrapper_has_no_mcp_config() {
        // A wrapper that returns None from mcp_config → no bridge (the pre-bridge path).
        struct NoBridge;
        impl AgentWrapper for NoBridge {
            fn id(&self) -> &'static str {
                "no-bridge"
            }
            fn command_args(&self, _: &AgentProfile, _: &str, _: &str) -> Vec<String> {
                vec![]
            }
            fn decode_line(&self, _: &str, _: u32) -> Decoded {
                Decoded::Ignore
            }
        }
        let dir = std::env::temp_dir().join("lb-bridge-test-none");
        std::fs::create_dir_all(&dir).unwrap();
        let rt = RunToken {
            token: "t".into(),
            refresh_at_sec: 0,
            exp_sec: 0,
        };
        let bridge = build(&NoBridge, &dir, "r", &[], &rt, "shim").unwrap();
        assert!(bridge.is_none(), "a no-bridge wrapper returns None");
    }

    #[test]
    fn gateway_url_defaults_to_localhost() {
        // When LB_GATEWAY_URL is unset, the default is 127.0.0.1:8080.
        std::env::remove_var("LB_GATEWAY_URL");
        assert_eq!(gateway_url(), "http://127.0.0.1:8080");
    }
}
