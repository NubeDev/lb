//! `identity_change_password` — the **self-service** password change (email-login scope). Backs
//! `POST /auth/password {old, new}` with a valid full token: an authenticated person changes THEIR
//! OWN global password. NOT admin-gated — the authorization is "you hold a valid token for this
//! `sub`" (the route already verified it) PLUS "you know the current password" (verified here). It
//! never touches another identity's credential; `sub` is the token's own subject, passed by the route.
//!
//! Verifies `old` against the stored hash first (a wrong/absent current password is `BadOldSecret` →
//! `401`), then hashes and upserts `new`. One argon2 verify + one write; no admin cap consulted.

use lb_authz::identity_credential_phc;
use lb_store::Store;

use crate::credential::{hash_secret, normalize_sub, verify_secret};

use super::error::IdentityCredentialError;

/// Change `sub`'s global password from `old` to `new`. `sub` MUST be the caller's own verified subject
/// (the route passes `principal.sub()`). Requires the current password to match — no admin bypass.
pub async fn identity_change_password(
    store: &Store,
    sub: &str,
    old: &str,
    new: &str,
    ts: u64,
) -> Result<(), IdentityCredentialError> {
    if new.is_empty() {
        return Err(IdentityCredentialError::BadInput(
            "new secret must not be empty".into(),
        ));
    }
    let sub = normalize_sub(sub);
    // Verify the current password. An absent credential means there is nothing to change with `old`
    // — treat it as a bad current secret (self-service cannot bootstrap a first password; that is the
    // admin `identity.set_password` path).
    let Some(phc) = identity_credential_phc(store, &sub).await? else {
        return Err(IdentityCredentialError::BadOldSecret);
    };
    if !verify_secret(old, &phc).map_err(IdentityCredentialError::Hash)? {
        return Err(IdentityCredentialError::BadOldSecret);
    }
    let new_phc = hash_secret(new).map_err(IdentityCredentialError::Hash)?;
    lb_authz::identity_credential_set(store, &sub, &new_phc, ts).await?;
    Ok(())
}
