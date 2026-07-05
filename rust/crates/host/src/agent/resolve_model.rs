//! `resolve_workspace_model` — the MODEL twin of [`resolve_effective_runtime`](super::resolve_default)
//! (active-agent-wiring scope, Slice 2). Where that answers "which *runtime* id" a run uses, this
//! answers "which live [`ModelAccess`] does this *workspace* route through" — the active definition's
//! `model_endpoint`, built into a real provider adapter, memoized per `(ws, endpoint)` on the node.
//!
//! **Precedence (decided):**
//!   1. the workspace's **active definition** (via [`resolve_active_definition`]) → its `model_endpoint`
//!      → a real provider adapter (the installed [`ModelBuilder`]), keyed by the sealed-secret→env key;
//!   2. else the node-level **in-house `default_model`** (registry) when it is a real provider
//!      ([`ErasedModel::is_configured`]) — the `LB_AGENT_MODEL_*` fallback tier;
//!   3. else the honest [`UnconfiguredModel`] placeholder (no pick, no node model).
//!
//! **Why a builder seam, not a direct `AiGateway<OpenAiCompat>` here (deviation from scope text).**
//! The scope's prose says host builds `AiGateway<OpenAiCompat>` directly. It cannot: `lb-host` must not
//! build-depend on a role crate (rule 1 — roles depend on host, never the reverse; `lb-role-ai-gateway`
//! is a host DEV-dependency only). So the concrete adapter is constructed by an installed
//! [`ModelBuilder`] (the `node` binary — which legitimately depends on both — installs one that returns
//! `AiGateway<OpenAiCompat>`). Host holds only the trait + the erased result. Same realization, correct
//! layering — the resolver, cache, wall, and invalidation are all here; only the concrete `new()` moves
//! to the binary.
//!
//! **Memoization + the wall.** The built model is cached in a `DashMap<(ws, endpoint-hash), Arc<dyn
//! ErasedModel>>` on the [`Node`] so rules/loop don't rebuild an adapter per call. The cache key is
//! `(ws, hash(provider,model,base_url,key-names))` — a rotated key or changed pick has a different hash
//! AND is explicitly invalidated on `agent.config.set` (`invalidate_workspace_model`), so a stale model
//! can never answer after a re-pick. The `ws` is part of the key, so ws-B never reads ws-A's entry.

use std::sync::Arc;

use lb_auth::Principal;

use super::config::get_agent_config;
use super::defs::DefinitionEndpoint;
use super::overlay_endpoint::overlay_config_endpoint;
use super::resolve_definition::resolve_active_definition;
use super::resolve_key::resolve_endpoint_key_host;
use super::runtime::ErasedModel;
use super::unconfigured::UnconfiguredModel;
use crate::boot::Node;

/// The host-owned **model-builder seam** — the node installs one impl that turns a resolved endpoint +
/// its already-resolved key into a live [`ErasedModel`] (an `AiGateway<Provider>`). Host never names a
/// concrete provider; the binary does. `Send + Sync` so it lives on the shared [`Node`]. Returns `None`
/// for a provider it has no adapter for — the caller keeps the honest unconfigured/fallback path.
pub trait ModelBuilder: Send + Sync {
    /// Build a model for `endpoint`, authenticating with the already-resolved `key` (may be empty — the
    /// adapter surfaces its own honest failure). `None` → no adapter for `endpoint.provider`.
    fn build(
        &self,
        endpoint: &DefinitionEndpoint,
        key: Option<&str>,
    ) -> Option<Arc<dyn ErasedModel>>;
}

/// Resolve the live model for `ws` under `caller`. Never panics; never leaks a key (the value goes only
/// to the builder, never logged). See the module precedence.
///
/// The `caller` scopes the definition read (the wall is inherited from `agent_def_get`); the KEY is
/// resolved host-mediated via [`resolve_endpoint_key_host`] (sealed WORKSPACE secret → node env), so a
/// derived `agent:` actor that holds no `secret:*` cap can still resolve its workspace's model key —
/// without widening any user authority (ws-walled, `Workspace`-visibility only).
pub async fn resolve_workspace_model(
    node: &Arc<Node>,
    caller: &Principal,
    ws: &str,
) -> Arc<dyn ErasedModel> {
    // (1) The active definition's endpoint → a real adapter (memoized). A missing pick / missing adapter
    // falls through to the node fallback below — never an error, never a panic.
    if let Ok(def) = resolve_active_definition(node, caller, ws, None).await {
        // Overlay the workspace's live `agent.config.model_endpoint` selection layer onto the
        // definition's preset endpoint. A built-in is read-only, so a workspace keys its *pick* by
        // writing a sealed `api_key_secret` PATH onto `agent.config` (the catalog's "Set model key");
        // without this overlay the resolver read the definition's endpoint only and never saw that key.
        // A store read error is treated as "no config" (best-effort, never a panic).
        let cfg = get_agent_config(&node.store, ws).await.ok().flatten();
        let ep = overlay_config_endpoint(
            &def.model_endpoint,
            cfg.as_ref().and_then(|c| c.model_endpoint.as_ref()),
        );
        let ep = &ep;
        let key = cache_key(ws, ep);
        if let Some(model) = node.workspace_model_cached(&key) {
            return model;
        }
        // Resolve the key host-mediated (sealed ws secret → env → none), then build via the installed
        // adapter seam. Cache the built model under (ws, endpoint-hash) so the next call is lock-free.
        let secret = ep.api_key_secret.as_deref();
        let env = ep.api_key_env.as_deref();
        let resolved = resolve_endpoint_key_host(&node.store, ws, secret, env).await;
        if let Some(builder) = node.model_builder() {
            if let Some(model) = builder.build(ep, resolved.as_deref()) {
                node.workspace_model_insert(key, model.clone());
                return model;
            }
        }
        // A pick whose provider has no adapter falls through to the node fallback — honest, not a fake.
    }

    // (2) The node-level in-house default model, when it is a REAL provider (the `LB_AGENT_MODEL_*`
    // fallback tier). (3) else the honest placeholder.
    let default = node.runtimes().default_model();
    if default.is_configured() {
        default
    } else {
        Arc::new(UnconfiguredModel)
    }
}

/// The memoization key for a workspace endpoint: `(ws, hash(provider, model, base_url, key-NAMES))`.
/// Names only — never a key value. A rotated key that keeps the same NAME still gets a fresh model
/// because `agent.config.set` explicitly invalidates the ws entry; the hash guards the field-change
/// case (a changed provider/model/base_url is a different model without an explicit bust).
fn cache_key(ws: &str, ep: &DefinitionEndpoint) -> (String, u64) {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in [
        ep.provider.as_str(),
        ep.model.as_str(),
        ep.base_url.as_deref().unwrap_or(""),
        ep.api_key_env.as_deref().unwrap_or(""),
        ep.api_key_secret.as_deref().unwrap_or(""),
    ] {
        for b in part.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff; // field separator so ("a","b") ≠ ("ab","")
        h = h.wrapping_mul(0x100000001b3);
    }
    (ws.to_string(), h)
}
