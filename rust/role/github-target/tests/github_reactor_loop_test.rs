//! The **full close-the-loop** proof, end to end over a real socket: an approval lands `Approved`,
//! the host's resolution **reactor** auto-starts the coding job, the enriched `create_pr` effect
//! rides the outbox, and the real `GithubTarget` adapter opens the PR against a **fake GitHub on an
//! ephemeral port** — no manual `start_job`, no hand-shaped payload.
//!
//! This is the one test that exercises *both* halves the slice connected: the host producer
//! (reactor + enriched payload) and the egress adapter (`role/github-target`). It lives here because
//! this crate already owns the fake-GitHub harness (copied from `github_target_test.rs`). A `Node` is
//! booted (real embedded SurrealDB + in-proc Zenoh), so it is a multi-thread test with a unique
//! workspace. GitHub is the only external mocked (testing §3).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    react_to_approvals, reactor_job_id, relay_outbox, request_approval, resolve_approval, Node,
    PrSpec,
};
use lb_inbox::Decision;
use lb_role_github_target::GithubTarget;
use serde_json::{json, Value};

// --- a fake GitHub origin (the PR-opening endpoint) ---------------------------------------------

#[derive(Clone)]
struct FakeGithub {
    hits: Arc<AtomicUsize>,
    last_body: Arc<std::sync::Mutex<Value>>,
}

async fn create_pr(
    State(fake): State<FakeGithub>,
    Path(_repo): Path<(String, String)>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    fake.hits.fetch_add(1, Ordering::SeqCst);
    *fake.last_body.lock().unwrap() = body;
    (StatusCode::CREATED, Json(json!({ "number": 7 })))
}

/// Serve the fake on an ephemeral port; return its base URL, the hit counter, and the last body it
/// received (so the test can assert the enriched payload arrived intact).
async fn serve() -> (String, Arc<AtomicUsize>, Arc<std::sync::Mutex<Value>>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let last_body = Arc::new(std::sync::Mutex::new(Value::Null));
    let fake = FakeGithub {
        hits: hits.clone(),
        last_body: last_body.clone(),
    };
    let app = Router::new()
        .route("/repos/{owner}/{repo}/pulls", post(create_pr))
        .with_state(fake);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), hits, last_body)
}

// --- principal helper ---------------------------------------------------------------------------

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_approval_opens_a_real_pr_through_the_reactor_and_the_github_target() {
    // THE FULL LOOP: approval → reactor auto-starts the job → enriched create_pr effect → outbox →
    // the real GithubTarget opens the PR against the fake origin, over a real socket. One test, both
    // halves of the slice.
    let ws = "loop-ws";
    let node = Arc::new(Node::boot().await.unwrap());
    let svc = principal(
        "ext:coding-workflow",
        ws,
        &[
            "mcp:workflow.request_approval:call",
            "mcp:workflow.resolve_approval:call",
            "mcp:workflow.start_job:call",
            "bus:chan/*:pub",
        ],
    );

    // Producer side: request approval (recording the enriched PR spec) and approve it.
    let pr = PrSpec::new(
        "acme/api",
        "fix/2451",
        "main",
        "Fix the token race",
        "closes #2451",
    );
    request_approval(
        &node.store,
        &svc,
        ws,
        "ap1",
        "scope-2451",
        "reviewers",
        &pr,
        1,
    )
    .await
    .unwrap();
    resolve_approval(&node.store, &svc, ws, "ap1", Decision::Approved, 2)
        .await
        .unwrap();

    // The reactor closes the loop: no manual start_job.
    let pass = react_to_approvals(&node, &svc, ws, "issue-2451", 3)
        .await
        .unwrap();
    assert_eq!(pass.started, 1, "the reactor auto-started the job");
    assert!(lb_jobs::load(&node.store, ws, &reactor_job_id("ap1"))
        .await
        .unwrap()
        .is_some());

    // Egress side: the real adapter delivers the queued effect to the fake GitHub over HTTP.
    let (base, hits, last_body) = serve().await;
    let target = GithubTarget::new(&base, "tok-never-logged");
    let rp = relay_outbox(&node.store, ws, &target, 4).await.unwrap();
    assert_eq!(rp.delivered, 1, "the PR was opened");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "one POST hit GitHub");

    // The enriched payload arrived intact at the GitHub endpoint — a real, openable PR.
    let body = last_body.lock().unwrap().clone();
    assert_eq!(body["head"], "fix/2451");
    assert_eq!(body["base"], "main");
    assert_eq!(body["title"], "Fix the token race");
    assert_eq!(body["body"], "closes #2451");

    // Idempotency end to end: a second reactor pass + relay opens NO second PR (job exists; the
    // create_pr dedups on the stable pr:ap1 key).
    let pass2 = react_to_approvals(&node, &svc, ws, "issue-2451", 5)
        .await
        .unwrap();
    assert_eq!(pass2.started, 0, "no second job");
    assert!(lb_outbox::pending(&node.store, ws)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "still exactly one PR opened"
    );
}
