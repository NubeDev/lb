//! Channel agent-worker tests (channels-agent scope + run-lifecycle #5 background execution), over a
//! REAL `Node` + the REAL `post` path, the REAL background reactor drain, and the REAL in-house agent
//! loop driven through the runtime seam. The ONLY stubbed external is the model provider HTTP
//! (`MockProvider`, rule 9 — store + bus + loop + channel + job queue are all real). The `default`
//! runtime is installed via `Node::install_runtimes` exactly as a feature-on node installs external
//! runtimes; here it is the in-house loop over a fixed-answer provider, so a posted `kind:"agent"` item
//! ENQUEUES a durable run that the drain drives and posts a real `agent_result` from.
//!
//! Mandatory invariants (mirroring the query worker's tests + testing-scope §2.1, plus #5's async gate):
//!   - BACKGROUND SPAWN (#5): posting an `agent` item returns from `post` BEFORE the run completes —
//!     the `agent_result` is absent right after `post` and appears only after the reactor drains.
//!   - HAPPY PATH: after the drain, an `agent` request from a granted poster yields an `agent_result`
//!     carrying the run's answer, posted under `system:agent-worker`, correlated to the run (`a:<job>`).
//!   - IDEMPOTENCY (#5 durable-resume safety): draining twice does NOT post a second result / re-run.
//!   - CAPABILITY DENY (opaque): a poster WITHOUT `mcp:agent.invoke:call` gets an `agent_error` whose
//!     message is EXACTLY "agent not permitted".
//!   - UNKNOWN RUNTIME (opaque): a named runtime that isn't registered collapses to the SAME "agent not
//!     permitted" — no runtime-existence leak.
//!   - RE-ENTRANCY: posting an `agent_result` item does NOT enqueue a run.
//!   - WORKSPACE ISOLATION: the result lands in the poster's workspace only; a ws-B reader can't see it,
//!     and a ws-B drain never picks up the ws-A run.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, drain_channel_agent_runs, drain_channel_agent_runs_with_ceiling, history,
    post, AgentConfig, AgentRuntime, ErasedModel, Node, RunContext, RuntimeRegistry,
};
use lb_inbox::Item;
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

const INVOKE: &str = "mcp:agent.invoke:call";

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// The in-house loop over a provider that stops immediately with `answer` — a real run, no real model.
fn answer_model(answer: &str) -> Arc<dyn ErasedModel> {
    Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
        answer, 1,
    )])))
}

/// Install a default-only registry whose `default` returns `answer` — the node's in-house loop.
fn install_answer_runtime(node: &Node, answer: &str) {
    node.install_runtimes(RuntimeRegistry::with_default(answer_model(answer)));
}

fn agent_request_body(goal: &str, runtime: Option<&str>, job: &str) -> String {
    let mut v = serde_json::json!({ "kind": "agent", "goal": goal, "job": job });
    if let Some(r) = runtime {
        v["runtime"] = serde_json::json!(r);
    }
    v.to_string()
}

/// The worker's `agent_result`/`agent_error` item for a channel, if the reactor has posted one.
async fn worker_item(node: &Node, p: &Principal, ws: &str, cid: &str) -> Option<Item> {
    history(&node.store, p, ws, cid)
        .await
        .expect("history")
        .into_iter()
        .find(|i| i.author == "system:agent-worker")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn post_enqueues_and_returns_before_the_run_completes() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    install_answer_runtime(&node, "the deploy rolled back at 14:02");
    let ws = "acme";
    let cid = "ops";
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body("what changed in the deploy?", None, "run-1");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");

    // BACKGROUND SPAWN (#5): `post` returned WITHOUT driving the run — no result yet, only the request.
    assert!(
        worker_item(&node, &p, ws, cid).await.is_none(),
        "post must return before the run completes (the run is detached from the POST connection)"
    );
    let items = history(&node.store, &p, ws, cid).await.expect("history");
    assert_eq!(
        items.len(),
        1,
        "only the request item is durable right after post"
    );

    // The reactor drains the durable queue and drives the run to completion — the answer appears now.
    drain_channel_agent_runs(&node, ws).await;

    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("the drain posted an agent_result");
    assert_eq!(
        result.id, "a:run-1",
        "the answer is correlated to the run id"
    );
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json body");
    assert_eq!(parsed["kind"], "agent_result");
    assert_eq!(parsed["answer"], "the deploy rolled back at 14:02");
    assert_eq!(parsed["runtime"], "default");
    assert_eq!(parsed["job"], "run-1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn draining_twice_does_not_post_a_second_result() {
    // #5 durable-resume safety: a re-drain (a second reactor tick, or a restart mid-queue) must not
    // re-run or double-post — the `a:<job>` idempotency guard short-circuits the re-drive.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    install_answer_runtime(&node, "answered once");
    let ws = "acme";
    let cid = "ops";
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body("g", None, "run-dup");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("request posts");

    drain_channel_agent_runs(&node, ws).await;
    drain_channel_agent_runs(&node, ws).await; // second tick — must be a no-op

    let results: Vec<_> = history(&node.store, &p, ws, cid)
        .await
        .expect("history")
        .into_iter()
        .filter(|i| i.author == "system:agent-worker")
        .collect();
    assert_eq!(
        results.len(),
        1,
        "exactly one result even after two drains: {results:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_request_without_the_invoke_grant_yields_opaque_agent_not_permitted() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    install_answer_runtime(&node, "unreachable");
    let ws = "acme";
    let cid = "ops";
    // Can pub/sub the channel (so the post is authorized) but lacks `mcp:agent.invoke:call`.
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
        ],
    );

    let body = agent_request_body("do a thing", None, "run-2");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("the request posts");
    drain_channel_agent_runs(&node, ws).await;

    let err = worker_item(&node, &p, ws, cid)
        .await
        .expect("the drain posted an agent_error");
    let parsed: serde_json::Value = serde_json::from_str(&err.body).expect("json body");
    assert_eq!(parsed["kind"], "agent_error");
    assert_eq!(
        parsed["error"], "agent not permitted",
        "the deny is opaque (no capability leak)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_named_unknown_runtime_collapses_to_the_same_opaque_deny() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    install_answer_runtime(&node, "unreachable"); // only `default` is registered
    let ws = "acme";
    let cid = "ops";
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    // Name a runtime the node doesn't have → opaque "agent not permitted" (no existence leak): a
    // client can't tell a forbidden/absent runtime from a missing grant.
    let body = agent_request_body("hi", Some("open-interpreter-default"), "run-3");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("the request posts");
    drain_channel_agent_runs(&node, ws).await;

    let err = worker_item(&node, &p, ws, cid)
        .await
        .expect("the drain posted an agent_error");
    let parsed: serde_json::Value = serde_json::from_str(&err.body).expect("json body");
    assert_eq!(parsed["error"], "agent not permitted");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn posting_an_agent_result_item_does_not_enqueue_a_run() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    install_answer_runtime(&node, "unreachable");
    let ws = "acme";
    let cid = "ops";
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    // The worker's OWN output shape — re-posting it must not enqueue another run (infinite-loop guard).
    let body = serde_json::json!({
        "kind": "agent_result", "goal": "g", "runtime": "default", "job": "run-4", "answer": "a"
    })
    .to_string();
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("r1", cid, "user:ada", body, 1),
    )
    .await
    .expect("the result item posts");
    drain_channel_agent_runs(&node, ws).await; // nothing was enqueued, so this is a no-op

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    assert_eq!(items.len(), 1, "no worker item spawned: {items:?}");
    assert!(!items.iter().any(|i| i.author == "system:agent-worker"));
}

/// A runtime that never settles — its `run` future sleeps far past any test ceiling — standing in for
/// a hung/looping external subprocess. Registered under `default` so an absent `runtime` selects it.
/// The supervision ceiling must reap it (drop its future) and post an honest `agent_error`.
struct HungRuntime;

impl AgentRuntime for HungRuntime {
    fn id(&self) -> &str {
        "default"
    }

    fn run<'a>(
        &'a self,
        _node: &'a std::sync::Arc<Node>,
        _ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, lb_host::AgentError>> + Send + 'a>> {
        Box::pin(async move {
            // Never completes within any test's wall — the supervision timeout must fire first.
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Ok("unreachable — should have been reaped".to_string())
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_run_that_exceeds_the_supervision_ceiling_is_reaped_with_an_agent_error() {
    // SUPERVISION (run-lifecycle #5): a hung run must not spin the card forever. Bound by a tiny
    // ceiling against a runtime that never settles, the drive drops the run future (reaping any
    // external subprocess) and posts an honest `agent_error` — NOT the opaque deny, and NOT a stuck
    // card. Terminal outcome is the ceiling (host authority), not the agent's eventual word.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let mut registry = RuntimeRegistry::with_default(answer_model("unused"));
    registry.register(Arc::new(HungRuntime)); // overrides `default` with the hung runtime
    node.install_runtimes(registry);

    let ws = "acme";
    let cid = "ops";
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body("loop forever", None, "run-hung");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("the request posts");

    // Drain with a tiny ceiling — the hung run is reaped in ~50ms, not the production 15 minutes.
    drain_channel_agent_runs_with_ceiling(&node, ws, Duration::from_millis(50)).await;

    let err = worker_item(&node, &p, ws, cid)
        .await
        .expect("the reaped run posted an agent_error, not a stuck card");
    let parsed: serde_json::Value = serde_json::from_str(&err.body).expect("json body");
    assert_eq!(
        parsed["kind"], "agent_error",
        "a reaped run is an error: {parsed:?}"
    );
    assert_eq!(
        parsed["error"], "agent run exceeded its time limit and was stopped",
        "the timeout message is honest (distinct from the opaque deny)"
    );
    assert_ne!(
        parsed["error"], "agent not permitted",
        "a timeout must NOT masquerade as a capability deny"
    );

    // The enqueue job was retired (terminal) — a reaped run does not stay pending to be re-driven.
    // A second drain is a pure no-op (idempotent on the `a:<job>` error item now present).
    drain_channel_agent_runs_with_ceiling(&node, ws, Duration::from_millis(50)).await;
    let errors: Vec<_> = history(&node.store, &p, ws, cid)
        .await
        .expect("history")
        .into_iter()
        .filter(|i| i.author == "system:agent-worker")
        .collect();
    assert_eq!(
        errors.len(),
        1,
        "exactly one error even after re-drain: {errors:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_result_is_workspace_scoped_and_not_visible_from_another_workspace() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    install_answer_runtime(&node, "ws-A only answer");
    let cid = "ops";
    let ada = principal(
        "acme",
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body("secret", None, "run-5");
    post(
        &node,
        &ada,
        "acme",
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("request posts in ws acme");

    // A ws-B drain must not pick up the ws-A run (the queue scan is workspace-namespaced).
    drain_channel_agent_runs(&node, "other").await;
    assert!(
        worker_item(&node, &ada, "acme", cid).await.is_none(),
        "a ws-B drain must not drive the ws-A run"
    );

    // The ws-A drain drives it; the result lands in ws acme only.
    drain_channel_agent_runs(&node, "acme").await;
    assert!(
        worker_item(&node, &ada, "acme", cid).await.is_some(),
        "the result lands in ws acme"
    );

    // A ws-B reader of the same channel id sees NOTHING — the store is workspace-namespaced.
    let bob = principal(
        "other",
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );
    let cross = history(&node.store, &bob, "other", cid).await.unwrap();
    assert!(
        cross.is_empty(),
        "ws-B cannot see ws-A's agent exchange: {cross:?}"
    );
}

/// A stub external runtime that returns a fixed sentinel — stands in for `open-interpreter-default`.
struct StubExternal {
    id: String,
    answer: String,
}

impl AgentRuntime for StubExternal {
    fn id(&self) -> &str {
        &self.id
    }
    fn run<'a>(
        &'a self,
        _node: &'a std::sync::Arc<Node>,
        _ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, lb_host::AgentError>> + Send + 'a>> {
        let a = self.answer.clone();
        Box::pin(async move { Ok(a) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_omitted_runtime_run_uses_and_labels_the_workspace_default() {
    // The bug this guards (surfaced live in the channel UI): with the workspace default set to an
    // external runtime, an `/agent` run that OMITS `runtime` must (a) actually RUN that runtime and
    // (b) LABEL the `agent_result.runtime` as the resolved id — not the misleading `"default"`. The
    // worker computed the label from the raw (omitted) runtime, so a resolved external run still
    // read `"default"` in the card. Fixed to label from `resolve_effective_runtime_id`.
    const STUB: &str = "open-interpreter-default";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // A node that OFFERS the external stub (a feature-on posture), installed like the real one.
    let mut registry = RuntimeRegistry::with_default(answer_model("in-house"));
    registry.register(Arc::new(StubExternal {
        id: STUB.into(),
        answer: "external ran".into(),
    }));
    node.install_runtimes(registry);

    let ws = "acme";
    let cid = "abc";
    let admin = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
            "mcp:agent.config.set:call",
        ],
    );

    // Seed the workspace default = the external stub via the REAL registry-validated write path.
    agent_config_set(
        &node,
        &admin,
        ws,
        &AgentConfig {
            active_definition: None,
            active_persona: None,
            enabled_personas: None,
            default_runtime: Some(STUB.into()),
            model_endpoint: None,
        },
    )
    .await
    .expect("admin sets the workspace default");

    // Post an agent request with NO `runtime` (the /agent-with-default case).
    let body = agent_request_body("do it", None, "run-oi");
    post(
        &node,
        &admin,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");

    drain_channel_agent_runs(&node, ws).await;

    let result = worker_item(&node, &admin, ws, cid)
        .await
        .expect("the drain posted an agent_result");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json body");
    assert_eq!(parsed["kind"], "agent_result");
    // (a) the STORED external runtime actually ran (not the in-house default).
    assert_eq!(
        parsed["answer"], "external ran",
        "the omitted-runtime run resolved and drove the workspace default (the stub), not in-house"
    );
    // (b) the label reflects the RESOLVED runtime, not the misleading `"default"`.
    assert_eq!(
        parsed["runtime"], STUB,
        "agent_result.runtime must label the resolved runtime, not `default`"
    );
}
