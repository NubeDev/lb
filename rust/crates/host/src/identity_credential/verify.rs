//! `global_credential_verify` — the **login-path** global-credential seam (email-login scope). Called
//! by the gateway's `GlobalPasswordHash` check BEFORE minting, with NO principal yet (we are deciding
//! whether to issue one) — un-gated, like `membership_login_resolve`. It reads the identity's global
//! credential from `_lb_identity` and constant-time-compares the presented secret against the stored
//! argon2 hash.
//!
//! **Timing-uniform on an unknown identity (§ account-enumeration risk).** When no credential exists
//! for `sub`, this does NOT early-return in µs — it verifies the secret against a fixed **dummy hash**
//! so an unknown-email login burns the same argon2 cost as a wrong-password login. Both then resolve
//! to `Absent`/`BadSecret`, which the route collapses to the one uniform `401`. The dummy compare's
//! result is discarded; only its *time* matters.

use std::sync::OnceLock;

use lb_authz::identity_credential_phc;
use lb_store::Store;

use crate::credential::{hash_secret, normalize_sub, verify_secret};

use super::error::IdentityCredentialError;

/// A process-wide dummy argon2id hash (computed once) used to spend argon2 time on an unknown-identity
/// login so its latency matches a wrong-password login (no timing oracle for email enumeration). The
/// value is irrelevant — it never matches a real login secret; only the cost of verifying against it
/// matters. Computed with the same `hash_secret` params the real path uses, so the cost is identical.
fn dummy_phc() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_secret("timing-uniform-dummy-secret").unwrap_or_else(|_| {
            // hash_secret only fails on an internal argon2 error, which would break the real path
            // too; fall back to a static well-formed PHC so verify_secret still spends compare time.
            "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzYWx0c2FsdA$\
             8Q4nqjD0m3nT2oJ8xkqg0mEC9d5aFmQ2h5m1r9m1a0"
                .to_string()
        })
    })
}

/// The outcome of a login-path global-credential check. Distinct from an `Err` (a store/hash
/// failure): these are the three *authentication* answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalCredentialCheck {
    /// A credential exists and the presented secret matches — minting may proceed.
    Ok,
    /// A credential exists but the secret does not match — `401`, no token.
    BadSecret,
    /// No global credential is set for `sub`. The caller decides: a `GlobalPasswordHash` policy
    /// `401`s (no password ⇒ cannot prove identity); a `GlobalDevTrustAny` policy passes (dev/CI).
    Absent,
}

/// Check `secret` against `sub`'s stored global credential. Timing-uniform: an absent credential still
/// runs one argon2 verify (against [`DUMMY_PHC`]) before returning `Absent`, so it cannot be told from
/// a wrong password by response time. `Err` only on a store failure or a corrupt stored hash — never
/// on a wrong password (`BadSecret`) or a missing record (`Absent`).
pub async fn global_credential_verify(
    store: &Store,
    sub: &str,
    secret: &str,
) -> Result<GlobalCredentialCheck, IdentityCredentialError> {
    let sub = normalize_sub(sub);
    match identity_credential_phc(store, &sub).await? {
        Some(phc) => {
            if verify_secret(secret, &phc).map_err(IdentityCredentialError::Hash)? {
                Ok(GlobalCredentialCheck::Ok)
            } else {
                Ok(GlobalCredentialCheck::BadSecret)
            }
        }
        None => {
            // Burn the argon2 cost so an unknown identity is timing-indistinguishable from a wrong
            // password. The dummy hash is well-formed, so this returns `Ok(false)`; a parse error on
            // our own constant would be a build-time bug — ignore its result either way.
            let _ = verify_secret(secret, dummy_phc());
            Ok(GlobalCredentialCheck::Absent)
        }
    }
}
