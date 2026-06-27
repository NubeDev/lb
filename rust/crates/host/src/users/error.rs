//! The users service error — `Denied` is opaque (§3.5). `Disabled` is the login-path refusal a
//! disabled user hits when trying to mint (surfaced distinctly so the gateway can `403` it).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UsersError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The user exists but is disabled — the login path refuses to mint a session. Distinct from
    /// `Denied` so the gateway maps it to its own status; carries no secret.
    #[error("user is disabled")]
    Disabled,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
