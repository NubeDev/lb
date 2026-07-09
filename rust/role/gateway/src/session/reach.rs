//! **The surface-reach guard** (nav-reach scope) — authenticate a request, then enforce that the
//! caller may OPEN the core `surface` (page) it targets. `authenticate` answers "who is calling";
//! this adds "may they reach this page?" — the server-side boundary that makes a curated nav the
//! allow-list of reachable surfaces, read included.
//!
//! It is called at each core surface's ENTRY route (the first list/get that loads a page), keyed on
//! the **surface** (not the entry verb — the surface→cap map handles the rules/data mismatches). A
//! caller whose nav did not grant that surface holds no `reach:<surface>:view` cap and gets a `403`,
//! the same opaque deny an ungranted verb returns. A caller with a fallback nav holds `reach:*:view`
//! and reaches everything (a default member/admin is never locked out).
//!
//! Defense in depth, not the *only* line: the client route guard hides/redirects for UX, but a curled
//! entry read hits THIS gate server-side — the deliverable the client guard could never be.

use axum::http::{HeaderMap, StatusCode};
use lb_auth::Principal;

use super::authenticate::authenticate;
use crate::state::Gateway;

/// Authenticate the request and require reach to `surface`. Returns the verified [`Principal`] on
/// success, or the HTTP tuple a route returns: `401` if authentication failed (unchanged), `403` if
/// authenticated but the caller's nav did not grant this surface. `surface` is the opaque core-surface
/// key (rule 10) — e.g. `"rules"`, `"ingest"`, `"datasources"`.
pub async fn require_reach(
    gw: &Gateway,
    headers: &HeaderMap,
    surface: &str,
) -> Result<Principal, (StatusCode, String)> {
    let principal = authenticate(gw, headers)
        .await
        .map_err(|e| e.into_response())?;
    if !lb_host::reach_check(&principal, principal.ws(), surface) {
        // Opaque 403 — the caller is authenticated but this page is not in their reachable set. Mirror
        // the wording of an ungranted-cap deny so a curled read can't distinguish "no reach" from any
        // other 403 (no oracle on why the page is closed).
        return Err((StatusCode::FORBIDDEN, "forbidden".to_string()));
    }
    Ok(principal)
}
