//! The `pack.<verb>` capability gate — the family's thin wrapper over the one caps wall, exactly
//! like `dashboard/authorize.rs`.
//!
//! This gates the `pack.*` SURFACE only. It grants nothing downstream: `pack.apply` additionally
//! drives each object through the same internal seam the equivalent public verb calls, and each of
//! those re-checks its OWN capability under this same principal. A caller who could not
//! `rules.save` cannot smuggle a rule in through a pack (pack-core-scope §Caps).

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::PackError;

/// Authorize the `pack.<verb>` MCP surface in workspace `ws`.
pub fn authorize_pack(principal: &Principal, ws: &str, verb: &str) -> Result<(), PackError> {
    authorize_tool(principal, ws, verb).map_err(|_| PackError::Denied)
}
