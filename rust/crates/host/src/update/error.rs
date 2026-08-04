//! [`UpdateError`] — the provider's typed refusal vocabulary, and its one mapping onto [`ToolError`].
//!
//! The vocabulary is **generic** (node-update scope §Seam 1): no variant names a backend, a package
//! format, or a supervisor. A provider whose backend has a richer notion maps it into one of these.
//!
//! The mapping preserves the *reason* on purpose — "a bare refusal tells an operator nothing
//! actionable". Note what it deliberately does NOT do: `Unsupported` is **not** [`ToolError::NotFound`]
//! (a node without a provider must answer "this node cannot update itself", never "no such tool" —
//! the honest `UnconfiguredModel` posture), and `Unauthorized` is **not** [`ToolError::Denied`]
//! (`Denied` is the caps wall's opaque refusal; a caller who passed the wall and was rejected by the
//! *backend* must be told so, or they will chase a capability grant that was never the problem).

use lb_mcp::ToolError;

/// The stable prefix every `Unsupported` refusal carries, so a client can branch on it without
/// parsing prose. Kept as a const because the UI's "this node cannot update itself" card keys on it.
pub const UNSUPPORTED_PREFIX: &str = "update.unsupported";

/// A provider's typed refusal. Generic by construction — a field only one backend could fill is a
/// leak and is refused in review (scope decision 11).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    /// This backend does not offer the operation at all (and the normal answer for a provider whose
    /// enrolment handshake does not exist — see [`UpdateProvider::provision_credential`]).
    ///
    /// [`UpdateProvider::provision_credential`]: super::UpdateProvider::provision_credential
    #[error("unsupported")]
    Unsupported,
    /// The backend refused the credential. `code_required` is the backend asking for a second factor
    /// — the UI re-submits `update.credential.claim {code}` rather than showing a dead end.
    #[error("unauthorized (code_required={code_required})")]
    Unauthorized { code_required: bool },
    /// The named version is not one this backend can reach.
    #[error("unknown version: {version}")]
    NotFound { version: String },
    /// The backend refuses right now and says why — an update already in flight, a quarantined
    /// version, a revision conflict. This is the authoritative answer to a double-click (decision 1).
    #[error("conflict: {reason}")]
    Conflict { reason: String },
    /// Anything else the backend reported. The provider MUST sanitize this — it is operator-visible
    /// and must never carry the credential.
    #[error("backend: {0}")]
    Backend(String),
}

impl From<UpdateError> for ToolError {
    fn from(e: UpdateError) -> Self {
        match e {
            UpdateError::Unsupported => ToolError::Extension(format!(
                "{UNSUPPORTED_PREFIX}: this node has no update provider configured"
            )),
            UpdateError::Unauthorized { code_required } => ToolError::Extension(format!(
                "update.unauthorized: the backend refused this node's credential (code_required={code_required})"
            )),
            // The caller asked for a version that does not exist: a statement about the REQUEST,
            // so `400` (BadInput), not the `NotFound` that means "no such tool".
            UpdateError::NotFound { version } => {
                ToolError::BadInput(format!("update: unknown version `{version}`"))
            }
            UpdateError::Conflict { reason } => {
                ToolError::BadInput(format!("update.conflict: {reason}"))
            }
            UpdateError::Backend(m) => ToolError::Extension(format!("update.backend: {m}")),
        }
    }
}
