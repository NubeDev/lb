//! `identity.set_credential` — the admin verb that sets/rotates a user's password hash
//! (login-hardening scope). Gated by `mcp:identity.manage:call` (the same admin gate the other
//! `identity.*` verbs ride). The secret VALUE flows only through here: it is argon2-hashed before any
//! write and NEVER returned by a read (secrets rule §6.7). Workspace-first — the record lands in the
//! caller's own workspace namespace (from the token), so a forged cross-workspace set is denied
//! server-side (the row would land in the caller's ws, never the target's).
//!
//! There is no `list` (never enumerate secrets), no live-feed, no batch — a single create/update
//! verb, exactly as the scope's MCP-surface §6.1 prescribes.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{write, Store};

use super::error::CredentialError;
use super::hash::hash_secret;
use super::model::{Credential, CREDENTIAL_TABLE};

/// Set (or rotate) the password credential for `sub` in the caller's workspace, as `principal`.
/// `secret` is the plaintext — hashed here, never stored raw. Idempotent on `sub` (rotation upserts).
/// Returns `Ok(())` with no view (a credential read never returns the hash).
pub async fn identity_set_credential(
    store: &Store,
    principal: &Principal,
    sub: &str,
    secret: &str,
    ts: u64,
) -> Result<(), CredentialError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| CredentialError::Denied)?;
    if secret.is_empty() {
        return Err(CredentialError::BadInput("secret must not be empty".into()));
    }
    let sub = normalize_sub(sub);
    let phc = hash_secret(secret).map_err(CredentialError::Hash)?;
    let record = Credential::new(&sub, phc, ts);
    let value = serde_json::to_value(&record)
        .map_err(|e| CredentialError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    // The record lands in the caller's OWN workspace namespace (the token's ws) — the hard wall.
    write(store, principal.ws(), CREDENTIAL_TABLE, &sub, &value).await?;
    Ok(())
}

/// Canonicalize a login handle to the `user:<name>` form the identity model keys on (the same
/// canonicalization the login route applies), so a credential set for `ada` and a login as `user:ada`
/// resolve to the same record.
pub fn normalize_sub(sub: &str) -> String {
    if sub.starts_with("user:") {
        sub.to_string()
    } else {
        format!("user:{sub}")
    }
}
