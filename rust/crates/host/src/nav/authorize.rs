//! The nav capability gate (gates 1+2) — each verb is a host-native MCP tool, gated by
//! `mcp:nav.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability — nav scope, "Capabilities"). The same gate every MCP surface uses; navs are not
//! special (rule 10 — a nav takes the exact same caps/auth path as any asset). Gate 3
//! (membership/visibility) is a separate check in `visibility.rs`, run strictly *after* this.
//!
//! A denial is opaque [`NavError::Denied`] — no existence signal, so an un-granted caller cannot
//! learn what navs exist.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::NavError;

/// Authorize the `nav.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:nav.<verb>:call` both pass.
pub fn authorize_nav(principal: &Principal, ws: &str, verb: &str) -> Result<(), NavError> {
    authorize_tool(principal, ws, verb).map_err(|_| NavError::Denied)
}
