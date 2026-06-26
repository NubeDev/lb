//! The host registry-service error domain (registry scope). Distinct from `lb_registry::RegistryError`
//! (the crate's verify-layer error): this one spans the *service* — the gate, the source, the cache,
//! and the install. The MCP bridge maps it onto `ToolError` at the edge.

use lb_registry::RegistryError as VerifyError;
use lb_store::StoreError;
use thiserror::Error;

use crate::load::LoadError;

#[derive(Debug, Error)]
pub enum RegistryServiceError {
    /// The MCP/workspace gate refused the caller. Opaque — no signal about whether the artifact
    /// exists (the same discipline as `ToolError::Denied`).
    #[error("denied")]
    Denied,
    /// The artifact failed verification (tampered or signed by an untrusted key). Surfaced distinctly
    /// from `Denied` because it is *not* an authorization failure — the caller may be fully granted
    /// and still be handed a bad artifact; the signature gate is independent of the capability gate.
    #[error("artifact failed verification")]
    Unverified,
    /// The requested `(ext_id, version)` is not in the catalog this workspace can see, or the source
    /// could not be reached (offline with nothing cached). One variant — a private artifact in
    /// another workspace and a genuinely missing one are indistinguishable from here (no leak).
    #[error("artifact not available: {0}")]
    NotAvailable(String),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("load error: {0}")]
    Load(#[from] LoadError),
}

impl From<VerifyError> for RegistryServiceError {
    fn from(e: VerifyError) -> Self {
        match e {
            // Both verify-layer failures are a refusal to trust the bytes — collapse to Unverified.
            VerifyError::Unverified | VerifyError::Malformed(_) => RegistryServiceError::Unverified,
        }
    }
}
