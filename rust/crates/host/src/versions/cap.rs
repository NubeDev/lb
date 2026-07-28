//! The **adjustable ring cap** — a const default, a per-workspace override, a per-kind override,
//! node-clamped (`docs/scope/versions/entity-version-history-scope.md`, "The adjustable cap").
//!
//! Resolution order: per-kind override → workspace cap → the const. Every result is clamped to
//! `MIN..=MAX` **at the node**, so a bad stored value (a hand-edited record, an older node's write)
//! can never make the ring unbounded or empty. A lowered cap applies on the next capture —
//! `capped_insert` trims to whatever cap it is handed — so there is no reaper job to run or to
//! forget to run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use lb_store::{read, Store, StoreError};

/// The house default (the `DEFAULT_FINISHED_RUN_CAP` / undo `DEFAULT_DEPTH_CAP` pattern).
pub const DEFAULT_VERSION_CAP: usize = 20;
/// The node's hard floor. 0 would mean "keep nothing", which is a disable switch wearing a cap's
/// clothes; disabling history is not a per-workspace setting.
pub const MIN_VERSION_CAP: usize = 1;
/// The node's hard ceiling. Snapshots are full records; the scope accepts `size × N × entities` only
/// because N is bounded here.
pub const MAX_VERSION_CAP: usize = 100;

/// The store table + record id the per-workspace override lives at. Reserved (host-owned).
pub const CONFIG_TABLE: &str = "versions_config";
pub const CONFIG_ID: &str = "default";

/// The per-workspace cap override. Both fields are optional/partial: an absent `cap` means "use the
/// const", and `per_kind` holds only the kinds an admin actually pinned.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<usize>,
    #[serde(default)]
    pub per_kind: BTreeMap<String, usize>,
}

/// The cap in force for `kind` — pure, so the resolution order is unit-testable without a store.
pub fn resolve_cap(cfg: &VersionsConfig, kind: &str) -> usize {
    let raw = cfg
        .per_kind
        .get(kind)
        .copied()
        .or(cfg.cap)
        .unwrap_or(DEFAULT_VERSION_CAP);
    clamp_cap(raw)
}

/// Clamp any cap to the node's `MIN..=MAX`. Applied on read AND on write, so a value that predates a
/// clamp change is still safe to act on.
pub fn clamp_cap(cap: usize) -> usize {
    cap.clamp(MIN_VERSION_CAP, MAX_VERSION_CAP)
}

/// Read workspace `ws`'s override. A missing (or undecodable) record resolves to the default config
/// rather than an error: history must keep working on a workspace nobody has configured, and a
/// corrupt config row must degrade to the safe default, not disable capture.
pub async fn read_config(store: &Store, ws: &str) -> Result<VersionsConfig, StoreError> {
    let raw: Option<Value> = read(store, ws, CONFIG_TABLE, CONFIG_ID).await?;
    Ok(raw
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(cap: Option<usize>, per_kind: &[(&str, usize)]) -> VersionsConfig {
        VersionsConfig {
            cap,
            per_kind: per_kind
                .iter()
                .map(|(k, v)| ((*k).to_string(), *v))
                .collect(),
        }
    }

    #[test]
    fn resolution_is_per_kind_then_workspace_then_const() {
        let c = cfg(Some(50), &[("dashboard", 5)]);
        assert_eq!(resolve_cap(&c, "dashboard"), 5, "per-kind wins");
        assert_eq!(resolve_cap(&c, "flow"), 50, "workspace cap is next");
        assert_eq!(
            resolve_cap(&VersionsConfig::default(), "flow"),
            DEFAULT_VERSION_CAP,
            "the const is the floor of the chain"
        );
    }

    #[test]
    fn every_layer_is_clamped_at_the_node() {
        assert_eq!(resolve_cap(&cfg(Some(0), &[]), "flow"), MIN_VERSION_CAP);
        assert_eq!(resolve_cap(&cfg(Some(9_999), &[]), "flow"), MAX_VERSION_CAP);
        assert_eq!(
            resolve_cap(&cfg(None, &[("rule", 100_000)]), "rule"),
            MAX_VERSION_CAP,
            "a per-kind override is clamped too — it is the layer an admin types into"
        );
    }
}
