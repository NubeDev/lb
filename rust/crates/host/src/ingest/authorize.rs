//! The ingest capability gate — each verb is a host-native MCP tool, gated by `mcp:<verb>:call`
//! through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first, then capability). The
//! same gate every MCP surface uses; ingest is not special (ingest scope, §3.5).
//!
//! Verbs: `ingest.write` (append), `series.read` (range), `series.latest` (newest). A denial is
//! opaque [`IngestError::Denied`] — no existence signal, so an un-granted producer cannot learn what
//! series exist.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::IngestError;

/// Authorize the `<verb>` MCP surface (e.g. `ingest.write`, `series.read`) in workspace `ws`.
/// `Ok(())` only if gate 1 (ws) and `mcp:<verb>:call` both pass.
pub fn authorize_ingest(principal: &Principal, ws: &str, verb: &str) -> Result<(), IngestError> {
    authorize_tool(principal, ws, verb).map_err(|_| IngestError::Denied)
}
