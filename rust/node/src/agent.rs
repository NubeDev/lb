//! The node's **in-house agent** wiring (default-agent-wiring scope) — the thin role-aware boot layer
//! (§3.1) that finishes the always-present `"default"` runtime: it builds the in-house model from node
//! config, installs it on the runtime registry, and serves the agent to routed callers. Like
//! `federation.rs` / `control_engine.rs`, no core crate is role-aware; the config-keyed decision lives
//! here in the binary, never an `if cloud`.
//!
//! Two steps, both symmetric (config only, same code on every node):
//!
//! 1. **Wire the model.** From env config (`LB_AGENT_MODEL_*`, mirroring the `ModelEndpoint` shape:
//!    provider / model / api-key-env NAME / base-url), build the real `AiGateway<Provider>` as the
//!    node's [`lb_host::ModelAccess`] and install it via [`lb_host::Node::install_runtimes`], so the
//!    in-house `default` binds the real model instead of [`lb_host::UnconfiguredModel`]. **No model
//!    configured → keep `UnconfiguredModel`** (the honest empty state, not a fake). The API key is an
//!    env NAME resolved through the provider adapter's secrets path — never compiled in or logged.
//!
//! 2. **Serve it.** Build the registry (in-house default over the wired model + the external
//!    `AcpRuntime` entries when the `external-agent` feature is on) and call
//!    [`lb_host::serve_agent`], so a routed `agent.invoke` reaches the finished agent. Mounted from
//!    `main.rs` AFTER the gateway installs its signing key (the same ordering federation/control-engine
//!    use), so the served run's tool callbacks verify.
//!
//! **The real adapter is wired (active-agent-wiring scope).** [`adapter_for`] maps the catalog's
//! providers (`zaicoding`, `openai`, the generic `openai-compat`) to a real `AiGateway<OpenAiCompat>`
//! — the OpenAI chat-completions wire shape. Both the node-level in-house fallback
//! ([`build_in_house_model`]) and the per-workspace [`NodeModelBuilder`] route through it, so "the
//! node's default model" and "a workspace's picked model" never diverge on which providers are real.
//! An unknown provider still keeps `UnconfiguredModel` (the honest empty state). The
//! unconfigured→configured **swap** is proven for real against `MockProvider` at the test boundary
//! (`crates/host/tests/agent_in_house_wiring_test.rs`); the OpenAI-compat adapter itself is tested
//! against a scripted in-process server (`role/ai-gateway/tests/openai_compat_test.rs`).

use std::sync::Arc;

use lb_host::{
    serve_agent, AgentServer, DefinitionEndpoint, ErasedModel, ModelBuilder, Node, RuntimeRegistry,
    UnconfiguredModel,
};
use lb_role_ai_gateway::{AiGateway, OpenAiCompat};

/// The node's in-house model config — the `ModelEndpoint` shape (provider / model / api-key-env NAME /
/// base-url), read from env at boot. Present on every node regardless of the external-agent feature
/// (the in-house default agent is symmetric). Names only — the `api_key_env` is the NAME of the env var
/// holding the key, never the key value.
#[derive(Debug, Clone)]
struct InHouseModelConfig {
    provider: String,
    model: String,
    api_key_env: String,
    base_url: Option<String>,
}

impl InHouseModelConfig {
    /// Read the in-house model config from env. Returns `None` (→ keep `UnconfiguredModel`) unless a
    /// provider is named — the honest "no model configured" state. `LB_AGENT_MODEL_PROVIDER`,
    /// `LB_AGENT_MODEL_MODEL`, `LB_AGENT_MODEL_API_KEY_ENV` (the env NAME), `LB_AGENT_MODEL_BASE_URL`.
    fn from_env() -> Option<Self> {
        let provider = std::env::var("LB_AGENT_MODEL_PROVIDER").ok()?;
        let provider = provider.trim().to_string();
        if provider.is_empty() {
            return None;
        }
        Some(Self {
            provider,
            model: std::env::var("LB_AGENT_MODEL_MODEL").unwrap_or_default(),
            api_key_env: std::env::var("LB_AGENT_MODEL_API_KEY_ENV").unwrap_or_default(),
            base_url: std::env::var("LB_AGENT_MODEL_BASE_URL").ok(),
        })
    }
}

/// Build the in-house [`ModelAccess`](lb_host::ModelAccess) from config, erased for the registry. This
/// is the real wiring seam: match the configured `provider` to a concrete `AiGateway<Provider>` and
/// return it as an `Arc<dyn ErasedModel>`. `None` means "no real adapter for this provider" — the
/// caller keeps [`UnconfiguredModel`], the honest empty state.
///
/// **The OpenAI-compatible adapter is live** ([`adapter_for`]): a configured `zaicoding` / `openai` /
/// `openai-compat` provider builds a real `AiGateway<OpenAiCompat>` — only an UNKNOWN provider returns
/// `None` here (with a clear log). A new wire shape (a provider that does NOT speak OpenAI
/// chat-completions) adds one `match` arm in [`adapter_for`], the ONE adapter-selection point — no
/// change anywhere else. The node-level tier resolves its key from the configured env NAME only; the
/// per-workspace path ([`NodeModelBuilder`]) adds sealed secrets host-mediated
/// (`resolve_endpoint_key_host`) so "test passes" and "run works" can never diverge (agent-catalog
/// test-and-secrets scope). Kept `cfg`-only (env key) so the boot path reads linearly.
fn build_in_house_model(cfg: &InHouseModelConfig) -> Option<Arc<dyn ErasedModel>> {
    // The node-level fallback model (the `LB_AGENT_MODEL_*` tier) is built from the SAME adapter
    // selection the per-workspace [`NodeModelBuilder`] uses (below), so "the node's default model" and
    // "a workspace's picked model" can never diverge on which providers are real. The key is resolved
    // from the configured env NAME (the node-level tier is env-only; the per-ws path adds sealed
    // secrets via `resolve_endpoint_key_host`). Never logged.
    let key = if cfg.api_key_env.is_empty() {
        None
    } else {
        std::env::var(&cfg.api_key_env).ok()
    };
    let model = adapter_for(
        &cfg.provider,
        &cfg.model,
        cfg.base_url.as_deref(),
        key.as_deref(),
    );
    if model.is_none() {
        eprintln!(
            "agent: in-house model provider '{}' has no adapter — keeping UnconfiguredModel (the \
             honest empty state). Known providers: zaicoding, openai-compat, openai.",
            cfg.provider
        );
    }
    model
}

/// Map a `(provider, model, base_url, key)` to a concrete [`ErasedModel`] — the ONE adapter-selection
/// point (active-agent-wiring scope, Slice 1 node-wiring). Both the node-level in-house fallback
/// ([`build_in_house_model`]) and the per-workspace [`NodeModelBuilder`] route here, so a provider is
/// real for one exactly when it is real for the other. Today one wire shape covers the catalog: the
/// OpenAI-compatible chat-completions [`OpenAiCompat`] (`zaicoding`, `openai`, and the generic
/// `openai-compat`). An unknown provider → `None` (the honest unconfigured path — never a fake). The
/// key goes only to the adapter transport; it is never logged.
fn adapter_for(
    provider: &str,
    model: &str,
    base_url: Option<&str>,
    key: Option<&str>,
) -> Option<Arc<dyn ErasedModel>> {
    match provider {
        // Every endpoint currently in the catalog speaks the OpenAI chat-completions shape; the
        // difference is base_url + key + model, never a code branch (§1 symmetric). `zaicoding` is
        // Z.AI's coding endpoint; `openai` is the public API (base_url None → api.openai.com);
        // `openai-compat` is any other server speaking the same shape (ollama/llama.cpp, a proxy).
        "zaicoding" | "openai" | "openai-compat" => {
            let key = key.unwrap_or("").to_string();
            let base_url = base_url.map(str::to_string);
            Some(Arc::new(AiGateway::new(OpenAiCompat::new(
                key,
                model.to_string(),
                base_url,
            ))))
        }
        _ => None,
    }
}

/// The node's [`ModelBuilder`] (active-agent-wiring scope, Slice 2). Installed on the [`Node`] so
/// [`lb_host::resolve_workspace_model`] can build a workspace's picked model without `lb-host`
/// build-depending on this role crate (rule 1 — host holds only the trait; the binary names the
/// provider). It delegates to the same [`adapter_for`] the in-house fallback uses. The key arrives
/// already resolved (host-mediated sealed secret → env) — this builder never touches secrets or logs.
struct NodeModelBuilder;

impl ModelBuilder for NodeModelBuilder {
    fn build(
        &self,
        endpoint: &DefinitionEndpoint,
        key: Option<&str>,
    ) -> Option<Arc<dyn ErasedModel>> {
        adapter_for(
            &endpoint.provider,
            &endpoint.model,
            endpoint.base_url.as_deref(),
            key,
        )
    }
}

/// The in-house model to install: the real `AiGateway<Provider>` when configured AND an adapter exists,
/// else the honest [`UnconfiguredModel`]. Kept as one function so the boot path reads linearly.
fn in_house_model() -> Arc<dyn ErasedModel> {
    match InHouseModelConfig::from_env()
        .as_ref()
        .and_then(build_in_house_model)
    {
        Some(model) => {
            println!("agent: in-house model configured");
            model
        }
        None => Arc::new(UnconfiguredModel),
    }
}

/// The capabilities the served agent actor holds (its half of `agent_caps ∩ caller.caps`). Broad on
/// purpose — the effective grant is always intersected with the CALLER's caps at the wall, so this is a
/// ceiling, never a widening. A minimal node just has no remote callers. Read from
/// `LB_AGENT_CAPS` (comma-separated) or defaults to the platform-tool surface the in-house agent uses.
fn agent_caps() -> Vec<String> {
    if let Ok(raw) = std::env::var("LB_AGENT_CAPS") {
        let caps: Vec<String> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if !caps.is_empty() {
            return caps;
        }
    }
    // Default ceiling: the invoke gate + the reachable-tools catalog + the host-native platform verbs
    // the in-house agent is meant to reach (memory / assets / series / query). Each is still
    // intersected with the caller and re-checked per call at `call_tool`.
    vec![
        "mcp:agent.invoke:call".into(),
        "mcp:tools.catalog:call".into(),
        "mcp:agent.memory.list:call".into(),
        "mcp:agent.memory.get:call".into(),
        "mcp:agent.memory.set:call".into(),
        "mcp:assets.get_doc:call".into(),
        "mcp:series.latest:call".into(),
        "mcp:series.find:call".into(),
        "mcp:query.run:call".into(),
    ]
}

/// Mount the in-house agent on `node`: install the runtime registry (in-house default over the wired
/// model + external entries when the feature is on) and serve routed invocations. Called from `main.rs`
/// after the gateway installs its signing key (the federation/control-engine ordering) so a served
/// run's tool callbacks verify. Returns the live [`AgentServer`]; the caller keeps it alive for the
/// node's lifetime (dropping it stops serving). A serve failure is logged, not fatal — the node still
/// runs the in-channel `/agent` path off the installed registry.
pub async fn mount(node: Arc<Node>) -> Option<AgentServer> {
    // 1. WIRE THE MODEL: build the registry (in-house default over the wired model), add the external
    //    `AcpRuntime` entries when the `external-agent` feature is on, then install it on the node. This
    //    replaces the boot-time default-only `UnconfiguredModel` registry with the configured one — the
    //    seam is the registry, not a code branch (unconfigured vs configured is config only).
    let mut registry = RuntimeRegistry::with_default(in_house_model());
    crate::external_agent::register_external(&mut registry);
    let ids = registry.ids();
    node.install_runtimes(registry);
    println!("agent: runtimes installed = {ids:?}");

    // 1b. INSTALL THE MODEL BUILDER (active-agent-wiring #2): the per-workspace model resolver in host
    //     builds a workspace's picked `model_endpoint` through THIS builder (host names no provider —
    //     rule 1). Without it, `resolve_workspace_model` falls back to the node model / placeholder.
    node.install_model_builder(Arc::new(NodeModelBuilder));

    // 2. SERVE IT: declare the routed `agent/invoke` queryable so an edge's `agent.invoke` reaches this
    //    node's finished agent (the serve-wiring TODO, now closed). The registry the server resolves
    //    against is the SAME one just installed (read back via `node.runtimes()`), so routed and
    //    in-channel runs drive the identical registry.
    match serve_agent(node.clone(), node.runtimes(), agent_caps()).await {
        Ok(server) => {
            println!("agent: serving routed agent.invoke");
            Some(server)
        }
        Err(e) => {
            eprintln!("agent: serve_agent failed: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(provider: &str, model: &str) -> DefinitionEndpoint {
        DefinitionEndpoint {
            provider: provider.into(),
            model: model.into(),
            api_key_env: None,
            api_key_secret: None,
            base_url: None,
        }
    }

    /// The catalog's OpenAI-compatible providers each map to a real, CONFIGURED adapter (never the
    /// unconfigured placeholder) — the regression against silently dropping a provider from the match.
    #[test]
    fn known_providers_build_a_configured_model() {
        for provider in ["zaicoding", "openai", "openai-compat"] {
            let model = adapter_for(provider, "some-model", None, Some("k"))
                .unwrap_or_else(|| panic!("{provider} must have an adapter"));
            assert!(
                model.is_configured(),
                "{provider} builds a real (configured) provider adapter"
            );
        }
    }

    /// An unknown provider → `None`: the honest unconfigured path (the caller keeps `UnconfiguredModel`
    /// / the node fallback), never a fake. Both `build_in_house_model` and `NodeModelBuilder` rely on it.
    #[test]
    fn an_unknown_provider_has_no_adapter() {
        assert!(adapter_for("mystery-llm", "m", None, Some("k")).is_none());
        assert!(NodeModelBuilder
            .build(&endpoint("mystery-llm", "m"), Some("k"))
            .is_none());
        // And a known provider through the builder is configured (the seam host installs).
        assert!(NodeModelBuilder
            .build(&endpoint("zaicoding", "glm-4.6"), Some("k"))
            .map(|m| m.is_configured())
            .unwrap_or(false));
    }
}
