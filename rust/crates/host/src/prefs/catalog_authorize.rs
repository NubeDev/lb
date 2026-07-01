//! The i18n-catalog capability gates (i18n-catalogs scope "How it fits the core → Capabilities"):
//!   - `message.render` — member-level (`mcp:message.render:call`) for a caller's OWN render;
//!     rendering for ANOTHER recipient additionally requires `mcp:message.render_recipient:call`
//!     (the service/admin fan-out grant the outbox producer holds).
//!   - `prefs.catalog` — member-level (`mcp:prefs.catalog:call`) — a member reads the merged catalog
//!     to render locally, mirroring member-level `prefs.resolve`.
//!   - `message.set_catalog` — admin-gated (`mcp:message.set_catalog:call`), beside `prefs.set_default`.
//!
//! Unlike the grant-free `format.*` tier (no tenant data), render is GATED: a catalog with workspace
//! overrides carries tenant content. All gates run through the shared `authorize_tool` chokepoint
//! (workspace-first, then capability); a denial is opaque (no key/existence signal).

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::PrefsSvcError;

/// Authorize a plain catalog verb (`prefs.catalog` / `message.set_catalog`) in `ws`. `Ok(())` only
/// if the workspace wall and `mcp:<verb>:call` both pass. Opaque denial.
pub fn authorize_catalog(principal: &Principal, ws: &str, verb: &str) -> Result<(), PrefsSvcError> {
    authorize_tool(principal, ws, verb).map_err(|_| PrefsSvcError::Denied)
}

/// Authorize `message.render`. The base `mcp:message.render:call` is always required; rendering FOR
/// ANOTHER recipient (`for_another`) requires the `mcp:message.render_recipient:call` fan-out grant
/// ON TOP. A caller lacking either is denied opaquely — a member can render for SELF but not fan out
/// for others (the mandatory deny-test).
pub fn authorize_render(
    principal: &Principal,
    ws: &str,
    for_another: bool,
) -> Result<(), PrefsSvcError> {
    authorize_tool(principal, ws, "message.render").map_err(|_| PrefsSvcError::Denied)?;
    if for_another {
        authorize_tool(principal, ws, "message.render_recipient")
            .map_err(|_| PrefsSvcError::Denied)?;
    }
    Ok(())
}
