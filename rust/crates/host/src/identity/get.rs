//! `identity.get` — read one global identity. Gated by `mcp:identity.manage:call`. Read-only on the
//! system directory; resolution paths (login, switcher) read it through the un-gated raw seam, but the
//! admin verb itself re-checks the cap server-side.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::IdentityError;
use super::model::IdentityView;

/// Read global identity `sub` as `principal`. `Ok(None)` if it does not exist.
pub async fn identity_get(
    store: &Store,
    principal: &Principal,
    sub: &str,
) -> Result<Option<IdentityView>, IdentityError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| IdentityError::Denied)?;
    Ok(raw::identity_get(store, sub).await?.map(IdentityView::from))
}
