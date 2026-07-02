//! Deterministic series naming + `scope` arg → `CovScope` parsing for `control-engine.watch`
//! (slice-6 §"Arm / disarm lifecycle"). One responsibility: turn `(appliance, scope-args)` into the
//! series name the frames ride and the `CovScope` the CE client subscribes.
//!
//! **Series-name scheme (chosen + documented):** `ce-cov:{appliance}:{args_hash}` — a `flow:`/`ce-cov:`
//! prefix so it groups under one namespace, the appliance selector so distinct appliances never collide,
//! and a stable hash of the scope so two callers with the SAME `(appliance, scope)` reuse ONE series
//! (arm-on-first / disarm-on-last coalesces them). The name is workspace-SAFE by construction: the host
//! walls every series under `ws/{id}/series/{name}` automatically (motion.rs `series_key`), so two
//! workspaces watching the same appliance never share a bus subject. The hash is FNV-1a over the
//! canonical (sorted) scope — no external dep, deterministic across processes.
//!
//! S7 consumes: open `GET /series/{series}/stream` on the gateway for the returned `series` name; each
//! SSE `event: sample` carries a `Sample` whose `payload` is the frame JSON (`frame.rs`).

use rubix_ce::{CovScope, Uid};
use serde_json::Value;

/// The parsed watch request: the series to publish onto + the CE subscription scope.
#[derive(Debug, Clone)]
pub struct WatchTarget {
    /// The deterministic series name (`ce-cov:{appliance}:{hash}`).
    pub series: String,
    /// The CE COV subscription scope (components / properties / tick).
    pub scope: CovScope,
}

/// Parse `control-engine.watch` args into a [`WatchTarget`]. `appliance` is the resolved selector; the
/// optional `scope` object carries `{ components?: [u32], properties?: [u32], tick_hz?: u32 }`.
#[must_use]
pub fn target(appliance: &str, args: &Value) -> WatchTarget {
    let scope_val = args.get("scope").cloned().unwrap_or(Value::Null);
    let components = uid_list(&scope_val, "components");
    let properties = uid_list(&scope_val, "properties");
    let tick_hz = scope_val
        .get("tick_hz")
        .and_then(Value::as_u64)
        .map(|n| n as u32);

    let hash = args_hash(appliance, &components, &properties, tick_hz);
    let series = format!("ce-cov:{appliance}:{hash:016x}");

    WatchTarget {
        series,
        scope: CovScope {
            components: components.into_iter().map(Uid).collect(),
            properties: properties.into_iter().map(Uid).collect(),
            tick_hz,
        },
    }
}

/// Read a `[u32]` uid list from `scope.<key>`, sorted+deduped so the hash is canonical (order-invariant).
fn uid_list(scope: &Value, key: &str) -> Vec<u32> {
    let mut out: Vec<u32> = scope
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default();
    out.sort_unstable();
    out.dedup();
    out
}

/// A stable FNV-1a hash of the canonical scope. Deterministic across processes/runs (no `RandomState`),
/// so the SAME `(appliance, scope)` always maps to the SAME series — the coalescing key.
fn args_hash(appliance: &str, components: &[u32], properties: &[u32], tick_hz: Option<u32>) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(PRIME);
        }
    };
    mix(appliance.as_bytes());
    mix(b"|c");
    for c in components {
        mix(&c.to_le_bytes());
    }
    mix(b"|p");
    for p in properties {
        mix(&p.to_le_bytes());
    }
    mix(b"|t");
    mix(&tick_hz.unwrap_or(0).to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn series_is_deterministic_and_prefixed() {
        let a = target("plant-1", &json!({ "scope": { "components": [3, 1, 2] } }));
        let b = target("plant-1", &json!({ "scope": { "components": [1, 2, 3] } }));
        // Order-invariant: same scope, different arg order → same series (coalescing key).
        assert_eq!(a.series, b.series);
        assert!(
            a.series.starts_with("ce-cov:plant-1:"),
            "prefixed: {}",
            a.series
        );
    }

    #[test]
    fn distinct_appliance_or_scope_gives_distinct_series() {
        let a = target("plant-1", &json!({ "scope": { "components": [1] } }));
        let b = target("plant-2", &json!({ "scope": { "components": [1] } }));
        let c = target("plant-1", &json!({ "scope": { "properties": [1] } }));
        assert_ne!(a.series, b.series);
        assert_ne!(a.series, c.series);
    }

    #[test]
    fn missing_scope_parses_to_empty_covscope() {
        let t = target("plant-1", &json!({}));
        assert!(t.scope.components.is_empty());
        assert!(t.scope.properties.is_empty());
        assert_eq!(t.scope.tick_hz, None);
    }

    #[test]
    fn scope_maps_components_properties_and_tick() {
        let t = target(
            "plant-1",
            &json!({ "scope": { "components": [10], "properties": [20, 21], "tick_hz": 5 } }),
        );
        assert_eq!(t.scope.components, vec![Uid(10)]);
        assert_eq!(t.scope.properties, vec![Uid(20), Uid(21)]);
        assert_eq!(t.scope.tick_hz, Some(5));
    }
}
