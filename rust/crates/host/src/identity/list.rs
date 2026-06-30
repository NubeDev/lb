//! `identity.list` — every global identity in the directory, for the admin console. Gated by
//! `mcp:identity.manage:call`. Returns secret-free views, sorted by `sub`.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::IdentityError;
use super::model::IdentityView;

/// Every global identity as `principal` sees them.
pub async fn identity_list(
    store: &Store,
    principal: &Principal,
) -> Result<Vec<IdentityView>, IdentityError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| IdentityError::Denied)?;
    let views = raw::identity_list(store)
        .await?
        .into_iter()
        .map(IdentityView::from)
        .collect();
    Ok(views)
}
