//! The GitHub outbox `Target`, end to end — the real `reqwest` adapter delivering outbox effects to
//! a GitHub origin, driven through the host's `relay_outbox`. The egress counterpart to the webhook
//! ingress: an effect goes OUT through this `Target`, over a real socket, with the outbox's
//! at-least-once + dedup + backoff/dead-letter guarantees intact.
//!
//! The GitHub origin is the only external (testing §3): a fake served on `127.0.0.1:0` (the same
//! axum-on-ephemeral-port harness `http_source_test.rs` uses) whose behaviour each test scripts —
//! accept, "already exists" (422), or always-5xx. Store is real (in-memory SurrealDB); no Node/bus
//! is needed (the relay is pure store + the `Target` seam), so a plain multi-thread `tokio::test`.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use lb_host::relay_outbox;
use lb_outbox::{dead_lettered, enqueue, pending, Effect};
use lb_role_github_target::GithubTarget;
use serde_json::{json, Value};

// --- a fake GitHub origin -----------------------------------------------------------------------

/// What the fake GitHub does with a request, and how many it has seen.
#[derive(Clone)]
struct FakeGithub {
    /// `200` accept, `422` already-exists, or `503` always-down — scripted per test.
    mode: Mode,
    hits: Arc<AtomicUsize>,
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Accept,
    AlreadyExists,
    Down,
}

async fn create_pr(
    State(fake): State<FakeGithub>,
    Path(_repo): Path<(String, String)>,
    Json(_body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    fake.hits.fetch_add(1, Ordering::SeqCst);
    match fake.mode {
        Mode::Accept => (StatusCode::CREATED, Json(json!({ "number": 7 }))),
        Mode::AlreadyExists => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "message": "A pull request already exists for acme:fix." })),
        ),
        Mode::Down => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "message": "no" })),
        ),
    }
}

/// Serve the fake on an ephemeral port; return its base URL and the hit counter.
async fn serve(mode: Mode) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let fake = FakeGithub {
        mode,
        hits: hits.clone(),
    };
    let app = Router::new()
        .route("/repos/{owner}/{repo}/pulls", post(create_pr))
        .with_state(fake);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), hits)
}

// --- fixtures -----------------------------------------------------------------------------------

use lb_store::Store;

/// Enqueue a `create_pr` effect with a structured payload the real adapter can map.
async fn enqueue_pr(store: &Store, ws: &str, id: &str, key: &str, max_attempts: Option<u32>) {
    let mut effect = Effect::new(
        id,
        "github",
        "create_pr",
        r#"{"repo":"acme/api","head":"fix/2451","base":"main","title":"Fix race","body":"b"}"#,
        key,
        1,
    );
    if let Some(m) = max_attempts {
        effect = effect.with_max_attempts(m);
    }
    enqueue(store, ws, "job", "sess", &json!({ "step": "pr" }), &effect)
        .await
        .unwrap();
}

// === tests ======================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_create_pr_effect_delivers_to_github_over_http() {
    // HAPPY PATH over a real socket: the relay delivers the effect through the real adapter; GitHub
    // accepts (201); the effect ends delivered and is no longer scheduled.
    let store = Store::memory().await.unwrap();
    let ws = "ght-happy";
    enqueue_pr(&store, ws, "e1", "pr:2451", None).await;

    let (base, hits) = serve(Mode::Accept).await;
    let target = GithubTarget::new(&base, "tok-never-logged");

    let pass = relay_outbox(&store, ws, &target, 1).await.unwrap();
    assert_eq!(pass.delivered, 1, "GitHub accepted the PR");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "one POST hit the origin");
    assert!(
        pending(&store, ws).await.unwrap().is_empty(),
        "the delivered effect is no longer scheduled"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_already_exists_422_is_idempotent_success() {
    // IDEMPOTENCY: at-least-once means a re-delivery must be a no-op. GitHub returns 422 "already
    // exists" when a PR for the head is open — the adapter treats that as delivered, so the relay
    // never opens a second PR and the effect leaves the queue.
    let store = Store::memory().await.unwrap();
    let ws = "ght-idem";
    enqueue_pr(&store, ws, "e1", "pr:2451", None).await;

    let (base, hits) = serve(Mode::AlreadyExists).await;
    let target = GithubTarget::new(&base, "tok");

    let pass = relay_outbox(&store, ws, &target, 1).await.unwrap();
    assert_eq!(pass.delivered, 1, "422 already-exists counts as delivered");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(
        pending(&store, ws).await.unwrap().is_empty(),
        "an idempotent re-delivery does not keep re-queuing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_persistently_failing_target_dead_letters_after_the_cap() {
    // BACKOFF + DEAD-LETTER over the real adapter: GitHub is always 5xx (transient), so each pass
    // fails. With max_attempts=2 the effect dead-letters on the 2nd failure — parked, no longer
    // scheduled. Passes must advance `now` past the backoff so the 2nd attempt is actually due.
    let store = Store::memory().await.unwrap();
    let ws = "ght-deadletter";
    enqueue_pr(&store, ws, "e1", "pr:2451", Some(2)).await;

    let (base, _hits) = serve(Mode::Down).await;
    let target = GithubTarget::new(&base, "tok");

    // Pass 1 at now=1 fails → Failed (backoff).
    let p1 = relay_outbox(&store, ws, &target, 1).await.unwrap();
    assert_eq!(p1.failed, 1);
    assert_eq!(p1.dead_lettered, 0);

    // Pass 2 once the backoff elapsed → 2nd failure hits the cap → dead-lettered.
    let now2 = 1 + lb_outbox::backoff(1);
    let p2 = relay_outbox(&store, ws, &target, now2).await.unwrap();
    assert_eq!(
        p2.dead_lettered, 1,
        "the poison effect is parked at the cap"
    );

    assert!(
        pending(&store, ws).await.unwrap().is_empty(),
        "a dead-lettered effect is no longer scheduled"
    );
    let parked = dead_lettered(&store, ws).await.unwrap();
    assert_eq!(parked.len(), 1);
    assert_eq!(parked[0].attempts, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_transport_failure_leaves_the_effect_schedulable() {
    // TRANSIENT failure: the origin is unreachable (no server bound at the URL). The delivery errors,
    // the effect stays schedulable (failed, not dead-lettered on the first miss), and a later pass
    // against a live origin delivers it — never lost.
    let store = Store::memory().await.unwrap();
    let ws = "ght-transport";
    enqueue_pr(&store, ws, "e1", "pr:2451", None).await;

    // Point at a port nothing is listening on (an unreachable origin).
    let dead = GithubTarget::new("http://127.0.0.1:1", "tok");
    let p1 = relay_outbox(&store, ws, &dead, 1).await.unwrap();
    assert_eq!(p1.failed, 1, "an unreachable origin is a transient failure");
    assert_eq!(pending(&store, ws).await.unwrap().len(), 1, "still owed");

    // The origin comes back: a later pass (past the backoff) delivers it.
    let (base, _hits) = serve(Mode::Accept).await;
    let live = GithubTarget::new(&base, "tok");
    let now2 = 1 + lb_outbox::backoff(1);
    let p2 = relay_outbox(&store, ws, &live, now2).await.unwrap();
    assert_eq!(p2.delivered, 1, "delivered once the origin recovered");
    assert!(pending(&store, ws).await.unwrap().is_empty());
}
