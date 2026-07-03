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
//! **Provider adapters are the ai-gateway scope's job.** Today the only `Provider` in the tree is the
//! test-only `MockProvider`; no real HTTP adapter exists yet (ai-gateway lists them deferred). So
//! [`build_in_house_model`] has the real construction seam but currently finds no adapter for any
//! configured provider — it logs that and keeps `UnconfiguredModel`. The moment a real adapter lands it
//! slots in behind the same `AiGateway<Provider>` with no change to the boot path or the loop. The
//! unconfigured→configured **swap** is proven for real against `MockProvider` at the test boundary
//! (`crates/host/tests/agent_in_house_wiring_test.rs`).

use std::sync::Arc;

use lb_host::{serve_agent, AgentServer, ErasedModel, Node, RuntimeRegistry, UnconfiguredModel};

/// The node's in-house model config — the `ModelEndpoint` shape (provider / model / api-key-env NAME /
/// base-url), read from env at boot. Present on every node regardless of the external-agent feature
/// (the in-house default agent is symmetric). Names only — the `api_key_env` is the NAME of the env var
/// holding the key, never the key value.
#[derive(Debug, Clone)]
struct InHouseModelConfig {
    provider: String,
    #[allow(dead_code)] // consumed by a real provider adapter (ai-gateway scope), absent today.
    model: String,
    #[allow(dead_code)]
    api_key_env: String,
    #[allow(dead_code)]
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
/// return it as an `Arc<dyn ErasedModel>`. `None` means "no real adapter for this provider (yet)" — the
/// caller keeps [`UnconfiguredModel`], the honest empty state.
///
/// **No real provider adapter exists today** (only the test `MockProvider`; ai-gateway defers the real
/// OpenAI-compatible / local adapters). So every configured provider currently returns `None` here with
/// a clear log — the seam is present and the swap point is explicit, but nothing is faked. When a real
/// adapter lands, add its `match` arm: `"openai" => Some(Arc::new(AiGateway::new(OpenAiProvider::new(
/// resolve_key(&cfg.api_key_env)?, &cfg.model, cfg.base_url.as_deref()))))` — no change anywhere else.
fn build_in_house_model(cfg: &InHouseModelConfig) -> Option<Arc<dyn ErasedModel>> {
    match cfg.provider.as_str() {
        // No real `Provider` adapter is implemented yet (ai-gateway scope owns them). The key would be
        // resolved from `cfg.api_key_env` through the adapter's secrets path here — never logged.
        other => {
            eprintln!(
                "agent: in-house model provider '{other}' has no adapter yet — keeping \
                 UnconfiguredModel (ai-gateway provider adapters are deferred). The seam is wired: a \
                 real AiGateway<Provider> drops in here with no other change."
            );
            None
        }
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
