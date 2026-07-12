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
        // Ordinary session token (not a run-scoped delegation) — no caller bound, no run scope.
        constraint: None,
        run_id: None,
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

/// A runtime that mimics the external driver's **no-progress stall**: it suspends its own run job and
/// returns `AgentError::Stalled` — exactly what `AcpRuntime::run` does when its watchdog fires. The
/// worker must turn this into an actionable `agent_stalled` pause-and-ask item (NOT an `agent_error`),
/// leaving the run resumable.
struct StalledRuntime;

impl AgentRuntime for StalledRuntime {
    fn id(&self) -> &str {
        "default"
    }

    fn run<'a>(
        &'a self,
        node: &'a std::sync::Arc<Node>,
        ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, lb_host::AgentError>> + Send + 'a>> {
        Box::pin(async move {
            // The real driver creates the run job, then on stall suspends it and returns `Stalled`.
            // Mirror that: ensure the run job exists (the AcpRuntime creates it at start) before
            // suspending, so the worker sees a `Suspended` run job exactly as in production.
            let job = lb_jobs::Job::new(ctx.job_id, "agent-session", "", ctx.ts);
            let _ = lb_jobs::create(&node.store, ctx.ws, &job).await;
            let _ = lb_jobs::suspend(&node.store, ctx.ws, ctx.job_id).await;
            Err(lb_host::AgentError::Stalled)
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_stalled_run_posts_an_actionable_agent_stalled_prompt_and_stays_resumable() {
    // PAUSE-AND-ASK (external-agent run-lifecycle): a run that stalls is PAUSED, not failed. The worker
    // posts a distinct `agent_stalled` item (the dock renders keep-going/stop) and the run job stays
    // `Suspended` so `resume_run` ("keep going") can continue it. This is NOT an `agent_error`.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let mut registry = RuntimeRegistry::with_default(answer_model("unused"));
    registry.register(Arc::new(StalledRuntime)); // overrides `default` with the stalling runtime
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

    let body = agent_request_body("build a thing", None, "run-stall");
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

    // The worker posted an `agent_stalled` prompt carrying the run job + an honest message — NOT an
    // `agent_error` (the run is paused, not dead).
    let item = worker_item(&node, &p, ws, cid)
        .await
        .expect("a stalled run posts a pause-and-ask item");
    let parsed: serde_json::Value = serde_json::from_str(&item.body).expect("json body");
    assert_eq!(
        parsed["kind"], "agent_stalled",
        "stall is a pause-and-ask item, not an error: {parsed:?}"
    );
    assert_eq!(
        parsed["job"], "run-stall",
        "carries the run job for resume/stop"
    );
    assert!(
        parsed["message"].as_str().unwrap_or("").contains("stuck"),
        "honest prompt: {parsed:?}"
    );

    // The run job stays SUSPENDED (resumable) — "keep going" (resume_run) can continue it.
    let job = lb_jobs::load(&node.store, ws, "run-stall")
        .await
        .expect("load")
        .expect("run job exists");
    assert!(
        matches!(job.status, lb_jobs::JobStatus::Suspended),
        "a stalled run stays Suspended (resumable), got {:?}",
        job.status
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
            compact_budget: None,
            loop_window: None,
            exfiltration_guard: None,
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

/// CHANNEL-WIDGETS: an agent run that proposes `channel.post` with a `rich_result` body lands a REAL
/// rendered-widget item in the conversation's channel history (the dock renders it through the shipped
/// ResponseView path), alongside the normal `agent_result`. Also pins the goal contract: the run's
/// goal carries the `[conversation channel: <cid>]` line the skill keys on.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_run_can_post_a_rich_result_widget_into_its_own_channel() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    let cid = "dock-1";

    let envelope = serde_json::json!({
        "kind": "rich_result", "v": 2, "view": "table",
        "source": { "tool": "store.query", "args": { "sql": "SELECT * FROM site" } },
        "tools": ["store.query"],
    })
    .to_string();
    let model = Arc::new(AiGateway::new(MockProvider::new(vec![
        lb_role_ai_gateway::AiResponse::calls(
            "posting a live table",
            vec![lb_role_ai_gateway::ToolCall {
                id: "c1".into(),
                name: "channel.post".into(),
                input: serde_json::json!({ "cid": cid, "id": "w1", "ts": 2, "body": envelope })
                    .to_string(),
            }],
            1,
        ),
        AiResponse::stop("here is your table, rendered live", 1),
    ])));
    node.install_runtimes(RuntimeRegistry::with_default(model as Arc<dyn ErasedModel>));

    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
            "mcp:channel.post:call",
        ],
    );

    let body = agent_request_body("show me the sites as a table", None, "run-widget");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");
    drain_channel_agent_runs(&node, ws).await;

    // The durable answer posted as usual.
    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("agent_result posted");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json");
    assert_eq!(parsed["answer"], "here is your table, rendered live");

    // The rich_result widget item is IN the channel history, bound to the real query.
    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let widget = items
        .iter()
        .find(|i| {
            serde_json::from_str::<serde_json::Value>(&i.body)
                .map(|v| v["kind"] == "rich_result")
                .unwrap_or(false)
        })
        .expect("the run posted a rich_result item into its own channel");
    let w: serde_json::Value = serde_json::from_str(&widget.body).unwrap();
    assert_eq!(w["view"], "table");
    assert_eq!(w["source"]["tool"], "store.query");

    // The goal the run saw carries the channel id (the fact the skill's choreography keys on).
    let job = lb_jobs::load(&node.store, ws, "run-widget")
        .await
        .expect("load run job")
        .expect("run job exists");
    assert!(
        job.payload
            .contains(&format!("[conversation channel: {cid}]")),
        "the run goal must name its channel; got: {}",
        job.payload
    );

    // WORKSPACE ISOLATION (mandatory): a ws-B principal cannot read the widget item.
    let outsider = principal("globex", &[&format!("bus:chan/{cid}:sub")]);
    assert!(
        history(&node.store, &outsider, "globex", cid)
            .await
            .map(|items| items.is_empty())
            .unwrap_or(true),
        "the rich_result must be invisible outside its workspace"
    );
}

/// CHANNEL-WIDGETS capability deny (mandatory): a poster WITHOUT `mcp:channel.post:call` has the
/// agent's `channel.post` proposal denied at the wall — no rich_result lands — while the run itself
/// still completes with its text answer (a tool deny is fed back, never a crash).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_channel_post_cap_the_widget_post_is_denied_but_the_run_answers() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    let cid = "dock-2";

    let model = Arc::new(AiGateway::new(MockProvider::new(vec![
        lb_role_ai_gateway::AiResponse::calls(
            "",
            vec![lb_role_ai_gateway::ToolCall {
                id: "c1".into(),
                name: "channel.post".into(),
                input: serde_json::json!({ "cid": cid, "body": "{\"kind\":\"rich_result\",\"v\":2,\"view\":\"table\"}" })
                    .to_string(),
            }],
            1,
        ),
        AiResponse::stop("could not render a widget (posting denied); here is the data in text", 1),
    ])));
    node.install_runtimes(RuntimeRegistry::with_default(model as Arc<dyn ErasedModel>));

    // No `mcp:channel.post:call` — the agent inherits `poster ∩ agent`, so the post must be denied.
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body("show me the sites as a table", None, "run-widget-deny");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");
    drain_channel_agent_runs(&node, ws).await;

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    assert!(
        !items
            .iter()
            .any(|i| serde_json::from_str::<serde_json::Value>(&i.body)
                .map(|v| v["kind"] == "rich_result")
                .unwrap_or(false)),
        "a poster without channel.post must not get a rich_result item from the agent"
    );
    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("the run still completes with a text answer");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json");
    assert_eq!(parsed["kind"], "agent_result");
}

/// CHANNEL-WIDGETS genui gate: an agent that posts a genui `rich_result` in the WRONG IR dialect
/// (`type` instead of `component`, no per-component `id`, no `surface`) is rejected loudly at
/// `channel.post` — the `BadInput` feeds back into the loop and the corrected repost lands. Exactly
/// the dashboard.save behavior, now on the preview path: a broken IR never reaches the dock, and the
/// only genui item in history is the valid one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_malformed_genui_rich_result_is_rejected_and_the_corrected_repost_lands() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    let cid = "dock-3";

    // The live defect observed in the dock run (2026-07-06): the not-our-IR dialect.
    let wrong_dialect = serde_json::json!({
        "kind": "rich_result", "v": 2, "view": "genui",
        "options": { "genui": { "v": 1, "ir": {
            "components": { "root": { "type": "stack" } }
        } } },
    })
    .to_string();
    // The corrected IR: `component` + repeated `id` + `v:1` + `surface{surfaceId,root}`.
    let corrected = serde_json::json!({
        "kind": "rich_result", "v": 2, "view": "genui",
        "options": { "genui": { "v": 1, "ir": {
            "v": 1,
            "surface": { "surfaceId": "s1", "root": "root" },
            "components": {
                "root": { "id": "root", "component": "stack", "children": ["t1"] },
                "t1": { "id": "t1", "component": "text", "props": { "text": "sites" } }
            }
        } } },
    })
    .to_string();

    let model = Arc::new(AiGateway::new(MockProvider::new(vec![
        lb_role_ai_gateway::AiResponse::calls(
            "posting a genui widget",
            vec![lb_role_ai_gateway::ToolCall {
                id: "c1".into(),
                name: "channel.post".into(),
                input:
                    serde_json::json!({ "cid": cid, "id": "w-bad", "ts": 2, "body": wrong_dialect })
                        .to_string(),
            }],
            1,
        ),
        // The loop fed the BadInput back; the "model" self-corrects and reposts.
        lb_role_ai_gateway::AiResponse::calls(
            "fixing the IR",
            vec![lb_role_ai_gateway::ToolCall {
                id: "c2".into(),
                name: "channel.post".into(),
                input:
                    serde_json::json!({ "cid": cid, "id": "w-good", "ts": 3, "body": corrected })
                        .to_string(),
            }],
            1,
        ),
        AiResponse::stop("here is your widget", 1),
    ])));
    node.install_runtimes(RuntimeRegistry::with_default(model as Arc<dyn ErasedModel>));

    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
            "mcp:channel.post:call",
        ],
    );

    let body = agent_request_body("build me a genui widget", None, "run-genui-gate");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");
    drain_channel_agent_runs(&node, ws).await;

    // The run survived the rejected post and answered.
    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("agent_result posted");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json");
    assert_eq!(parsed["answer"], "here is your widget");

    // ONLY the corrected genui widget landed; the wrong-dialect one never reached the channel.
    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let genui_items: Vec<_> = items
        .iter()
        .filter(|i| {
            serde_json::from_str::<serde_json::Value>(&i.body)
                .map(|v| v["kind"] == "rich_result" && v["view"] == "genui")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        genui_items.len(),
        1,
        "exactly one (valid) genui rich_result must land"
    );
    assert_eq!(genui_items[0].id, "w-good");
    assert!(!items.iter().any(|i| i.id == "w-bad"));
}

/// CHANNEL-WIDGETS (no-`channel.post` dock path): an agent that emits its widget as a fenced
/// ```lb-widget block INSIDE its final answer text gets the envelope split off by the worker — the
/// durable `agent_result` carries the STRIPPED prose answer, and a separate `rich_result` widget
/// item lands in the same dock channel under the worker's authorship, correlated to the run
/// (`w:<job>`). The model never calls `channel.post`; the worker owns the cid. This is the path the
/// agent dock uses — the model is not asked to discover a channel id or fight arg schemas.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_widget_block_in_the_answer_is_split_off_by_the_worker_no_channel_post() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    let cid = "dock-user-ada-widget-block";

    let envelope = serde_json::json!({
        "kind": "rich_result", "v": 2, "view": "genui",
        "options": { "genui": { "v": 1, "ir": {
            "v": 1,
            "surface": { "surfaceId": "s1", "root": "root" },
            "components": {
                "root": { "id": "root", "component": "stack", "children": ["t1"] },
                "t1":   { "id": "t1", "component": "text", "props": { "value": "hi" } }
            }
        } } },
        "sources": [{ "refId": "A", "tool": "store.query", "args": { "sql": "SELECT 1" } }],
        "tools": ["store.query"],
    })
    .to_string();

    // The model emits the widget INSIDE its prose answer — no `channel.post` tool call.
    let answer = format!(
        "Here's the widget I built:\n\n```lb-widget\n{envelope}\n```\nLet me know if you want changes."
    );
    node.install_runtimes(RuntimeRegistry::with_default(answer_model(&answer)));

    // NOTE: no `mcp:channel.post:call` grant — the new path requires NO channel-post capability.
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body("build me a widget", None, "run-widget-block");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");
    drain_channel_agent_runs(&node, ws).await;

    // The agent_result is STRIPPED — the fenced block is gone from the persisted answer.
    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("agent_result posted");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json");
    assert_eq!(parsed["kind"], "agent_result");
    let answer_text = parsed["answer"].as_str().expect("answer string");
    assert!(
        !answer_text.contains("```lb-widget") && !answer_text.contains("\"view\":\"genui\""),
        "the fenced block must be stripped from the persisted answer: {answer_text}"
    );
    assert!(
        answer_text.contains("Here's the widget I built:") && answer_text.contains("Let me know"),
        "the surrounding prose stays: {answer_text}"
    );

    // A separate `rich_result` widget item landed in the same channel, under the worker's authorship,
    // correlated to the run (`w:<job>`).
    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let widget = items
        .iter()
        .find(|i| {
            serde_json::from_str::<serde_json::Value>(&i.body)
                .map(|v| v["kind"] == "rich_result")
                .unwrap_or(false)
        })
        .expect("the worker posted a rich_result widget item into the dock channel");
    assert_eq!(widget.author, "system:agent-worker");
    assert_eq!(widget.id, "w:run-widget-block");
    let w: serde_json::Value = serde_json::from_str(&widget.body).unwrap();
    assert_eq!(w["view"], "genui");
    assert_eq!(w["options"]["genui"]["ir"]["surface"]["root"], "root");

    // The widget item sorts AFTER the agent_result (worker posts widget at ts+1).
    let result_idx = items
        .iter()
        .position(|i| i.id == "a:run-widget-block")
        .expect("result present");
    let widget_idx = items
        .iter()
        .position(|i| i.id == "w:run-widget-block")
        .expect("widget present");
    assert!(widget_idx > result_idx, "widget lands after the answer");

    // WORKSPACE ISOLATION (mandatory): a ws-B principal cannot read the widget item.
    let outsider = principal("globex", &[&format!("bus:chan/{cid}:sub")]);
    assert!(
        history(&node.store, &outsider, "globex", cid)
            .await
            .map(|items| items.is_empty())
            .unwrap_or(true),
        "the widget item must be invisible outside its workspace"
    );
}

/// CHANNEL-WIDGETS (no-`channel.post` dock path): a present-but-INVALID fenced block (the wrong IR
/// dialect) is left in the answer text — no widget item lands, the user sees what the agent tried.
/// The worker is best-effort, not a second gate: a bad block is not a run fault.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_invalid_widget_block_in_the_answer_is_left_untouched_no_widget_lands() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    let cid = "dock-user-ada-widget-bad";

    // The wrong IR dialect (the live 2026-07-06 defect): `type` instead of `component`.
    let bad = serde_json::json!({
        "kind": "rich_result", "v": 2, "view": "genui",
        "options": { "genui": { "v": 1, "ir": {
            "components": { "root": { "type": "stack" } }
        } } },
    })
    .to_string();
    let answer = format!("```lb-widget\n{bad}\n```");
    node.install_runtimes(RuntimeRegistry::with_default(answer_model(&answer)));

    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body("build me a widget", None, "run-widget-bad");
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");
    drain_channel_agent_runs(&node, ws).await;

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    // No widget item landed.
    assert!(
        !items
            .iter()
            .any(|i| serde_json::from_str::<serde_json::Value>(&i.body)
                .map(|v| v["kind"] == "rich_result")
                .unwrap_or(false)),
        "an invalid block must not yield a widget item"
    );
    // The block is LEFT in the persisted answer so the user sees the agent's attempt.
    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("agent_result posted");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json");
    let answer_text = parsed["answer"].as_str().expect("answer string");
    assert!(
        answer_text.contains("```lb-widget"),
        "an invalid block is left in the answer (visible to the user): {answer_text}"
    );
}

/// RENDER-WIDGETS (built-in view dock path): the no-`channel.post` extractor is GENERIC over the
/// `rich_result` view. A `view:"stat"` fenced block (the shipped single-number renderer — NOT genui)
/// is split off by the worker exactly like the genui case: the durable `agent_result` carries the
/// STRIPPED prose, and a separate `rich_result` widget item with `view:"stat"` lands in the same
/// dock channel under the worker's authorship, correlated to the run (`w:<job>`). This locks the
/// non-genui render path: a `stat`/`chart`/`gauge`/`table`/etc. preview works identically to `genui`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_stat_widget_block_in_the_answer_is_split_off_by_the_worker_no_channel_post() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    let cid = "dock-user-ada-stat-block";

    // A built-in `view:"stat"` envelope — the shipped StatPanel renderer, not genui. Bound to a real
    // store.query the worker re-runs at view time; `fieldConfig.defaults.unit` styles the value (the
    // render-widgets skill's "average session time" example).
    let envelope = serde_json::json!({
        "kind": "rich_result", "v": 2, "view": "stat",
        "source": { "tool": "store.query",
                    "args": { "sql": "SELECT duration_s AS value FROM session LIMIT 1" } },
        "options": { "reduceOptions": { "calcs": ["mean"], "fields": ["value"] },
                     "textMode": "auto", "colorMode": "value" },
        "fieldConfig": { "defaults": { "unit": "s" } },
        "tools": ["store.query"],
    })
    .to_string();

    // The model emits the widget INSIDE its prose answer — no `channel.post` tool call.
    let answer = format!(
        "Here's the average session time:\n\n```lb-widget\n{envelope}\n```\nWant me to pin it?"
    );
    node.install_runtimes(RuntimeRegistry::with_default(answer_model(&answer)));

    // NOTE: no `mcp:channel.post:call` grant — the dock path requires NO channel-post capability.
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
        ],
    );

    let body = agent_request_body(
        "make me a stat widget for avg session time",
        None,
        "run-stat-block",
    );
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("agent request posts");
    drain_channel_agent_runs(&node, ws).await;

    // The agent_result is STRIPPED — the fenced block (and the envelope JSON) is gone from the answer.
    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("agent_result posted");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).expect("json");
    assert_eq!(parsed["kind"], "agent_result");
    let answer_text = parsed["answer"].as_str().expect("answer string");
    assert!(
        !answer_text.contains("```lb-widget") && !answer_text.contains("\"view\":\"stat\""),
        "the fenced block must be stripped from the persisted answer: {answer_text}"
    );
    assert!(
        answer_text.contains("Here's the average session time:")
            && answer_text.contains("Want me to pin it?"),
        "the surrounding prose stays: {answer_text}"
    );

    // A separate `rich_result` widget item with view:"stat" landed in the same channel under the
    // worker's authorship, correlated to the run (`w:<job>`).
    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let widget = items
        .iter()
        .find(|i| {
            serde_json::from_str::<serde_json::Value>(&i.body)
                .map(|v| v["kind"] == "rich_result")
                .unwrap_or(false)
        })
        .expect("the worker posted a rich_result widget item into the dock channel");
    assert_eq!(widget.author, "system:agent-worker");
    assert_eq!(widget.id, "w:run-stat-block");
    let w: serde_json::Value = serde_json::from_str(&widget.body).unwrap();
    assert_eq!(w["view"], "stat", "the non-genui view survives the split");
    assert_eq!(w["source"]["tool"], "store.query");
    assert_eq!(w["options"]["reduceOptions"]["calcs"][0], "mean");
    assert_eq!(w["fieldConfig"]["defaults"]["unit"], "s");

    // The widget item sorts AFTER the agent_result (worker posts widget at ts+1).
    let result_idx = items
        .iter()
        .position(|i| i.id == "a:run-stat-block")
        .expect("result present");
    let widget_idx = items
        .iter()
        .position(|i| i.id == "w:run-stat-block")
        .expect("widget present");
    assert!(widget_idx > result_idx, "widget lands after the answer");

    // WORKSPACE ISOLATION (mandatory): a ws-B principal cannot read the widget item.
    let outsider = principal("globex", &[&format!("bus:chan/{cid}:sub")]);
    assert!(
        history(&node.store, &outsider, "globex", cid)
            .await
            .map(|items| items.is_empty())
            .unwrap_or(true),
        "the stat widget item must be invisible outside its workspace"
    );
}
