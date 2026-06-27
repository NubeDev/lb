//! The DB-browser capability gate — each verb (`store.tables`/`store.scan`/`store.graph`) is a
//! host-native MCP tool gated by `mcp:<verb>:call` through the shared `lb_mcp::authorize_tool`
//! chokepoint (workspace-first, then capability). Same gate every MCP surface uses.
//!
//! **The headline decision (data-console scope):** these caps are **admin-only**. The raw browser
//! deliberately relaxes the per-record membership gate (gate 3) that typed verbs like `get_doc`
//! enforce — a raw `SELECT * FROM <table>` answers "every record in the workspace". So the grant
//! belongs to the workspace-admin role *only*, never `member_caps`. Two gates still hold hard: the
//! **workspace wall** (`use_ws` binds the namespace — a ws-B admin physically cannot scan ws-A) and
//! the **capability** (no grant → opaque `Denied`). A denial is opaque [`DbViewError::Denied`].

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::DbViewError;

/// Authorize the `<verb>` DB-browser MCP surface (e.g. `store.tables`, `store.scan`, `store.graph`)
/// in workspace `ws`. `Ok(())` only if gate 1 (ws) and `mcp:<verb>:call` both pass.
pub fn authorize_dbview(principal: &Principal, ws: &str, verb: &str) -> Result<(), DbViewError> {
    authorize_tool(principal, ws, verb).map_err(|_| DbViewError::Denied)
}
