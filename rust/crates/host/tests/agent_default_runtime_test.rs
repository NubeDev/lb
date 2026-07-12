//! Agent-config follow-up (runtime-id resolution) — "honor the stored default in `agent.invoke` when
//! `runtime` is omitted". Proves the ONE resolution seam's precedence end to end through
//! `invoke_via_runtime` against a REAL `Node` (rule 9 — store + registry + gate all real; the only
//! stubbed thing is a deterministic `AgentRuntime` standing in for an external engine, which is a
//! runtime trait-object, not a mocked backend). The workspace default is seeded via the REAL write
//! path (`agent_config_set`), validated against the node's own registry — so the node offers the stub
//! id exactly as a `--features external-agent` node would offer `open-interpreter-default`.
//!
//! Precedence proven here:
//!   - EXPLICIT arg wins over the stored default;
//!   - ABSENT arg + a stored default the node offers → the stored runtime runs;
//!   - ABSENT arg + a stored-but-UNAVAILABLE default → falls back to the registry default (no error);
//!   - WORKSPACE ISOLATION — ws-A's stored default never affects a run in ws-B;
//!   - the invoke GATE still denies without `mcp:agent.invoke:call` (resolving a default widens nothing).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, invoke_via_runtime, AgentConfig, AgentError, AgentRuntime, AllowedTool,
    ErasedModel, Node, RunContext, RuntimeRegistry, Substrate, DEFAULT_RUNTIME,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

const INVOKE: &str = "mcp:agent.invoke:call";
const SET: &str = "mcp:agent.config.set:call";
const STUB_ID: &str = "open-interpreter-default";

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

/// The in-house `default` model — stops immediately with a fixed answer (enough to prove which
/// runtime served the run).
fn answer_model(answer: &str) -> Arc<dyn ErasedModel> {
    Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
        answer, 1,
    )])))
}

/// A deterministic stub `AgentRuntime` standing in for an external engine: it ignores the model and
/// returns a fixed sentinel, so a test can tell "the STORED runtime ran" from "the default ran". This
/// is a runtime trait-object (what the seam #1 abstracts over), NOT a mocked backend.
struct StubRuntime {
    id: String,
    answer: String,
}

impl AgentRuntime for StubRuntime {
    fn id(&self) -> &str {
        &self.id
    }
    fn run<'a>(
        &'a self,
        _node: &'a std::sync::Arc<Node>,
        _ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send + 'a>> {
        let answer = self.answer.clone();
        Box::pin(async move { Ok(answer) })
    }
}

/// A registry holding the in-house `default` plus a registered external stub `id`.
fn registry_with_stub(default_answer: &str, id: &str, stub_answer: &str) -> RuntimeRegistry {
    let mut registry = RuntimeRegistry::with_default(answer_model(default_answer));
    registry.register(Arc::new(StubRuntime {
        id: id.to_string(),
        answer: stub_answer.to_string(),
    }));
    registry
}

/// Store `default_runtime` for `ws` via the REAL, registry-validated write path.
async fn set_default(node: &Node, admin: &Principal, ws: &str, runtime: &str) {
    agent_config_set(
        node,
        admin,
        ws,
        &AgentConfig {
            compact_budget: None,
            loop_window: None,
            active_definition: None,
            active_persona: None,
            enabled_personas: None,
            default_runtime: Some(runtime.into()),
            model_endpoint: None,
        },
    )
    .await
    .expect("admin seeds the workspace default (registry-validated)");
}

/// Drive a run through the seam against the NODE's own registry (exactly what the production
/// entrypoints pass), with no substrate/tools — the shared body of these cases.
async fn run(
    node: &Arc<Node>,
    runtime: Option<&str>,
    caller: &Principal,
    ws: &str,
) -> Result<String, AgentError> {
    let registry = node.runtimes();
    invoke_via_runtime(
        node,
        &registry,
        runtime,
        None,
        caller,
        &[],
        ws,
        "job",
        "do a thing",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn absent_runtime_uses_the_stored_workspace_default() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // The node offers the stub (a `--features external-agent` node's posture).
    node.install_runtimes(registry_with_stub("in-house", STUB_ID, "external-ran"));
    let ws = "rt-stored";
    let admin = principal("user:ada", ws, &[SET, INVOKE]);

    set_default(&node, &admin, ws, STUB_ID).await;

    let answer = run(&node, None, &admin, ws)
        .await
        .expect("absent runtime resolves to the stored default and runs");
    assert_eq!(
        answer, "external-ran",
        "the stored workspace default (the stub) served the run, not the in-house default"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn explicit_runtime_overrides_the_stored_default() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    node.install_runtimes(registry_with_stub("in-house", STUB_ID, "external-ran"));
    let ws = "rt-explicit";
    let admin = principal("user:ada", ws, &[SET, INVOKE]);

    set_default(&node, &admin, ws, STUB_ID).await; // stored default = the stub…

    // …but an EXPLICIT `default` arg wins over it.
    let answer = run(&node, Some(DEFAULT_RUNTIME), &admin, ws)
        .await
        .expect("explicit default runs");
    assert_eq!(
        answer, "in-house",
        "the explicit runtime argument overrides the stored workspace default"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_stored_but_unavailable_default_falls_back_to_the_registry_default() {
    // Registry drift, fail-open: the workspace's stored default names an id this node no longer
    // offers (feature off / config changed). The run must NOT error — it falls back to the registry
    // default. Seed with a registry that HAS the stub (so `set` accepts it), then "drift" the node by
    // re-installing a default-only registry before the run.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    node.install_runtimes(registry_with_stub("unused", STUB_ID, "should-not-run"));
    let ws = "rt-drift";
    let admin = principal("user:ada", ws, &[SET, INVOKE]);

    set_default(&node, &admin, ws, STUB_ID).await; // valid at write time

    // The node drifts: the stub is gone; only `default` remains (a feature-off restart).
    node.install_runtimes(RuntimeRegistry::with_default(answer_model(
        "in-house-fallback",
    )));

    let answer = run(&node, None, &admin, ws)
        .await
        .expect("a stored-but-unavailable default falls back, never errors");
    assert_eq!(
        answer, "in-house-fallback",
        "the run fell back to the registry default rather than erroring on the missing stored id"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspaces_are_isolated_for_the_stored_default() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    node.install_runtimes(registry_with_stub("in-house", STUB_ID, "external-ran"));
    let admin_a = principal("user:ada", "ws-a", &[SET, INVOKE]);
    let admin_b = principal("user:bob", "ws-b", &[SET, INVOKE]);

    // ws-A stores the stub as its default; ws-B stores nothing.
    set_default(&node, &admin_a, "ws-a", STUB_ID).await;

    // A run in ws-B (no stored default) uses the registry default — ws-A's choice never leaks.
    let b = run(&node, None, &admin_b, "ws-b").await.expect("ws-b run");
    assert_eq!(
        b, "in-house",
        "ws-b has no stored default and is unaffected by ws-a's"
    );

    // A run in ws-A uses ITS stored default (the stub) — the record is scoped per workspace.
    let a = run(&node, None, &admin_a, "ws-a").await.expect("ws-a run");
    assert_eq!(a, "external-ran", "ws-a uses its own stored default");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn invoke_is_still_denied_without_the_cap_even_with_a_stored_default() {
    // Resolving a default widens NOTHING — the invoke gate still fires first. A caller lacking
    // `mcp:agent.invoke:call` is refused before any config read or runtime selection.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    node.install_runtimes(registry_with_stub("in-house", STUB_ID, "external-ran"));
    let ws = "rt-deny";
    let admin = principal("user:ada", ws, &[SET, INVOKE]);

    set_default(&node, &admin, ws, STUB_ID).await;

    let caller = principal("user:eve", ws, &[]); // no invoke cap
    let err = run(&node, None, &caller, ws)
        .await
        .expect_err("ungranted invoke is denied");
    assert!(
        matches!(err, AgentError::Denied),
        "denied at the invoke gate, before the stored default is even read"
    );
}
