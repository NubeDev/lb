//! The read-only SQL capability gate — `store.query` and `store.schema` are host-native MCP tools,
//! each gated by `mcp:store.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint
//! (workspace-first §3.6, then capability §3.5). A denial is opaque [`StoreQueryError::Denied`].
//!
//! Unlike the `dbview` raw-store lens (admin-only, gate-3-relaxed), `store.query` is a *bounded,
//! parse-allowlisted SELECT* — it is leashed by the parse gate (`parse.rs`) and the workspace wall,
//! so it can be granted to whoever the install grant allows (a widget cell calls it only if
//! `mcp:store.query:call ∈ cell.tools ∩ install-grant`). The cap grant convention is a separate
//! decision (dev claims grant it admin-side); this gate just enforces whatever the token carries.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::StoreQueryError;

/// Authorize the `store.<verb>` read-only SQL surface in `ws`. `Ok(())` only if gate 1 (workspace)
/// and `mcp:store.<verb>:call` both pass.
pub fn authorize_store_query(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), StoreQueryError> {
    authorize_tool(principal, ws, verb).map_err(|_| StoreQueryError::Denied)
}
