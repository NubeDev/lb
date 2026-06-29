//! The prefs capability gates (prefs scope "Resolved decisions"):
//!   - `prefs.get` / `prefs.resolve` — read OWN (gated): `mcp:prefs.get:call` / `mcp:prefs.resolve:call`.
//!   - `prefs.set` — write OWN: `mcp:prefs.set:call`.
//!   - `prefs.set_default` — **admin-gated**: `mcp:prefs.set_default:call` (granted only to admins).
//!
//! All run through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first, then capability).
//! `format.*` / `convert.*` are a **grant-free utility tier** — pure CLDR/unit math over no tenant
//! data — so they are NOT authorized here (the tool bridge dispatches them without a gate).
//!
//! "Read OWN" is enforced structurally beyond the cap: the host forces the target `user` to the
//! caller's own `sub` for `prefs.get`/`prefs.set` (a caller cannot name another user), so even a
//! holder of `prefs.get` can only read their own record — the deny-test asserts this.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::PrefsSvcError;

/// Authorize the `prefs.<verb>` MCP surface in `ws`. `Ok(())` only if the workspace wall and
/// `mcp:prefs.<verb>:call` both pass. A denial is opaque.
pub fn authorize_prefs(principal: &Principal, ws: &str, verb: &str) -> Result<(), PrefsSvcError> {
    authorize_tool(principal, ws, verb).map_err(|_| PrefsSvcError::Denied)
}
