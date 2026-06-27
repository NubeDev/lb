//! [`ExtRow`] — the uniform `ext.list` row across both tiers (lifecycle-management scope). One shape
//! the console renders regardless of tier: `{ext, version, tier, enabled, running, health,
//! restart_count}`. `enabled` is durable intent (the `Install` flag); `running`/`restart_count` are
//! the live runtime truth the host joins in (the `SidecarMap` for native; wasm has no separate
//! process so it `running == enabled`).

use lb_assets::{Install, Tier};
use serde::{Deserialize, Serialize};

/// A single installed extension as the admin console sees it — durable intent joined with live state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        }
    }
}
