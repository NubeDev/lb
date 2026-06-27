//! The capability gate for the system-observability verbs. Identical shape to `dbview`/`dashboard`:
//! delegate to the shared `authorize_tool` (workspace-first §7, then `mcp:<verb>:call`), collapsing
//! any refusal to the opaque `Denied`. Admin-only by grant convention — a system snapshot reads
//! across the whole workspace (every subsystem's state), so the cap belongs to the workspace-admin
//! role, mirroring the `store.*` lens, NOT the member set.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::SystemError;

/// Authorize `principal` to call `verb` (`system.overview` / `system.topology`) in `ws`. The single
/// gate the two verbs run before reading any subsystem state; a denial is opaque.
pub fn authorize_system(principal: &Principal, ws: &str, verb: &str) -> Result<(), SystemError> {
    authorize_tool(principal, ws, verb).map_err(|_| SystemError::Denied)
}
