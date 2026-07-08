//! The run-scoped configuration the role crate hands the shim via env. One struct, one reader:
//! every env name lives here so a rename is a one-file change (the "wrapper config drift" risk
//! the scope names).

use std::env;
use std::time::Duration;

/// The env name of the gateway base URL (e.g. `http://127.0.0.1:8080`). The shim appends
/// `/mcp/call` and `/agent/runs/{id}/token/refresh`.
pub const ENV_GATEWAY_URL: &str = "LB_MCP_GATEWAY_URL";
/// The env name of the run-scoped bearer token (the only credential the shim holds).
pub const ENV_RUN_TOKEN: &str = "LB_MCP_RUN_TOKEN";
/// The env name of the run id (used in the refresh path).
pub const ENV_RUN_ID: &str = "LB_MCP_RUN_ID";
/// The env name of the path to the pre-baked menu JSON (advertisement only — `caps::check` is
/// the wall). See [`crate::menu`].
pub const ENV_MENU_PATH: &str = "LB_MCP_MENU_PATH";
/// The env name of the unix-second timestamp at which to refresh the token before the next call
/// (60% of TTL, set by the role crate at mint time — D2). Absent ⇒ never refresh proactively.
pub const ENV_REFRESH_AT_SEC: &str = "LB_MCP_REFRESH_AT_SEC";

/// The liveness bound for the whole stdio session. The shim is the child of the agent process;
/// when the agent exits, stdin closes and [`serve`] returns promptly. This is a backstop for a
/// wedged agent that holds stdin open without sending — NOT a run supervision ceiling (the job
/// record owns that, run-lifecycle #5).
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(3600);

/// The resolved env config. Returned by [`read_env`] so callers handle the missing-field error
/// once, at the boundary.
#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub gateway_url: String,
    pub token: String,
    pub run_id: String,
    pub menu_path: String,
    pub refresh_at: Option<u64>,
}

/// Read the four mandatory env vars + the optional refresh timestamp. A missing mandatory var is
/// a fatal misconfiguration (the role crate always sets them) — surfaced as a plain error string
/// so the bin's `main` can print it and exit non-zero (the agent sees a fast, clear failure).
pub fn read_env() -> Result<EnvConfig, String> {
    let gateway_url = env::var(ENV_GATEWAY_URL)
        .map_err(|_| format!("{ENV_GATEWAY_URL} is not set"))?
        .trim_end_matches('/')
        .to_string();
    let token = env::var(ENV_RUN_TOKEN).map_err(|_| format!("{ENV_RUN_TOKEN} is not set"))?;
    let run_id = env::var(ENV_RUN_ID).map_err(|_| format!("{ENV_RUN_ID} is not set"))?;
    let menu_path = env::var(ENV_MENU_PATH).map_err(|_| format!("{ENV_MENU_PATH} is not set"))?;
    let refresh_at = env::var(ENV_REFRESH_AT_SEC)
        .ok()
        .and_then(|s| s.parse().ok());
    Ok(EnvConfig {
        gateway_url,
        token,
        run_id,
        menu_path,
        refresh_at,
    })
}

/// The idle timeout for the stdio loop. Exposed for the loop + tests.
pub fn idle_timeout() -> Duration {
    DEFAULT_IDLE_TIMEOUT
}
