//! `identity.set_password` — the admin verb that sets/rotates a person's GLOBAL password
//! (email-login scope). Gated `mcp:identity.manage:call` (the same admin gate the other `identity.*`
//! verbs ride). The secret VALUE flows only through here: argon2-hashed before any write, never
//! returned by a read (§6.7). Unlike the per-ws `identity.set_credential`, this credential is global
//! (one per identity, all workspaces) and lands in the reserved `_lb_identity` namespace — a person
//! has one password everywhere.
//!
//! No `list` (never enumerate secrets), no live-feed, no batch — a single create/update verb.

use lb_auth::Principal;
use lb_authz::identity_credential_set;
use lb_mcp::authorize_tool;
use lb_store::Store;

use crate::credential::{hash_secret, normalize_sub};

use super::error::IdentityCredentialError;

/// Set (or rotate) the global password for `sub`, as `principal`. `secret` is the plaintext — hashed
/// here, never stored raw. Idempotent on `sub` (rotation upserts). Returns `Ok(())` — a credential
/// read never returns the hash.
pub async fn identity_set_password(
    store: &Store,
    principal: &Principal,
    sub: &str,
    secret: &str,
    ts: u64,
) -> Result<(), IdentityCredentialError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| IdentityCredentialError::Denied)?;
    if secret.is_empty() {
        return Err(IdentityCredentialError::BadInput(
            "secret must not be empty".into(),
        ));
    }
    let sub = normalize_sub(sub);
    let phc = hash_secret(secret).map_err(IdentityCredentialError::Hash)?;
    identity_credential_set(store, &sub, &phc, ts).await?;
    Ok(())
}
