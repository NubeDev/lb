//! [`ApiKeyError`] — the apikey service's error domain (api-keys scope). Management verbs
//! (`create`/`revoke`/`rotate`/`list`/`get`) collapse a denied gate to opaque `Denied`; the auth
//! path's distinct outcomes (`NotFound`/`Revoked`/`Expired`/`Invalid`) are kept separate internally
//! so the gateway can map them ALL to the same opaque `401` (no oracle distinguishing a wrong secret
//! from a revoked or expired key). `Widen`/`BadInput` surface as structured `400`s (the admin needs
//! to know creation was refused for privilege-escalation / bad args).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiKeyError {
    #[error("denied")]
    Denied,
    /// The referenced key id does not exist in this workspace (auth) — collapses to `401` on the
    /// auth path, `404`-shaped on the management path.
    #[error("not found")]
    NotFound,
    #[error("revoked")]
    Revoked,
    #[error("expired")]
    Expired,
    /// The presented secret did not verify (auth path only) — collapses to `401`, never distinguished
    /// from `NotFound`/`Revoked`/`Expired` on the wire.
    #[error("invalid")]
    Invalid,
    /// The key's effective resolved caps widen beyond the creator's own (the privilege-escalation
    /// guard). Surfaces as a `400` so the admin console can explain the refusal.
    #[error("cannot grant a cap the creator lacks: {0}")]
    Widen(String),
    #[error("bad input: {0}")]
    BadInput(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The auth-path outcomes — every one of these becomes the SAME opaque `401` on the wire, so a
/// caller cannot learn whether a key exists, is revoked, is expired, or had the wrong secret.
pub fn is_auth_failure(e: &ApiKeyError) -> bool {
    matches!(
        e,
        ApiKeyError::NotFound | ApiKeyError::Revoked | ApiKeyError::Expired | ApiKeyError::Invalid
    )
}
