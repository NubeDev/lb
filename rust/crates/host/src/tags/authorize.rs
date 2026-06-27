//! The tags capability gate — each verb is a host-native MCP tool, gated by `mcp:<verb>:call`
//! through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first, then capability). The
//! same gate every MCP surface uses; tags are not special (tags scope, §3.5).
//!
//! Verbs: `tags.add`, `tags.remove`, `tags.of`, `tags.find` — and nothing else. There is NO
//! caller-facing verb for `DEFINE EVENT` (event registration is host-internal only, so a grant can
//! never weaponize write-amplification). A denial is opaque [`TagsError::Denied`].

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::TagsError;

/// Authorize the `tags.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:tags.<verb>:call` both pass.
pub fn authorize_tags(principal: &Principal, ws: &str, verb: &str) -> Result<(), TagsError> {
    authorize_tool(principal, ws, verb).map_err(|_| TagsError::Denied)
}
