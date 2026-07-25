//! The form capability gate (gates 1+2) — each verb is a host-native MCP tool, gated by
//! `mcp:forms.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability). The same gate every MCP surface uses; forms are not special. Forms are simple
//! owner/workspace assets — there is no gate-3 visibility check (a workspace member with the cap may
//! read), so this is the only authorization step.
//!
//! A denial is opaque [`FormError::Denied`] — no existence signal, so an un-granted caller cannot
//! learn what forms exist.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::FormError;

/// Authorize the `forms.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:forms.<verb>:call` both pass.
pub fn authorize_form(principal: &Principal, ws: &str, verb: &str) -> Result<(), FormError> {
    authorize_tool(principal, ws, verb).map_err(|_| FormError::Denied)
}
