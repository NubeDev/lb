//! The **credential check** — the seam that decides whether a `/login` may mint a token at all
//! (login-hardening scope, change 2). Before this, `login` trusted the request body: any caller who
//! could reach the port minted a valid signed token for any `(user, workspace)`, including an admin.
//! Now `login` calls [`CredentialCheck::verify`] BEFORE `mint`; a bad/absent secret is a `401` with
//! no token. The trait keeps the check pluggable behind the existing mint/verify boundary (README
//! §6.6): OIDC lands later as a third impl with no route change.
//!
//! Two impls ship:
//!   - [`PasswordHash`] — the real check: argon2 against the per-`(ws, user)` credential record
//!     (`lb_host::credential_verify`). A wrong secret `401`s; an ABSENT credential also `401`s (no
//!     password ⇒ identity unproven) — a workspace using passwords must set them.
//!   - [`DevTrustAny`] — today's password-less behavior, for local dev/CI ONLY. **Opt-in** via the
//!     `LB_DEV_LOGIN` env flag; a release build without the flag selects `PasswordHash` and refuses
//!     a password-less login (the scope's "hard-refuse in release" decision).
//!
//! Selection is [`CredentialCheck::from_env`]: `LB_DEV_LOGIN=1` → `DevTrustAny`, else `PasswordHash`.
//! Even under `DevTrustAny` the minted token is **role-scoped** (the cap trim in `credentials.rs`), so
//! a dev "member" login still cannot add members — dev convenience never re-opens the escalation.

use async_trait::async_trait;
use lb_host::{credential_verify, CredentialCheck as CheckOutcome, Node};

/// The env var that opts a node into the password-less dev-login. Set (to any non-empty value) for
/// local dev / CI; UNSET in a real deployment (which then requires a real credential).
pub const DEV_LOGIN_ENV: &str = "LB_DEV_LOGIN";

/// Why a login credential was refused. Collapses to `401` at the route (authenticity before
/// authority — a `403` would leak that the credential was valid but the principal ungranted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialRejection {
    /// The presented secret did not match, or no credential is set and the policy requires one.
    BadCredential,
    /// An internal store/hash failure while checking — fail closed (no token).
    CheckFailed,
}

/// The pluggable credential check `login` runs before minting. One method: prove `(user, workspace,
/// secret)` before a token is issued. `Ok(())` allows the mint; any `Err` is a `401`.
#[async_trait]
pub trait CredentialCheck: Send + Sync {
    async fn verify(
        &self,
        node: &Node,
        workspace: &str,
        user: &str,
        secret: &str,
    ) -> Result<(), CredentialRejection>;
}

/// Select the credential check from the environment. `LB_DEV_LOGIN` set → `DevTrustAny` (dev/CI);
/// unset → `PasswordHash` (the production default — a real credential is required).
pub fn credential_check_from_env() -> std::sync::Arc<dyn CredentialCheck> {
    match std::env::var(DEV_LOGIN_ENV) {
        Ok(v) if !v.trim().is_empty() => std::sync::Arc::new(DevTrustAny),
        _ => std::sync::Arc::new(PasswordHash),
    }
}

/// The dev/CI check: trust any `(user, workspace)` with no secret (today's password-less login).
/// Opt-in only (`LB_DEV_LOGIN`); the token it enables is still role-scoped, so a dev member ≠ admin.
pub struct DevTrustAny;

#[async_trait]
impl CredentialCheck for DevTrustAny {
    async fn verify(
        &self,
        _node: &Node,
        _workspace: &str,
        _user: &str,
        _secret: &str,
    ) -> Result<(), CredentialRejection> {
        Ok(())
    }
}

/// The real check: argon2 against the per-`(workspace, user)` credential record. A wrong secret or an
/// ABSENT credential is `BadCredential` (no token); a store/hash failure is `CheckFailed` (fail
/// closed). Never distinguishes "wrong password" from "no such credential" to a caller (no oracle —
/// both become the route's opaque `401`).
pub struct PasswordHash;

#[async_trait]
impl CredentialCheck for PasswordHash {
    async fn verify(
        &self,
        node: &Node,
        workspace: &str,
        user: &str,
        secret: &str,
    ) -> Result<(), CredentialRejection> {
        match credential_verify(&node.store, workspace, user, secret).await {
            Ok(CheckOutcome::Ok) => Ok(()),
            Ok(CheckOutcome::BadSecret) | Ok(CheckOutcome::Absent) => {
                Err(CredentialRejection::BadCredential)
            }
            Err(_) => Err(CredentialRejection::CheckFailed),
        }
    }
}
