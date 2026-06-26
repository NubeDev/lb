//! The install record shape (README §6.4, extensions scope).
//!
//! What an extension is durably allowed in a workspace: its `ext_id`, the `version` installed,
//! and `granted` — the `requested ∩ admin_approved` capability strings the host computed at
//! install. The running instance's token carries exactly `granted`; nothing the manifest asked
//! for is live unless an admin approved it.

use serde::{Deserialize, Serialize};

/// A persisted extension install: the approved-and-granted capability set for `ext_id` in a
/// workspace. Addressed by `ext_id` (one install per extension per workspace at S4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Install {
    pub ext_id: String,
    pub version: String,
    /// The granted caps = `requested ∩ admin_approved`, persisted so a restart re-grants
    /// exactly this set without re-asking the admin (extensions scope, the S4 open question).
    pub granted: Vec<String>,
    pub ts: u64,
}

impl Install {
    pub fn new(
        ext_id: impl Into<String>,
        version: impl Into<String>,
        granted: Vec<String>,
        ts: u64,
    ) -> Self {
        Self {
            ext_id: ext_id.into(),
            version: version.into(),
            granted,
            ts,
        }
    }
}
