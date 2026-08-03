//! The **credential check** — the ONE seam `/auth/login` runs before minting (email-login scope). It
//! verifies a person's ONE global password (all workspaces) against the `identity_credential` record —
//! the Slack model. `/auth/login` resolves email→sub, then calls [`GlobalCredentialCheck::verify`]; a
//! bad/absent secret is a `401` with no token, uniform with an unknown email (no enumeration oracle).
//!
//! There is no per-workspace credential check any more: the legacy `POST /login {user, workspace,
//! secret}` and its `CredentialCheck` seam were deleted in the pre-production legacy sweep, so this is
//! the only door a password is ever presented at (machines carry an lb API key instead).
//!
//! Two impls, selected by `LB_DEV_LOGIN`:
//!   - [`GlobalPasswordHash`] — the real check: argon2 against the stored global hash
//!     (`lb_host::global_credential_verify`, which is itself timing-uniform on an unknown identity).
//!     A wrong secret AND an absent credential both `401` (no password ⇒ identity unproven).
//!   - [`GlobalDevTrustAny`] — password-less, dev/CI ONLY, opt-in via `LB_DEV_LOGIN`. A release build
//!     without the flag selects `GlobalPasswordHash` and demands a real password.
//!
//! The minted token is role-correct regardless (the `/auth/login` route unions `resolve_caps`), so
//! dev convenience never re-opens the escalation login-hardening closed.

use async_trait::async_trait;
use lb_host::{global_credential_verify, GlobalCredentialCheck as CheckOutcome, Node};

/// The env var that opts a node into the password-less dev login. Set (to any non-empty value) for
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

/// The pluggable global credential check `/auth/login` runs before minting. One method: prove
/// `(sub, secret)` — the person's global password — before a token is issued. `Ok(())` allows the
/// mint; any `Err` is a `401`. No workspace: the global credential is workspace-independent (the
/// workspace is chosen AFTER authentication).
#[async_trait]
pub trait GlobalCredentialCheck: Send + Sync {
    async fn verify(&self, node: &Node, sub: &str, secret: &str)
        -> Result<(), CredentialRejection>;
}

/// Select the global credential check from the environment: `LB_DEV_LOGIN` set → `GlobalDevTrustAny`
/// (dev/CI), unset → `GlobalPasswordHash` (production).
pub fn global_credential_check_from_env() -> std::sync::Arc<dyn GlobalCredentialCheck> {
    match std::env::var(DEV_LOGIN_ENV) {
        Ok(v) if !v.trim().is_empty() => std::sync::Arc::new(GlobalDevTrustAny),
        _ => std::sync::Arc::new(GlobalPasswordHash),
    }
}

/// The dev/CI check: trust any resolved `sub` with no secret. Opt-in only (`LB_DEV_LOGIN`); the token
/// it enables is still role-scoped, so a dev member ≠ admin.
pub struct GlobalDevTrustAny;

#[async_trait]
impl GlobalCredentialCheck for GlobalDevTrustAny {
    async fn verify(
        &self,
        _node: &Node,
        _sub: &str,
        _secret: &str,
    ) -> Result<(), CredentialRejection> {
        Ok(())
    }
}

/// The real check: argon2 against the person's global credential. A wrong secret OR an absent
/// credential is `BadCredential` (no token); a store/hash failure is `CheckFailed` (fail closed).
/// Never distinguishes "wrong password" from "no credential" to a caller (both → the route's opaque
/// `401`); the underlying host verify is also timing-uniform on an unknown identity.
pub struct GlobalPasswordHash;

#[async_trait]
impl GlobalCredentialCheck for GlobalPasswordHash {
    async fn verify(
        &self,
        node: &Node,
        sub: &str,
        secret: &str,
    ) -> Result<(), CredentialRejection> {
        match global_credential_verify(&node.store, sub, secret).await {
            Ok(CheckOutcome::Ok) => Ok(()),
            Ok(CheckOutcome::BadSecret) | Ok(CheckOutcome::Absent) => {
                Err(CredentialRejection::BadCredential)
            }
            Err(_) => Err(CredentialRejection::CheckFailed),
        }
    }
}
