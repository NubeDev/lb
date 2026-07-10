//! The brand capability gate (gates 1+2) — each verb is a host-native MCP tool, gated by
//! `mcp:brand.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability). Brands are not special (mirrors `authorize_panel`). A denial is opaque
//! [`BrandError::Denied`].

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::BrandError;

/// Authorize the `brand.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:brand.<verb>:call` both pass.
pub fn authorize_brand(principal: &Principal, ws: &str, verb: &str) -> Result<(), BrandError> {
    authorize_tool(principal, ws, verb).map_err(|_| BrandError::Denied)
}
