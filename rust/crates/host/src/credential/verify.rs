//! `credential_verify` — the **login-path** credential seam (login-hardening scope). Called by the
//! gateway's `PasswordHash` credential check BEFORE minting, with NO principal yet (we are deciding
//! whether to issue one) — so this is un-gated, exactly like `user_login_check` /
//! `membership_login_resolve`. It reads the `(ws, sub)` credential record from the workspace namespace
//! and constant-time-compares the presented secret against the stored argon2 hash.
//!
//! Workspace isolation is structural: the record is read from `ws`'s own namespace, so a password set
//! in `acme` is invisible to a `beta` login (the hard wall §7). A missing record → `CredentialAbsent`
//! (the caller decides whether that is a 401 or, under the dev flag, a trust-any pass).

use lb_store::{read, Store};

use super::error::CredentialError;
use super::hash::verify_secret;
use super::model::{Credential, CREDENTIAL_TABLE};
use super::set::normalize_sub;

/// The outcome of a login-path credential check. Distinct from an `Err` (a store/hash failure): these
/// are the three *authentication* answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialCheck {
    /// A credential exists and the presented secret matches — minting may proceed.
    Ok,
    /// A credential exists but the secret does not match — `401`, no token.
    BadSecret,
    /// No credential is set for `(ws, sub)`. The caller decides: a `PasswordHash` policy `401`s
    /// (no password ⇒ cannot prove identity); a `DevTrustAny` policy passes (dev/CI).
    Absent,
}

/// Check `secret` against the stored credential for `sub` in workspace `ws`. Reads the workspace-
/// namespaced record (the hard wall). `Err` only on a store failure or a corrupt hash — never on a
/// wrong password (that is `BadSecret`) or a missing record (that is `Absent`).
pub async fn credential_verify(
    store: &Store,
    ws: &str,
    sub: &str,
    secret: &str,
) -> Result<CredentialCheck, CredentialError> {
    let sub = normalize_sub(sub);
    let Some(value) = read(store, ws, CREDENTIAL_TABLE, &sub).await? else {
        return Ok(CredentialCheck::Absent);
    };
    let record: Credential = serde_json::from_value(value)
        .map_err(|e| CredentialError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if verify_secret(secret, &record.phc).map_err(CredentialError::Hash)? {
        Ok(CredentialCheck::Ok)
    } else {
        Ok(CredentialCheck::BadSecret)
    }
}
