//! [`ExtRow`] — the uniform `ext.list` row across both tiers (lifecycle-management scope). One shape
//! the console renders regardless of tier: `{ext, version, tier, enabled, running, health,
//! restart_count}`. `enabled` is durable intent (the `Install` flag); `running`/`restart_count` are
//! the live runtime truth the host joins in (the `SidecarMap` for native; wasm has no separate
//! process so it `running == enabled`).

use lb_assets::{ExtUi, Install, Tier};
use serde::{Deserialize, Serialize};

/// A single installed extension as the admin console sees it — durable intent joined with live state.
// No `Eq` — carries `ExtUi`, whose option defs hold non-`Eq` `serde_json::Value`. `PartialEq` only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtRow {
    pub ext: String,
    pub version: String,
    /// `"wasm"` or `"native"`.
    pub tier: String,
    /// Durable intent: may it auto-start on boot (lifecycle-management scope).
    pub enabled: bool,
    /// Live: is an instance running right now.
    pub running: bool,
    /// A coarse health string for the row (`"ok"` / `"stopped"` / `"disabled"`).
    pub health: String,
    /// Live restart count (native only; `0` for wasm).
    pub restart_count: u32,
    /// The full **page** this extension contributes to the sidebar — `Some` iff it declared `[ui]`
    /// (ui-federation scope). The shell builds a cap-gated nav slot + mounts the page from this.
    #[serde(default)]
    pub ui: Option<ExtUi>,
    /// The dashboard **widget** tiles this extension contributes — one per `[[widget]]` table it
    /// declared (dashboard-widgets scope). The shell adds each to the widget palette. Empty if none.
    #[serde(default)]
    pub widgets: Vec<ExtUi>,
}

impl ExtRow {
    /// Build a row from the durable `Install` plus the joined live `running`/`restart_count`.
    pub fn from_install(install: &Install, running: bool, restart_count: u32) -> Self {
        let tier = match install.tier {
            Tier::Wasm => "wasm",
            Tier::Native => "native",
        };
        let health = if !install.enabled {
            "disabled"
        } else if running {
            "ok"
        } else {
            "stopped"
        };
        Self {
            ext: install.ext_id.clone(),
            version: install.version.clone(),
            tier: tier.to_string(),
            enabled: install.enabled,
            running,
            health: health.to_string(),
            restart_count,
            ui: install.ui.clone(),
            widgets: install.widgets.clone(),
        }
    }
}
