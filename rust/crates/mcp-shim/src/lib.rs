//! `lb-mcp-shim` — the stdio MCP ⇄ `POST /mcp/call` bridge for an external agent run
//! (external-agent-authoring scope S1).
//!
//! A spawned external agent (Open Interpreter / codex-family) is configured to treat this binary
//! as its only MCP server. It speaks JSON-RPC 2.0 over stdio (one message per line) on three
//! methods: `initialize`, `tools/list`, `tools/call`. The first two are answered locally (the
//! menu is a JSON file the role crate pre-baked into the scratch dir — advertisement is UX; the
//! wall is `caps::check`). `tools/call` is forwarded to the gateway's `POST /mcp/call` under the
//! run-scoped bearer token; the gateway re-checks the workspace + `mcp:<tool>:call` capability
//! exactly as it does for a UI page — nothing is reachable from the subprocess that the caller
//! could not already do.
//!
//! ## Token lifecycle (agent-key-lifecycle D1–D5)
//! The role crate mints a short-TTL (5 min) run-scoped token and hands it to the shim via env.
//! The shim refreshes it lazily — on the first call after `LB_MCP_REFRESH_AT_SEC` (60% TTL, D2)
//! AND as a one-shot self-heal on a 401 (D2). The gateway's verify refuses a token whose run is
//! terminal (D3 — run-status-gated, see `verify_token` in the gateway), so a hard cancel is
//! instant regardless of TTL. Refresh is `POST /agent/runs/{id}/token/refresh`, itself run-status
//! gated (refused once the run is terminal).
//!
//! ## What the shim deliberately does NOT do
//! - Hold principal state or caps. The token is opaque bearer material; every call is re-checked
//!   by the gateway. The shim is plumbing.
//! - Sandbox the subprocess. The OS sandbox is `capability-wall-scope.md` (#3); publish/install
//!   stay Ask-gated, which is the actual safety floor shipped here.
//! - Touch the goal/transcript/build log. The token appears only in env + the per-run MCP config
//!   file inside the scratch dir (mode 0600, deleted with the run) — byte-asserted in tests.
//!
//! File layout (one verb per file, FILE-LAYOUT §3): [`serve`] (the JSON-RPC stdio loop),
//! [`forward`] (the HTTP forward), [`refresh`] (token refresh), [`menu`] (read the menu file).

pub mod config;
mod forward;
mod menu;
mod refresh;
mod serve;

pub use config::{
    read_env, EnvConfig, ENV_GATEWAY_URL, ENV_MENU_PATH, ENV_REFRESH_AT_SEC, ENV_RUN_ID,
    ENV_RUN_TOKEN,
};
pub use forward::{call_gateway, ForwardError};
pub use menu::{load_menu, MenuEntry};
pub use refresh::Refresher;
pub use serve::{serve, serve_on};
