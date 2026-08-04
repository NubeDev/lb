//! [`UpdateProvider`] + [`UpdateConfig`] — the embedder-filled seam behind the `update.*` family.
//!
//! **lb performs no update and stores no artifact.** The mechanism — a supervisor, an orchestrator,
//! a package manager — is the embedder's, exactly as the identity seam split machine identity
//! (rule 10). lb owns the vocabulary, the wall, the audit trail, and the credential's custody.
//!
//! The config carries a secret **PATH** and an env var **NAME**, never a value — the same custody
//! discipline `AgentModelConfig::api_key_env` and `EmailTransport` already follow.

use std::sync::Arc;

use async_trait::async_trait;

use super::error::UpdateError;
use super::model::{Accepted, AvailableVersion, UpdateEvent, UpdateStatus};

/// The per-call context handed to the provider: the resolved credential (opaque to lb, never logged)
/// and the audited actor whose verb triggered this call.
#[derive(Debug, Clone)]
pub struct UpdateCx {
    /// The credential lb resolved for this call (sealed secret → env NAME → `None`). The provider
    /// receives an opaque string and never touches the store.
    pub credential: Option<String>,
    /// The subject of the principal that called the verb — for the backend's own audit trail.
    pub actor: String,
}

/// The embedder's update mechanism. Every method is a question lb asks and reports verbatim.
///
/// **`apply` returns accepted, never done.** A backend whose apply call is synchronous through its
/// own health gate (minutes, ending in this process's death) must be driven through an async accept:
/// surface the early typed refusals (unknown version, quarantined, revision conflict) and then
/// detach. A backend that cannot offer an async accept is driven fire-and-validate — refusals are
/// read synchronously and the connection being severed after acceptance is the expected outcome.
#[async_trait]
pub trait UpdateProvider: Send + Sync {
    /// Running version, backend labels, in-flight tx, last outcome, quarantine, self-identity check.
    /// The `credential` field of the returned status is **overwritten by lb** with its own custody
    /// view — a provider cannot report a credential state lb did not resolve.
    async fn status(&self, cx: &UpdateCx) -> Result<UpdateStatus, UpdateError>;

    /// The versions this backend can reach, **in the provider's own order**. lb never sorts them.
    async fn check(&self, cx: &UpdateCx) -> Result<Vec<AvailableVersion>, UpdateError>;

    /// Accept an update to `version`. Returns a tx id — not a verdict (see the trait note).
    async fn apply(&self, cx: &UpdateCx, version: &str) -> Result<Accepted, UpdateError>;

    /// Accept a rollback to whatever the backend considers the previous good state.
    async fn rollback(&self, cx: &UpdateCx) -> Result<Accepted, UpdateError>;

    /// The backend's own journal, newest-first, at most `limit` rows.
    async fn history(&self, cx: &UpdateCx, limit: u32) -> Result<Vec<UpdateEvent>, UpdateError>;

    /// Drive the backend's own enrolment handshake and return the plaintext credential **to lb, not
    /// to the caller**. [`UpdateError::Unsupported`] is a normal answer — it degrades to "paste it
    /// instead", never an error page. `code` carries a second factor when the backend asked for one
    /// via [`UpdateError::Unauthorized`]`{code_required: true}`.
    async fn provision_credential(&self, code: Option<&str>) -> Result<String, UpdateError>;

    /// A cheap authenticated probe — refuse a wrong credential BEFORE sealing it. A store write that
    /// has not been proven to work is a trap set for the next outage (scope decision 4).
    async fn verify_credential(&self, candidate: &str) -> Result<(), UpdateError>;
}

/// What an embedder puts on `BootConfig.update` to give this node an update surface.
///
/// `None` on the boot config ⇒ `update.status` answers `{"supported": false}` and every other verb
/// is `Unsupported`. Byte-for-byte prior behaviour for every existing embedder.
#[derive(Clone)]
#[non_exhaustive]
pub struct UpdateConfig {
    /// The mechanism. lb calls it; it never calls lb.
    pub provider: Arc<dyn UpdateProvider>,
    /// Secret **PATH** (never a value), e.g. `"update/credential"`. Sealed at
    /// `secret:{boot_workspace}:{path}` and stamped owner = the node host principal, so no principal
    /// — admin included — can read it back through `secret.get`. `None` ⇒ nothing is ever sealed
    /// (and first-use auto-enrolment cannot run: there is nowhere to put the result).
    pub credential_secret: Option<String>,
    /// Env var **NAME** (never a value) — the fallback consulted when nothing is sealed.
    pub credential_env: Option<String>,
}

impl UpdateConfig {
    /// The minimal config: a provider and no credential custody at all.
    pub fn new(provider: Arc<dyn UpdateProvider>) -> Self {
        Self {
            provider,
            credential_secret: None,
            credential_env: None,
        }
    }
}

impl std::fmt::Debug for UpdateConfig {
    /// Prints the PATH and the NAME — both are non-secret by construction — and never a value.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateConfig")
            .field("provider", &"<dyn UpdateProvider>")
            .field("credential_secret", &self.credential_secret)
            .field("credential_env", &self.credential_env)
            .finish()
    }
}
