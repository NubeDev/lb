//! `agent.def.test {id?}` + DB-sealed per-workspace model key (agent-catalog test-and-secrets scope).
//!
//! **Rule 9: everything real** — the `mem://` store, caps, `lb-secrets`, and the context assembly
//! (`reachable_tools` + `render_catalog` + `list_granted_skills`) are the real code. The ONLY permitted
//! fake is the model **provider HTTP** (`MockProvider`, behind the `Provider` trait via
//! `AiGateway<MockProvider>`); the test drives the real invoke path (one turn), not a fake.
//!
//! What this proves (the scope's Testing plan):
//!   - **Capability-deny** — `agent.def.test` without `mcp:agent.def.test:call` → opaque `Denied`.
//!   - **Test proves context (the headline)** — the returned `context.tools`/`context.skills` name the
//!     caller's REAL grants (seeded skill + a reachable tool surface), proving the assembly ran.
//!   - **Test inherits the wall** — fewer grants → fewer tools/skills; ws-B never sees ws-A's.
//!   - **Sealed key, names-only** — after `secret.set` + a definition write, the record holds only the
//!     PATH; the test's `answer` never contains the key value (it goes to transport, not the prompt).
//!   - **Key-resolution precedence** — `resolve_endpoint_key` resolves secret → env → none (all three).
//!   - **Workspace-isolation of the key** — a ws-B admin cannot `secret.get`/rotate ws-A's key.
//!   - **Built-in stays read-only + node-env** — a `builtin.*` write is `Reserved`; its key is env.
//!   - **Bounded diagnostic** — the test persists NO durable run record (no job for its session id).
//!   - **`provider_configured` honest** — `UnconfiguredModel` → false; a real `AiGateway` → true.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, agent_def_create, agent_def_test, grant_skill, put_skill,
    resolve_endpoint_key, AgentConfig, AgentDefinition, DefinitionEndpoint, ErasedModel,
    ModelBuilder, ModelEndpointPatch, Node, RuntimeRegistry, UnconfiguredModel,
};
use lb_mcp::ToolError;
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

// ── caps ─────────────────────────────────────────────────────────────────────────────────────────
const TEST: &str = "mcp:agent.def.test:call";
const CATALOG: &str = "mcp:tools.catalog:call";
const INVOKE: &str = "mcp:agent.invoke:call";
const DEF_CREATE: &str = "mcp:agent.def.create:call";
const DEF_GET: &str = "mcp:agent.def.get:call";
const DEF_LIST: &str = "mcp:agent.def.list:call";
const SKILL_R: &str = "store:skill/*:read";
const SKILL_W: &str = "store:skill/*:write";
/// The shipped secrets gate for a sealed path under `agent/`.
const SECRET_W: &str = "secret:agent/*:write";
const SECRET_G: &str = "secret:agent/*:get";

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A gateway whose single turn answers with a fixed line that does NOT contain any key value.
fn mock_gateway(answer: &str) -> Arc<dyn ErasedModel> {
    Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
        answer, 5,
    )])))
}

/// Install a real `AiGateway<MockProvider>` as the node's default model.
fn configure(node: &Arc<Node>, answer: &str) {
    node.install_runtimes(RuntimeRegistry::with_default(mock_gateway(answer)));
}

/// A custom definition binding the always-present `default` runtime, referencing a sealed key PATH.
fn def_with_secret(id: &str, secret_path: &str) -> AgentDefinition {
    AgentDefinition {
        id: id.into(),
        label: "custom test agent".into(),
        description: None,
        runtime: "default".into(),
        model_endpoint: DefinitionEndpoint {
            provider: "zaicoding".into(),
            model: "glm-4.6".into(),
            api_key_env: None,
            api_key_secret: Some(secret_path.into()),
            base_url: None,
        },
        builtin: false,
    }
}

// ── capability-deny (§2.1) ────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_without_the_cap_is_denied_opaquely() {
    // MANDATORY deny: a member lacking `mcp:agent.def.test:call` cannot spend model budget via the
    // test — the gate is opaque `Denied`, before any target resolution.
    let ws = "adt-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    configure(&node, "hello");

    // Holds create/get so a definition exists, but NOT the test cap.
    let admin = principal("user:ada", ws, &[DEF_CREATE, DEF_GET]);
    agent_def_create(
        &node,
        &admin,
        ws,
        &def_with_secret("acme", "agent/acme-key"),
    )
    .await
    .unwrap();

    let no_test = principal("user:bob", ws, &[DEF_GET]);
    let err = agent_def_test(&node, &no_test, ws, Some("acme"))
        .await
        .expect_err("test without the cap must be denied");
    assert!(matches!(err, ToolError::Denied), "opaque Denied, no leak");
}

// ── the headline: the test proves the real assembled context ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_returns_the_callers_real_context() {
    // The headline: seed a workspace with a granted skill + a reachable tool surface, run the test, and
    // assert `context.skills` names the skill and `context.tools` names a reachable tool — proving the
    // REAL assembly ran (not a stub). Against the mock the `answer` is deterministic; assert it too.
    let ws = "adt-context";
    let node = Arc::new(Node::boot().await.unwrap());
    configure(&node, "I am the workspace agent.");

    // The caller needs the test cap + the reads the assembly performs (catalog + skill read) + the
    // author caps to seed + a definition. Holding `mcp:agent.invoke:call` makes `agent.invoke` appear
    // in the reachable tool surface (a host-native descriptor ∩ grants) — a stable real tool name to
    // assert (the same discriminator the in-house-wiring test uses for the menu = catalog proof).
    let caps = &[
        TEST, CATALOG, INVOKE, DEF_CREATE, DEF_GET, DEF_LIST, SKILL_R, SKILL_W,
    ];
    let admin = principal("user:ada", ws, caps);

    put_skill(
        &node.store,
        &admin,
        ws,
        "repo-conventions",
        "1",
        "repo coding conventions",
        "Always run the linter.",
        1,
    )
    .await
    .unwrap();
    grant_skill(&node.store, &admin, ws, "repo-conventions")
        .await
        .unwrap();

    agent_def_create(
        &node,
        &admin,
        ws,
        &def_with_secret("acme", "agent/acme-key"),
    )
    .await
    .unwrap();

    let result = agent_def_test(&node, &admin, ws, Some("acme"))
        .await
        .expect("the test runs");

    assert_eq!(result.id, "acme");
    assert_eq!(result.runtime, "default");
    assert_eq!(result.model, "zaicoding/glm-4.6");
    assert_eq!(result.answer, "I am the workspace agent.");
    assert!(result.ok);
    assert!(result.provider_configured, "a real AiGateway is configured");

    // Context proves the assembly ran: the granted skill is named, and a reachable tool is named.
    assert!(
        result
            .context
            .skills
            .iter()
            .any(|s| s == "repo-conventions"),
        "the granted skill is in the assembled context: {:?}",
        result.context.skills
    );
    assert_eq!(result.context.skill_count, result.context.skills.len());
    assert!(
        result.context.tools.iter().any(|t| t == "agent.invoke"),
        "a reachable catalog tool the caller holds is in the assembled context: {:?}",
        result.context.tools
    );
    assert_eq!(
        result.context.tool_count,
        result.context.tools.len(),
        "tool_count reflects the reachable surface (unbounded here — well under MAX_LISTED)"
    );
}

// ── the test inherits the wall (§2.2) ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_caller_with_fewer_grants_sees_fewer_skills() {
    // Inherits-the-wall: two callers, same workspace + definition; the one WITHOUT the skill read sees
    // no skill in its context. No context widening — the test resolves for the caller's own grants.
    let ws = "adt-wall";
    let node = Arc::new(Node::boot().await.unwrap());
    configure(&node, "ok");

    // Author seeds + grants a skill (needs the write cap).
    let author = principal("user:ada", ws, &[SKILL_R, SKILL_W, DEF_CREATE, DEF_GET]);
    put_skill(
        &node.store,
        &author,
        ws,
        "conv",
        "1",
        "conventions",
        "Be terse.",
        1,
    )
    .await
    .unwrap();
    grant_skill(&node.store, &author, ws, "conv").await.unwrap();
    agent_def_create(
        &node,
        &author,
        ws,
        &def_with_secret("acme", "agent/acme-key"),
    )
    .await
    .unwrap();

    // A rich caller (skill read) sees the skill; a poor caller (no skill read) does not.
    let rich = principal("user:rich", ws, &[TEST, CATALOG, DEF_GET, SKILL_R]);
    let poor = principal("user:poor", ws, &[TEST, CATALOG, DEF_GET]); // no SKILL_R

    let rich_ctx = agent_def_test(&node, &rich, ws, Some("acme"))
        .await
        .unwrap()
        .context;
    let poor_ctx = agent_def_test(&node, &poor, ws, Some("acme"))
        .await
        .unwrap()
        .context;

    assert!(
        rich_ctx.skills.iter().any(|s| s == "conv"),
        "the caller with the skill-read grant sees it"
    );
    assert!(
        !poor_ctx.skills.iter().any(|s| s == "conv"),
        "the caller without the grant does NOT see it — the wall is inherited, not widened"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_test_never_lists_ws_a_skills() {
    // Workspace-isolation: ws-A grants a skill; a ws-B test of a ws-B definition never lists it.
    let node = Arc::new(Node::boot().await.unwrap());
    configure(&node, "ok");

    let ws_a = "adt-iso-a";
    let ada = principal("user:ada", ws_a, &[SKILL_R, SKILL_W]);
    put_skill(&node.store, &ada, ws_a, "a-only", "1", "ws-A", "A BODY", 1)
        .await
        .unwrap();
    grant_skill(&node.store, &ada, ws_a, "a-only")
        .await
        .unwrap();

    let ws_b = "adt-iso-b";
    let bob = principal(
        "user:bob",
        ws_b,
        &[TEST, CATALOG, DEF_CREATE, DEF_GET, SKILL_R],
    );
    agent_def_create(
        &node,
        &bob,
        ws_b,
        &def_with_secret("b-agent", "agent/b-key"),
    )
    .await
    .unwrap();

    let ctx = agent_def_test(&node, &bob, ws_b, Some("b-agent"))
        .await
        .unwrap()
        .context;
    assert!(
        ctx.skills.iter().all(|s| s != "a-only"),
        "ws-B's test never lists a ws-A-only skill"
    );
}

// ── sealed key, names-only invariant ────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_record_holds_only_the_path_and_the_answer_is_key_free() {
    // Names-only: seal a key via `secret.set`, write a definition referencing only the PATH, and assert
    // (a) the stored definition carries the path but no value, and (b) the test's `answer` never
    // contains the key value (it goes to the provider transport, not the prompt).
    let ws = "adt-names-only";
    let node = Arc::new(Node::boot().await.unwrap());
    // The mock's answer deliberately does NOT include the secret — a real invoke never puts the key in
    // the prompt, so structurally it can't. We still assert on the returned answer.
    configure(&node, "I am the agent; my key is not in this message.");

    let secret_value = "sk-super-secret-KEYVALUE-123";
    let admin = principal(
        "user:ada",
        ws,
        &[TEST, CATALOG, DEF_CREATE, DEF_GET, SECRET_W, SECRET_G],
    );

    // Seal the value through the shipped sealed path — the value lands ONLY in lb-secrets.
    lb_secrets::set(&node.store, &admin, ws, "agent/acme-key", secret_value)
        .await
        .unwrap();

    // Write the definition referencing only the PATH.
    agent_def_create(
        &node,
        &admin,
        ws,
        &def_with_secret("acme", "agent/acme-key"),
    )
    .await
    .unwrap();

    // (a) The stored record carries the path, never the value.
    let stored = lb_host::agent_def_get(&node, &admin, ws, "acme")
        .await
        .unwrap();
    assert_eq!(
        stored.model_endpoint.api_key_secret.as_deref(),
        Some("agent/acme-key"),
        "the record references the path"
    );
    let serialized = serde_json::to_string(&stored).unwrap();
    assert!(
        !serialized.contains(secret_value),
        "the definition record must NOT contain the key value"
    );

    // (b) The test's answer is key-free.
    let result = agent_def_test(&node, &admin, ws, Some("acme"))
        .await
        .unwrap();
    assert!(
        !result.answer.contains(secret_value),
        "the returned answer must never echo the sealed key"
    );
    // And the whole DTO is value-free.
    let dto = serde_json::to_string(&result).unwrap();
    assert!(
        !dto.contains(secret_value),
        "the test result DTO carries no secret value"
    );
}

// ── key-resolution precedence: secret → env → none ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_endpoint_key_precedence_secret_then_env_then_none() {
    // The single-source-of-truth helper both the test and a real run consume. Cover all three legs:
    //   1. sealed secret present  → the SECRET value,
    //   2. only a node env var    → the ENV value,
    //   3. neither                → None (a clear unconfigured path, not a panic).
    let ws = "adt-resolve";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:ada", ws, &[SECRET_W, SECRET_G]);

    // (1) Sealed secret wins even when an env var is also set.
    lb_secrets::set(&node.store, &admin, ws, "agent/acme-key", "from-secret")
        .await
        .unwrap();
    // SAFETY: single-threaded test env manipulation; the var name is unique to this test.
    std::env::set_var("ADT_RESOLVE_ENV", "from-env");
    let v = resolve_endpoint_key(
        &node.store,
        &admin,
        ws,
        Some("agent/acme-key"),
        Some("ADT_RESOLVE_ENV"),
    )
    .await;
    assert_eq!(v.as_deref(), Some("from-secret"), "secret takes precedence");

    // (2) No secret path → the env var resolves.
    let v = resolve_endpoint_key(&node.store, &admin, ws, None, Some("ADT_RESOLVE_ENV")).await;
    assert_eq!(v.as_deref(), Some("from-env"), "env is the fallback");

    // A referenced-but-absent path also falls through to the env (best-effort, not an error).
    let v = resolve_endpoint_key(
        &node.store,
        &admin,
        ws,
        Some("agent/does-not-exist"),
        Some("ADT_RESOLVE_ENV"),
    )
    .await;
    assert_eq!(
        v.as_deref(),
        Some("from-env"),
        "an absent sealed path falls through to the env"
    );

    // (3) Neither → None.
    let v = resolve_endpoint_key(&node.store, &admin, ws, None, Some("ADT_UNSET_VAR_XYZ")).await;
    assert!(v.is_none(), "neither secret nor env → unset, never a panic");

    std::env::remove_var("ADT_RESOLVE_ENV");
}

// ── workspace-isolation of the sealed key (§2.2) ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_read_or_resolve_ws_a_sealed_key() {
    // The sealed key is workspace-walled: ws-A seals a key; a ws-B admin `secret.get`s the same path →
    // not found (its own ws namespace has nothing), and `resolve_endpoint_key` in ws-B yields None.
    let node = Arc::new(Node::boot().await.unwrap());
    let ws_a = "adt-key-a";
    let ws_b = "adt-key-b";

    let ada = principal("user:ada", ws_a, &[SECRET_W, SECRET_G]);
    lb_secrets::set(&node.store, &ada, ws_a, "agent/zaicoding-key", "ADA-KEY")
        .await
        .unwrap();
    // ws-A resolves its own key.
    assert_eq!(
        resolve_endpoint_key(&node.store, &ada, ws_a, Some("agent/zaicoding-key"), None)
            .await
            .as_deref(),
        Some("ADA-KEY")
    );

    // ws-B admin cannot get ws-A's key (different workspace namespace).
    let bob = principal("user:bob", ws_b, &[SECRET_W, SECRET_G]);
    assert!(
        lb_secrets::get(&node.store, &bob, ws_b, "agent/zaicoding-key")
            .await
            .is_err(),
        "ws-B cannot read ws-A's sealed key"
    );
    // And ws-B's resolution of the same path yields None (nothing in ws-B), never ws-A's value.
    assert!(
        resolve_endpoint_key(&node.store, &bob, ws_b, Some("agent/zaicoding-key"), None)
            .await
            .is_none(),
        "ws-B resolves nothing for the same path — the key is workspace-walled"
    );
}

// ── built-in stays read-only + node-env ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_builtin_write_referencing_a_secret_path_is_rejected() {
    // A `builtin.*` definition cannot be given a per-workspace secret path — the write is `Reserved`
    // (read-only tier) BEFORE the caps gate. A built-in's key stays the node-env name it ships with.
    let ws = "adt-builtin";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:ada", ws, &[DEF_CREATE, DEF_GET]);

    let err = agent_def_create(
        &node,
        &admin,
        ws,
        &def_with_secret("builtin.in-house", "agent/should-not-take"),
    )
    .await
    .expect_err("a builtin write must be rejected");
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "a builtin.* id is read-only (BadInput/Reserved), never accepted: {err:?}"
    );
}

// ── bounded diagnostic: no durable run record ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_test_persists_no_durable_run_record() {
    // Bounded: the test runs exactly one turn and persists NO session/transcript — assert the job the
    // test would use (`{ws}:agent-def-test`) does not exist as a durable record.
    let ws = "adt-bounded";
    let node = Arc::new(Node::boot().await.unwrap());
    configure(&node, "ok");
    let admin = principal("user:ada", ws, &[TEST, CATALOG, DEF_CREATE, DEF_GET]);

    agent_def_create(
        &node,
        &admin,
        ws,
        &def_with_secret("acme", "agent/acme-key"),
    )
    .await
    .unwrap();
    agent_def_test(&node, &admin, ws, Some("acme"))
        .await
        .unwrap();

    // No job record was created for the test's derived session id.
    let job = lb_jobs::load(&node.store, ws, &format!("{ws}:agent-def-test"))
        .await
        .unwrap();
    assert!(
        job.is_none(),
        "the test is a bounded diagnostic — it persists no durable run record"
    );
}

// ── provider_configured honest: unconfigured → false ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provider_configured_is_false_on_the_unconfigured_placeholder() {
    // Honest signal: the boot placeholder is not a real provider — `provider_configured` is false and
    // the answer is the honest unconfigured line (never implies a real LLM answered).
    let ws = "adt-unconfigured";
    let node = Arc::new(Node::boot().await.unwrap());
    node.install_runtimes(RuntimeRegistry::with_default(Arc::new(UnconfiguredModel)));
    let admin = principal("user:ada", ws, &[TEST, CATALOG, DEF_CREATE, DEF_GET]);

    agent_def_create(
        &node,
        &admin,
        ws,
        &def_with_secret("acme", "agent/acme-key"),
    )
    .await
    .unwrap();

    let result = agent_def_test(&node, &admin, ws, Some("acme"))
        .await
        .unwrap();
    assert!(
        !result.provider_configured,
        "the UnconfiguredModel placeholder reports provider_configured = false"
    );
    assert_eq!(
        result.answer,
        lb_host::UNCONFIGURED_ANSWER,
        "the honest unconfigured answer, not a fabricated one"
    );
}

// ── the in-house Test rides the WORKSPACE model, not the node default ───────────────────────────

/// A `ModelBuilder` twin of the node's `NodeModelBuilder`, scripting the provider HTTP: it builds a
/// real `AiGateway<MockProvider>` whose stop-turn names the endpoint's model, so a test can assert the
/// picked endpoint (not the node default) was the one built.
struct ScriptedBuilder;

impl ModelBuilder for ScriptedBuilder {
    fn build(
        &self,
        endpoint: &DefinitionEndpoint,
        _key: Option<&str>,
    ) -> Option<Arc<dyn ErasedModel>> {
        Some(mock_gateway(&format!("answer from {}", endpoint.model)))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_in_house_test_rides_the_workspace_picked_model_not_the_node_default() {
    // The regression the live test surfaced: the "Test" button for an in-house `default` pick resolved
    // `registry.default_model()` (the node-level `UnconfiguredModel`) and IGNORED the workspace's picked
    // + keyed endpoint entirely — so a workspace that keyed its pick still tested as the placeholder.
    // The node default is left UNconfigured here; only the WORKSPACE picks a keyed definition. The
    // in-house test must now resolve THAT model (`resolve_workspace_model`) and report it configured.
    let ws = "adt-ws-model";
    let node = Arc::new(Node::boot().await.unwrap());
    node.install_runtimes(RuntimeRegistry::with_default(Arc::new(UnconfiguredModel)));
    node.install_model_builder(Arc::new(ScriptedBuilder));
    let admin = principal(
        "user:ada",
        ws,
        &[
            TEST,
            CATALOG,
            DEF_CREATE,
            DEF_GET,
            DEF_LIST,
            "mcp:agent.config.set:call",
        ],
    );

    // A workspace pick of a keyed in-house definition (runtime `default`).
    let def = def_with_secret("acme", "agent/acme-key");
    agent_def_create(&node, &admin, ws, &def).await.unwrap();
    agent_config_set(
        &node,
        &admin,
        ws,
        &AgentConfig {
            active_definition: Some("acme".into()),
            default_runtime: Some("default".into()),
            compact_budget: None,
            loop_window: None,
            exfiltration_guard: None,
            model_endpoint: Some(ModelEndpointPatch {
                provider: Some("zaicoding".into()),
                model: Some("glm-4.6".into()),
                api_key_secret: Some("agent/acme-key".into()),
                ..Default::default()
            }),
            active_persona: None,
            enabled_personas: None,
        },
    )
    .await
    .unwrap();

    // Test the ACTIVE pick (no id — the exact `/agent/defs/test` route the button hits).
    let result = agent_def_test(&node, &admin, ws, None).await.unwrap();
    assert!(
        result.provider_configured,
        "the in-house test resolved the WORKSPACE model (a real AiGateway), not the node placeholder"
    );
    assert_eq!(
        result.answer, "answer from glm-4.6",
        "the picked endpoint's model answered — proving resolve_workspace_model, not registry.default_model()"
    );
}
