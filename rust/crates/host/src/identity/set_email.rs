//! `identity.set_email` — the admin verb that sets/changes a global identity's email login handle
//! (email-login scope). Gated `mcp:identity.manage:call` (the same admin gate the other `identity.*`
//! verbs ride). The email is claimed globally-unique via the race-safe reverse index; a duplicate is
//! [`IdentityError::EmailTaken`] (409), never a silent overwrite. The identity must already exist.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::create::map_email_conflict;
use super::error::IdentityError;
use super::model::IdentityView;

/// Set (or change) `sub`'s email as `principal`. Returns the updated secret-free view.
pub async fn identity_set_email(
    store: &Store,
    principal: &Principal,
    sub: &str,
    email: &str,
) -> Result<IdentityView, IdentityError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| IdentityError::Denied)?;
    let identity = raw::identity_set_email(store, sub, email)
        .await
        .map_err(map_email_conflict)?;
    Ok(IdentityView::from(identity))
}
