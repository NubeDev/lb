//! `resolve_effective_runtime` — the ONE seam that picks which runtime id a run uses when the caller
//! did not name one explicitly (agent-config follow-up: "honor the stored default in `agent.invoke`").
//!
//! **Precedence (single source of truth — no second copy anywhere):**
//!   1. an **explicit** `runtime` argument — returned verbatim (the caller asked for a specific
//!      engine; `RuntimeRegistry::resolve` still errors on a named-unknown id, so a typo is never a
//!      silent downgrade — the decided rule is unchanged);
//!   2. else the workspace's persisted **`agent.config.default_runtime`** — *if the node's registry
//!      currently offers it*;
//!   3. else the **registry default** (the in-house loop) — expressed by returning `None`, which
//!      `RuntimeRegistry::resolve(None)` maps to the default.
//!
//! **Registry drift is fail-open, never fatal.** A stored default naming an id this node no longer
//! offers (feature off, config changed) falls back to the registry default with a `warn!` — a run is
//! never errored because a workspace's stored choice went away (scope "Risks → Registry drift"). A
//! store read that itself fails is likewise treated as "no stored default" (best-effort): the run
//! proceeds on the registry default rather than failing on a transient read hiccup.
//!
//! **This widens nothing.** Resolving the default is pure runtime *selection* — an argument, not a
//! grant. The invoke gate (`mcp:agent.invoke:call`) still fires in `invoke_via_runtime`, and every
//! tool the run calls is still re-checked under the derived `agent ∩ caller` principal. Reading the
//! config here does not require (or confer) `mcp:agent.config.get:call`: this is the host resolving
//! its own dispatch, not a caller-facing read of the record.

use tracing::warn;

use super::config::get_agent_config;
use super::registry::RuntimeRegistry;
use crate::boot::Node;

/// Resolve the effective runtime id for a run. `explicit` is the caller-supplied `runtime` (verbatim
/// from `agent.invoke` / the channel payload). Returns the id to hand to
/// [`RuntimeRegistry::resolve`], or `None` to mean "the registry default".
///
/// - `Some(explicit)` → `Some(explicit)` (explicit wins; the registry validates it).
/// - `None` + a stored default the registry offers → `Some(stored)`.
/// - `None` + a stored default the registry does NOT offer → `None` (registry default) + a `warn!`.
/// - `None` + no stored default (or a store read error) → `None` (registry default).
pub async fn resolve_effective_runtime(
    node: &Node,
    registry: &RuntimeRegistry,
    ws: &str,
    explicit: Option<&str>,
) -> Option<String> {
    // (1) An explicit runtime always wins — returned verbatim so `resolve` can still error on a
    // named-unknown id (a caller that asked for a specific engine is never silently downgraded).
    if let Some(id) = explicit {
        return Some(id.to_string());
    }

    // (2) No explicit runtime → consult the workspace's stored default. A store read failure is
    // treated as "unset" (best-effort — never fail a run on a transient read hiccup).
    let stored = match get_agent_config(&node.store, ws).await {
        Ok(cfg) => cfg.and_then(|c| c.default_runtime),
        Err(e) => {
            warn!(%ws, error = %e, "agent-config read failed; using registry default runtime");
            None
        }
    };

    match stored {
        // A stored default the node currently offers → use it.
        Some(id) if registry_offers(registry, &id) => Some(id),
        // (3a) Stored, but the node no longer offers it (registry drift) → registry default + warn.
        // Fail-open: a run is never errored because a workspace's stored choice went away.
        Some(id) => {
            warn!(
                %ws,
                stored_runtime = %id,
                "workspace default runtime is not offered by this node; falling back to the registry default"
            );
            None
        }
        // (3b) No stored default → registry default.
        None => None,
    }
}

/// Whether `registry` currently offers `id` (same membership check `agent.config.set` validates a
/// write against, so read and write agree on "the node offers this").
fn registry_offers(registry: &RuntimeRegistry, id: &str) -> bool {
    registry.ids().iter().any(|known| known == id)
}
