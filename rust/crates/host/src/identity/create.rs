//! `identity.create` — provision a global identity in the system directory (in NO workspace).
//! Gated by `mcp:identity.manage:call` (hub-only, decision #8). Idempotent on `sub` — re-creating
//! upserts the display name / created_ts. Provisioning ≠ joining (decision #4): a freshly-created
//! identity has zero memberships and cannot mint until `membership.add` (or `create_workspace`).

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::IdentityError;
use super::model::IdentityView;

/// Create (or update) global identity `sub` as `principal`, optionally with an `email` login handle
/// (email-login scope). When `email` is given it is claimed globally-unique via the race-safe reverse
/// index — a duplicate email is [`IdentityError::EmailTaken`]. Returns the secret-free view.
pub async fn identity_create(
    store: &Store,
    principal: &Principal,
    sub: &str,
    display_name: Option<&str>,
    email: Option<&str>,
    ts: u64,
) -> Result<IdentityView, IdentityError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| IdentityError::Denied)?;
    let identity = match email {
        Some(email) => raw::identity_create_with_email(store, sub, display_name, email, ts)
            .await
            .map_err(map_email_conflict)?,
        None => raw::identity_create(store, sub, display_name, ts).await?,
    };
    Ok(IdentityView::from(identity))
}

/// Translate a raw `Conflict` (email already owned by another identity) into the typed
/// [`IdentityError::EmailTaken`]; every other store error passes through.
pub(crate) fn map_email_conflict(e: lb_store::StoreError) -> IdentityError {
    match e {
        lb_store::StoreError::Conflict => IdentityError::EmailTaken,
        other => IdentityError::Store(other),
    }
}
