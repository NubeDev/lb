//! `identity_by_email` — the **login-path** email→`sub` resolution (email-login scope). Called by the
//! gateway `/auth/login` route BEFORE any principal exists (it is deciding *whom* to authenticate), so
//! it is un-gated, exactly like `membership_login_resolve` / the credential verify. Case-insensitive
//! via the folded reverse index. `Ok(None)` when no identity claims the email — the route then runs
//! the timing-uniform credential path anyway (no email-enumeration oracle) and returns the uniform
//! `401`.

use lb_authz as raw;
use lb_store::Store;

use super::error::IdentityError;

/// Resolve the `sub` that owns `email` (case-insensitive). `Ok(None)` if unknown. No authorization —
/// pre-principal login path.
pub async fn identity_by_email(
    store: &Store,
    email: &str,
) -> Result<Option<String>, IdentityError> {
    Ok(raw::identity_by_email(store, email).await?)
}
