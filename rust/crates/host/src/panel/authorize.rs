//! The panel capability gate (gates 1+2) — each verb is a host-native MCP tool, gated by
//! `mcp:panel.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability). The same gate every MCP surface uses; panels are not special (library-panels
//! scope, mirrors `authorize_dashboard`). Gate 3 (membership/visibility) is a separate check in
//! `visibility.rs`, run strictly *after* this.
//!
//! A denial is opaque [`PanelError::Denied`] — no existence signal, so an un-granted caller cannot
//! learn what panels exist.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::PanelError;

/// Authorize the `panel.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:panel.<verb>:call` both pass.
pub fn authorize_panel(principal: &Principal, ws: &str, verb: &str) -> Result<(), PanelError> {
    authorize_tool(principal, ws, verb).map_err(|_| PanelError::Denied)
}
