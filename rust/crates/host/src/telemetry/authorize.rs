//! The telemetry capability gate (telemetry-console scope). Workspace-first, then
//! `mcp:telemetry.<verb>:call` — the same two-gate chokepoint every host verb runs (rule 5) before
//! touching the ring. A denied caller is opaque, indistinguishable from a missing tool.
//!
//! Two capability tiers, by design (the operator-sink vs tenant-wall boundary):
//!   - **`mcp:telemetry.read:call`** — the workspace-facing read grant (query/trace/tail). It is
//!     workspace-walled: even holding it, a caller sees ONLY their `ws` (enforced in the read verbs,
//!     not here).
//!   - **`mcp:telemetry.purge:call`** — the destructive node-admin op (a separate, higher grant).

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::TelemetrySvcError;

/// Authorize a telemetry `verb` for `principal` in `ws`. Workspace isolation first, then the
/// capability. The **read** verbs (query/trace/tail) all gate on the ONE `mcp:telemetry.read:call`
/// grant (the scope's "a new telemetry:read capability gates the query + tail tools"); `purge` gates
/// on its own separate, higher `mcp:telemetry.purge:call`. `Ok(())` only if both pass.
pub fn authorize_telemetry(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), TelemetrySvcError> {
    let cap = read_or_admin_cap(verb);
    authorize_tool(principal, ws, cap).map_err(|_| TelemetrySvcError::Denied)
}

/// Map a telemetry verb to the capability string it gates on. The three read verbs collapse to the
/// single `telemetry.read`; `purge` is its own higher grant. Shared with the dispatch outer gate so
/// the outer gate and this inner authorize agree (one gate, two callers).
pub fn read_or_admin_cap(verb: &str) -> &'static str {
    match verb {
        "telemetry.purge" => "telemetry.purge",
        // query / trace / tail / any other telemetry read → the single read grant.
        _ => "telemetry.read",
    }
}
