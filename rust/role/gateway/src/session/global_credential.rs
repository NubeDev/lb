//! The **global credential check** — the seam `/auth/login` runs before minting (email-login scope),
//! the global-identity analogue of the per-ws [`CredentialCheck`](crate::session::credential). Where
//! that one verifies a `(workspace, user)` password for the legacy `/login`, THIS one verifies a
//! person's ONE global password (all workspaces) against the `identity_credential` record — the Slack
//! model. `/auth/login` resolves email→sub, then calls [`GlobalCredentialCheck::verify`]; a
//! bad/absent secret is a `401` with no token, uniform with an unknown email (no enumeration oracle).
//!
//! Two impls, selected by the SAME `LB_DEV_LOGIN` env as the per-ws seam (resolved open question):
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

pub use crate::session::credential::{CredentialRejection, DEV_LOGIN_ENV};

/// The pluggable global credential check `/auth/login` runs before minting. One method: prove
/// `(sub, secret)` — the person's global password — before a token is issued. `Ok(())` allows the
/// mint; any `Err` is a `401`. No workspace: the global credential is workspace-independent (the
/// workspace is chosen AFTER authentication).
#[async_trait]
pub trait GlobalCredentialCheck: Send + Sync {
    async fn verify(&self, node: &Node, sub: &str, secret: &str)
        -> Result<(), CredentialRejection>;
}

/// Select the global credential check from the environment — the SAME switch as the per-ws seam:
/// `LB_DEV_LOGIN` set → `GlobalDevTrustAny` (dev/CI), unset → `GlobalPasswordHash` (production).
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
