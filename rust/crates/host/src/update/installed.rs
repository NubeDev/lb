//! [`InstalledUpdate`] — what the boot layer installs on the [`Node`](crate::Node) so the `update.*`
//! verbs can reach the embedder's provider long after `boot_full` returned.
//!
//! It carries three things the verbs need and nothing else: the config (provider + the secret PATH +
//! the env NAME), the **boot workspace** the credential is sealed into, and the seal mutex that
//! serializes first-use auto-enrolment so a concurrent double-trigger mints exactly once.

use std::sync::Arc;

use super::provider::UpdateConfig;

/// The node's installed update seam.
pub struct InstalledUpdate {
    /// The embedder's config, shared so a verb can hold the provider across an await.
    pub cfg: Arc<UpdateConfig>,
    /// The node's BOOT workspace — where the credential is sealed. Deliberately not the caller's:
    /// an update is not workspace data, so one node credential rather than one per workspace
    /// (scope decision 3).
    pub boot_workspace: String,
    /// Serializes auto-enrolment. The loser of a concurrent double-trigger re-resolves under this
    /// lock and finds the winner's sealed secret, so `provision_credential` runs exactly once.
    pub seal_lock: tokio::sync::Mutex<()>,
}

impl InstalledUpdate {
    pub fn new(cfg: UpdateConfig, boot_workspace: String) -> Self {
        Self {
            cfg: Arc::new(cfg),
            boot_workspace,
            seal_lock: tokio::sync::Mutex::new(()),
        }
    }
}
