//! Resolve a graph verb's `appliance` selector → the CE **base** to connect to locally (control-engine
//! scope, S4). This runs on the OWNING node: any `control-engine.*` graph call that reaches this
//! sidecar is for an appliance this node serves (the host router forwarded a remote appliance's call to
//! its owner — symmetry is the host's job, see `host.rs`). So resolution's only job is to turn the
//! selector into the localhost CE base.
//!
//! Resolution order:
//!   1. Look up `ce_appliance:{selector}` in THIS workspace (via the `store.query` callback). Found →
//!      use its `base`. Absent in this workspace → **not-found** (the isolation wall: a ws-B caller
//!      naming a ws-A appliance, or an unknown id, is indistinguishable from non-existent — no leak).
//!   2. Two literal-base fallbacks, both narrow and safe:
//!      - the EMPTY selector → the canonical local CE (no lookup);
//!      - a selector reached when the STORE ITSELF is unreachable (no gateway/token — the real-engine
//!        dev tier, which has no registry) → treat the selector as a literal `host:port`. This is a
//!        *transport* failure of the registry, not a "registry says no", so it cannot leak isolation:
//!        with a real gateway present, a `store.query` for an unknown/other-ws id returns `Ok(None)`
//!        → **not-found**, never a literal base. So a ws-B caller with a real store can never reach a
//!        ws-A CE by passing its host:port.

use crate::appliance::store;
use crate::host::{HostCtx, HostError};

/// The outcome of resolving an appliance selector: the CE base to bind a local client to.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The CE origin/base (`host:port` or `http://host:port`) the sidecar connects to on localhost.
    pub base: String,
}

/// Resolve `selector` → the local CE base, reading the `ce_appliance` registry (workspace-walled).
///
/// - empty selector → the canonical local CE (no registry lookup; S3/back-compat).
/// - a known appliance in this workspace → its recorded `base`.
/// - an unknown/other-workspace appliance → [`HostError::NotFound`] (the isolation not-found).
pub async fn resolve(host: &HostCtx, selector: &str) -> Result<Resolved, HostError> {
    let sel = selector.trim();
    if sel.is_empty() {
        // Canonical local CE — the sidecar's `Registry` maps an empty base to 127.0.0.1:CANONICAL_PORT.
        return Ok(Resolved {
            base: String::new(),
        });
    }
    match store::get(host, sel).await {
        // Known appliance in THIS workspace → its recorded base.
        Ok(Some(appliance)) => Ok(Resolved {
            base: appliance.base,
        }),
        // Reachable registry, absent id (or another workspace's id) → not-found (the isolation wall).
        Ok(None) => Err(HostError::NotFound),
        // The registry store itself is unreachable (no gateway/token — the real-engine dev tier). Fall
        // back to the literal selector as a base. NOT reachable with a real gateway, so no isolation leak.
        Err(HostError::Callback(_)) => Ok(Resolved {
            base: sel.to_string(),
        }),
        Err(other) => Err(other),
    }
}
