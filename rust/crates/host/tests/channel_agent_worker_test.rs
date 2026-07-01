//! Channel agent-worker tests (channels-agent scope), over a REAL `Node` + the REAL `post` path and
//! the REAL in-house agent loop driven through the runtime seam. The ONLY stubbed external is the
//! model provider HTTP (`MockProvider`, rule 9 — store + bus + loop + channel are all real). The
//! worker is installed on the node via `Node::install_runtimes` exactly as a feature-on node would
//! install external runtimes; here the installed `default` is the in-house loop over a fixed-answer
//! provider, so a posted `kind:"agent"` item drives a genuine run and posts a real `agent_result`.
//!
//! Mandatory invariants (mirroring the query worker's tests + testing-scope §2.1):
//!   - HAPPY PATH: an `agent` request from a granted poster yields an `agent_result` carrying the run's
//!     answer, posted under `system:agent-worker`, correlated to the run (`a:<job>`).
//!   - CAPABILITY DENY (opaque): a poster WITHOUT `mcp:agent.invoke:call` gets an `agent_error` whose
//!     message is EXACTLY "agent not permitted".
//!   - UNKNOWN RUNTIME (opaque): a named runtime that isn't registered collapses to the SAME "agent not
//!     permitted" — no runtime-existence leak.
//!   - RE-ENTRANCY: posting an `agent_result` item does NOT re-trigger the worker.
//!   - WORKSPACE ISOLATION: the result lands in the poster's workspace only; a ws-B reader can't see it.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history, post, ErasedModel, Node, RuntimeRegistry};
use lb_inbox::Item;
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};
use std::sync::Arc;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_agent_request_yields_an_agent_result_with_the_run_answer() {
    let node = Node::boot().await.expect("node boots");
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

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let result = items
        .iter()
        .find(|i| i.author == "system:agent-worker")
        .expect("the worker posted an agent_result");
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
async fn a_request_without_the_invoke_grant_yields_opaque_agent_not_permitted() {
    let node = Node::boot().await.expect("node boots");
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

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let err = items
        .iter()
        .find(|i| i.author == "system:agent-worker")
        .expect("the worker posted an agent_error");
    let parsed: serde_json::Value = serde_json::from_str(&err.body).expect("json body");
    assert_eq!(parsed["kind"], "agent_error");
    assert_eq!(
        parsed["error"], "agent not permitted",
        "the deny is opaque (no capability leak)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_named_unknown_runtime_collapses_to_the_same_opaque_deny() {
    let node = Node::boot().await.expect("node boots");
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

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let err = items
        .iter()
        .find(|i| i.author == "system:agent-worker")
        .expect("the worker posted an agent_error");
    let parsed: serde_json::Value = serde_json::from_str(&err.body).expect("json body");
    assert_eq!(parsed["error"], "agent not permitted");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn posting_an_agent_result_item_does_not_re_trigger_the_worker() {
    let node = Node::boot().await.expect("node boots");
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

    // The worker's OWN output shape — re-posting it must not spawn another run (infinite-loop guard).
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

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    assert_eq!(items.len(), 1, "no worker item spawned: {items:?}");
    assert!(!items.iter().any(|i| i.author == "system:agent-worker"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_result_is_workspace_scoped_and_not_visible_from_another_workspace() {
    let node = Node::boot().await.expect("node boots");
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
    assert!(
        history(&node.store, &ada, "acme", cid)
            .await
            .unwrap()
            .iter()
            .any(|i| i.author == "system:agent-worker"),
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
