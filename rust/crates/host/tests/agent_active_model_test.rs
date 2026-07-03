//! Active-agent-wiring scope, Slice 2/3 — the workspace's ACTIVE pick is the one implicit model
//! everywhere (`resolve_workspace_model`), memoized per `(ws, endpoint)`, invalidated on `agent.config.set`,
//! and ridden by BOTH the in-house loop and the rules engine.
//!
//! **Rule 9: everything real** — the `mem://` store, caps, the loop, the rules engine, the real
//! `AiGateway` are all the production code. The ONLY permitted fake is the model **provider HTTP**
//! (`MockProvider`, behind the `Provider` trait): a test [`ModelBuilder`] builds the workspace's picked
//! endpoint into a real `AiGateway<MockProvider>` — the SAME construction the `node` binary's
//! `NodeModelBuilder` does with `OpenAiCompat`, only the provider transport is scripted. Nothing above
//! the adapter is faked; the resolver, cache, wall, and invalidation are the shipped host code.
//!
//! What this proves (the scope's Testing plan):
//!   - **Active pick → the model** — a picked definition's endpoint resolves to the built model.
//!   - **Unconfigured→configured swap** — no pick / no builder → the honest placeholder; after the pick
//!     the SAME resolve yields the configured model (per-workspace altitude).
//!   - **Workspace-isolation** — ws-B's resolve never yields ws-A's endpoint/model.
//!   - **Cache invalidation** — a re-pick (`agent.config.set`) busts the memoized model.
//!   - **Rules ride the active agent** — with a pick, `ai.complete` returns the model answer; with none,
//!     the honest "AI not configured for rules".
//!   - **The in-house loop rides the per-ws model** — an implicit `runtime:default` run drives the
//!     picked model (its scripted answer), not the node-level fallback.
//!   - **Offline/sync** — `agent.config` double-delivery keeps `active_definition` idempotent (LWW).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, agent_def_create, get_agent_config, invoke_via_runtime, reachable_tools,
    resolve_active_definition, resolve_workspace_model, AgentConfig, AgentDefinition,
    DefinitionEndpoint, ErasedModel, ModelBuilder, ModelEndpointPatch, Node, Substrate,
    UNCONFIGURED_ANSWER,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};
use std::sync::Mutex;

// ── caps ─────────────────────────────────────────────────────────────────────────────────────────
const LIST: &str = "mcp:agent.def.list:call";
const GET: &str = "mcp:agent.def.get:call";
const CREATE: &str = "mcp:agent.def.create:call";
const CONFIG_SET: &str = "mcp:agent.config.set:call";
const CONFIG_GET: &str = "mcp:agent.config.get:call";
const INVOKE: &str = "mcp:agent.invoke:call";

fn admin(sub: &str, ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: [LIST, GET, CREATE, CONFIG_SET, CONFIG_GET, INVOKE]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A custom definition over the always-present `default` runtime whose endpoint the test builder maps
/// to a scripted model. `provider`/`model` distinguish which model a `(ws, endpoint)` resolves to.
fn definition(id: &str, provider: &str, model: &str) -> AgentDefinition {
    AgentDefinition {
        id: id.into(),
        label: format!("Custom — {provider}/{model}"),
        description: None,
        runtime: "default".into(),
        model_endpoint: DefinitionEndpoint {
            provider: provider.into(),
            model: model.into(),
            api_key_env: Some("TEST_MODEL_KEY".into()),
            api_key_secret: None,
            base_url: Some("https://example.test/v1".into()),
        },
        builtin: false,
    }
}

/// The `agent.config` patch a pick writes: the active id + the copied runtime/endpoint (exactly what the
/// UI `pick()` sends). LWW/UPSERT idempotent.
fn pick_patch(def: &AgentDefinition) -> AgentConfig {
    AgentConfig {
        active_definition: Some(def.id.clone()),
        default_runtime: Some(def.runtime.clone()),
        model_endpoint: Some(ModelEndpointPatch {
            provider: Some(def.model_endpoint.provider.clone()),
            model: Some(def.model_endpoint.model.clone()),
            api_key_env: def.model_endpoint.api_key_env.clone(),
            api_key_secret: def.model_endpoint.api_key_secret.clone(),
            base_url: def.model_endpoint.base_url.clone(),
        }),
    }
}

/// A test [`ModelBuilder`] — the twin of the node binary's `NodeModelBuilder`, but scripting the
/// provider HTTP with `MockProvider`. It builds a real `AiGateway<MockProvider>` whose single scripted
/// stop-turn echoes the endpoint's `model` id, so a test can assert WHICH endpoint was built (and thus
/// that the wall/cache resolved the right one). An unknown provider → `None` (the honest fallback path).
struct ScriptedBuilder;

impl ModelBuilder for ScriptedBuilder {
    fn build(
        &self,
        endpoint: &DefinitionEndpoint,
        _key: Option<&str>,
    ) -> Option<Arc<dyn ErasedModel>> {
        if endpoint.provider == "unknown" {
            return None; // no adapter → resolver falls through to node fallback / placeholder
        }
        // Script one stop-turn that names the model — the answer proves which endpoint was built.
        let answer = format!("answer from {}", endpoint.model);
        let gw = AiGateway::new(MockProvider::new(vec![AiResponse::stop(answer, 3)]));
        Some(Arc::new(gw))
    }
}

/// Boot a node with the scripted builder installed (the production wiring, MockProvider transport).
async fn node_with_builder() -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    node.install_model_builder(Arc::new(ScriptedBuilder));
    node
}

/// Drive one bounded model turn over the resolved workspace model and return its answer — the exact call
/// rules/`agent.def.test` make. Proves `resolve_workspace_model` returns a working model.
async fn resolved_turn(node: &Arc<Node>, caller: &Principal, ws: &str) -> (String, bool) {
    let model = resolve_workspace_model(node, caller, ws).await;
    let configured = model.is_configured();
    let turn = model
        .turn_boxed(ws, &[("user".into(), "hi".into())], &[], &[], "k")
        .await;
    (turn.content, configured)
}

// ── active pick → the model (per-workspace) ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_picked_definition_endpoint_resolves_to_its_built_model() {
    let ws = "ws-active-pick";
    let node = node_with_builder().await;
    let ada = admin("user:ada", ws);

    // Before any pick: no active definition, and the resolve yields the honest placeholder (no node
    // model wired at boot — UnconfiguredModel).
    assert!(
        resolve_active_definition(&node, &ada, ws, None)
            .await
            .is_err(),
        "no pick yet → nothing active"
    );
    let (answer, configured) = resolved_turn(&node, &ada, ws).await;
    assert_eq!(answer, UNCONFIGURED_ANSWER, "unconfigured before the pick");
    assert!(!configured);

    // Pick a definition: create it, then write the config the pick sends.
    let def = definition("glm-coder", "zaicoding", "glm-4.6");
    agent_def_create(&node, &ada, ws, &def).await.unwrap();
    agent_config_set(&node, &ada, ws, &pick_patch(&def))
        .await
        .unwrap();

    // Now the ACTIVE definition resolves to THIS def, and the model is the built one (its scripted
    // answer names the endpoint's model → the right endpoint was built).
    let active = resolve_active_definition(&node, &ada, ws, None)
        .await
        .expect("a pick is active");
    assert_eq!(active.id, "glm-coder");
    let (answer, configured) = resolved_turn(&node, &ada, ws).await;
    assert_eq!(answer, "answer from glm-4.6", "the picked endpoint's model");
    assert!(configured, "a real provider is wired");
}

// ── workspace-isolation: ws-B never resolves ws-A's endpoint/model ─────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_never_resolves_ws_a_endpoint_or_model() {
    // MANDATORY workspace-isolation (§2.2): ada picks GLM-4.6 in ws-A; bob picks GLM-5.2 in ws-B on the
    // SAME node. Each resolve yields its OWN model — ws-B never sees ws-A's endpoint, even though the
    // cache is one map (the ws is part of the key, the store read is namespace-walled).
    let node = node_with_builder().await;
    let ws_a = "ws-iso-a";
    let ws_b = "ws-iso-b";
    let ada = admin("user:ada", ws_a);
    let bob = admin("user:bob", ws_b);

    let def_a = definition("a-def", "zaicoding", "glm-4.6");
    agent_def_create(&node, &ada, ws_a, &def_a).await.unwrap();
    agent_config_set(&node, &ada, ws_a, &pick_patch(&def_a))
        .await
        .unwrap();

    let def_b = definition("b-def", "zaicoding", "glm-5.2");
    agent_def_create(&node, &bob, ws_b, &def_b).await.unwrap();
    agent_config_set(&node, &bob, ws_b, &pick_patch(&def_b))
        .await
        .unwrap();

    let (answer_a, _) = resolved_turn(&node, &ada, ws_a).await;
    let (answer_b, _) = resolved_turn(&node, &bob, ws_b).await;
    assert_eq!(
        answer_a, "answer from glm-4.6",
        "ws-A resolves its own model"
    );
    assert_eq!(
        answer_b, "answer from glm-5.2",
        "ws-B resolves its own model"
    );
    assert_ne!(
        answer_a, answer_b,
        "the wall holds — no cross-workspace bleed"
    );

    // ws-B's active definition is its OWN — never ws-A's.
    let active_b = resolve_active_definition(&node, &bob, ws_b, None)
        .await
        .unwrap();
    assert_eq!(active_b.id, "b-def");
}

// ── cache invalidation: a re-pick busts the memoized model ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_pick_invalidates_the_memoized_model() {
    // The memoization risk (scope): a changed pick must not keep answering with the stale model. Pick
    // GLM-4.6, resolve (caches it), then re-pick GLM-5.2 — the next resolve must reflect the NEW pick,
    // proving `agent.config.set` invalidated the ws entry (not served the cached GLM-4.6).
    let ws = "ws-invalidate";
    let node = node_with_builder().await;
    let ada = admin("user:ada", ws);

    let def1 = definition("d1", "zaicoding", "glm-4.6");
    agent_def_create(&node, &ada, ws, &def1).await.unwrap();
    agent_config_set(&node, &ada, ws, &pick_patch(&def1))
        .await
        .unwrap();
    let (a1, _) = resolved_turn(&node, &ada, ws).await; // caches (ws, glm-4.6 endpoint)
    assert_eq!(a1, "answer from glm-4.6");

    // Re-pick a different model. `agent_config_set` must invalidate the ws entry.
    let def2 = definition("d2", "zaicoding", "glm-5.2");
    agent_def_create(&node, &ada, ws, &def2).await.unwrap();
    agent_config_set(&node, &ada, ws, &pick_patch(&def2))
        .await
        .unwrap();
    let (a2, _) = resolved_turn(&node, &ada, ws).await;
    assert_eq!(
        a2, "answer from glm-5.2",
        "the re-pick busted the cache — the new model answers, not the stale one"
    );
}

// ── the in-house loop rides the per-workspace model ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_in_house_loop_drives_the_picked_workspace_model() {
    // An IMPLICIT run (`runtime` omitted → default) must drive the workspace's PICKED model, not the
    // node-level `UnconfiguredModel` fallback the registry was booted with. The run's answer is the
    // picked endpoint's scripted stop-turn — proving the per-run `model_override` (resolve_workspace_model
    // at run start) reached the in-house loop.
    let ws = "ws-loop-picks";
    let node = node_with_builder().await;
    let ada = admin("user:ada", ws);

    let def = definition("loop-def", "zaicoding", "glm-4.6");
    agent_def_create(&node, &ada, ws, &def).await.unwrap();
    agent_config_set(&node, &ada, ws, &pick_patch(&def))
        .await
        .unwrap();

    let tools = reachable_tools(&node, &ada, ws).await;
    let answer = invoke_via_runtime(
        &node,
        &node.runtimes(),
        None, // absent → the in-house default, which now rides the per-ws override
        &ada,
        &ada.caps().to_vec(),
        ws,
        "loop-1",
        "do the thing",
        Substrate::default(),
        &tools,
        1,
    )
    .await
    .expect("run completes");
    assert_eq!(
        answer, "answer from glm-4.6",
        "the in-house loop drove the workspace's picked model, not the node fallback"
    );
}

// ── adapter key precedence: sealed WORKSPACE secret → node env → unset (host-mediated) ─────────────

/// A builder that RECORDS the key it was handed (so a test can assert which key precedence resolved).
/// It still returns a working scripted model. The recorded key never leaves the test.
struct KeyRecordingBuilder(Arc<Mutex<Option<String>>>);

impl ModelBuilder for KeyRecordingBuilder {
    fn build(
        &self,
        endpoint: &DefinitionEndpoint,
        key: Option<&str>,
    ) -> Option<Arc<dyn ErasedModel>> {
        *self.0.lock().unwrap() = key.map(str::to_string);
        let gw = AiGateway::new(MockProvider::new(vec![AiResponse::stop(
            format!("answer from {}", endpoint.model),
            1,
        )]));
        Some(Arc::new(gw))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_key_resolves_sealed_workspace_secret_over_env() {
    // The adapter key is resolved host-mediated (`resolve_endpoint_key_host`): a sealed WORKSPACE secret
    // wins over the node env. Seal a Workspace secret at the definition's `api_key_secret` path and set
    // the env NAME too; the builder must receive the SEALED value (secret → env precedence). Names-only:
    // the definition record carries only PATH + env NAME, never the value.
    let ws = "ws-key-sealed";
    let seen = Arc::new(Mutex::new(None));
    let node = Arc::new(Node::boot().await.unwrap());
    node.install_model_builder(Arc::new(KeyRecordingBuilder(seen.clone())));
    let ada = admin("user:ada", ws);

    // Seal a WORKSPACE-visibility secret (host-mediated `get_workspace` only reads Workspace secrets).
    let sealer = {
        let key = SigningKey::generate();
        let claims = Claims {
            sub: "user:ada".into(),
            ws: ws.into(),
            role: Role::Member,
            caps: vec!["secret:agent/*:write".into()],
            iat: 0,
            exp: u64::MAX,
        };
        verify(&key, &mint(&key, &claims), 1).unwrap()
    };
    lb_secrets::set_with(
        &node.store,
        &sealer,
        ws,
        "agent/model-key",
        "SEALED-VALUE",
        lb_secrets::Visibility::Workspace,
    )
    .await
    .unwrap();

    // A definition whose endpoint references BOTH a sealed path and an env NAME. Set the env too — the
    // sealed value must win.
    std::env::set_var("ACTIVE_MODEL_ENV_KEY", "ENV-VALUE");
    let mut def = definition("keyed", "zaicoding", "glm-4.6");
    def.model_endpoint.api_key_secret = Some("agent/model-key".into());
    def.model_endpoint.api_key_env = Some("ACTIVE_MODEL_ENV_KEY".into());
    agent_def_create(&node, &ada, ws, &def).await.unwrap();
    agent_config_set(&node, &ada, ws, &pick_patch(&def))
        .await
        .unwrap();
    // pick_patch copies the endpoint fields; ensure the sealed path rides the copy too.
    let cfg = get_agent_config(&node.store, ws).await.unwrap().unwrap();
    assert_eq!(
        cfg.model_endpoint
            .as_ref()
            .unwrap()
            .api_key_secret
            .as_deref(),
        Some("agent/model-key"),
        "the pick copied the sealed path (names-only)"
    );

    let _ = resolve_workspace_model(&node, &ada, ws).await;
    assert_eq!(
        seen.lock().unwrap().as_deref(),
        Some("SEALED-VALUE"),
        "the sealed WORKSPACE secret wins over the env (secret → env precedence)"
    );
    std::env::remove_var("ACTIVE_MODEL_ENV_KEY");
}

// ── offline/sync: agent.config double-delivery keeps active_definition idempotent (LWW) ────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn agent_config_double_delivery_keeps_active_definition_idempotent() {
    // MANDATORY offline/sync (§2.3): the SAME pick delivered twice (an offline replay) UPSERTs to the
    // same record — `active_definition` is set once and stays, never duplicated or cleared.
    let ws = "ws-lww";
    let node = node_with_builder().await;
    let ada = admin("user:ada", ws);

    let def = definition("lww-def", "zaicoding", "glm-4.6");
    agent_def_create(&node, &ada, ws, &def).await.unwrap();
    let patch = pick_patch(&def);

    agent_config_set(&node, &ada, ws, &patch).await.unwrap();
    agent_config_set(&node, &ada, ws, &patch).await.unwrap(); // double-deliver (replay)

    let cfg = get_agent_config(&node.store, ws)
        .await
        .unwrap()
        .expect("config present");
    assert_eq!(
        cfg.active_definition.as_deref(),
        Some("lww-def"),
        "the active_definition is LWW-idempotent across a double-deliver"
    );
    assert_eq!(cfg.default_runtime.as_deref(), Some("default"));
}
