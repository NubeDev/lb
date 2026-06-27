//! The install record shape (README §6.4, extensions scope).
//!
//! What an extension is durably allowed in a workspace: its `ext_id`, the `version` installed,
//! and `granted` — the `requested ∩ admin_approved` capability strings the host computed at
//! install. The running instance's token carries exactly `granted`; nothing the manifest asked
//! for is live unless an admin approved it.

use serde::{Deserialize, Serialize};

/// The extension tier an install belongs to (README §6.3). `Wasm` is a Tier-1 component (no OS
/// process); `Native` is a Tier-2 supervised sidecar. The lifecycle surface dispatches by this
/// (lifecycle-management scope) so one verb set serves both tiers — no `if tier` in the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Wasm,
    Native,
}

/// The constant `kind` discriminant so `list_installs` can equality-filter every install row in a
/// workspace (the union both tiers share — lifecycle-management scope's `ext.list`).
pub(crate) const KIND: &str = "install";

/// A persisted extension install: the approved-and-granted capability set for `ext_id` in a
/// workspace, plus the durable lifecycle intent. Addressed by `ext_id` (one install per extension
/// per workspace).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Install {
    pub ext_id: String,
    pub version: String,
    /// The granted caps = `requested ∩ admin_approved`, persisted so a restart re-grants
    /// exactly this set without re-asking the admin (extensions scope, the S4 open question).
    pub granted: Vec<String>,
    /// Which tier this install is — the lifecycle surface dispatches on it.
    #[serde(default = "wasm_tier")]
    pub tier: Tier,
    /// Durable **intent**, distinct from running: `disable` sets `false` (do-not-auto-start-on-boot);
    /// the boot reconciler honors `enabled ∧ started`. Defaults `true` for records written before
    /// this field existed (lifecycle-management scope).
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Constant discriminant so `list_installs` selects every row.
    #[serde(default = "install_kind")]
    pub kind: String,
    pub ts: u64,
}

fn wasm_tier() -> Tier {
    Tier::Wasm
}
fn enabled_default() -> bool {
    true
}
fn install_kind() -> String {
    KIND.to_string()
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
            tier: Tier::Wasm,
            enabled: true,
            kind: KIND.to_string(),
            ts,
        }
    }

    /// Set the tier (builder-style) — native installs call this so `ext.list` reports the row's tier.
    pub fn with_tier(mut self, tier: Tier) -> Self {
        self.tier = tier;
        self
    }
}
