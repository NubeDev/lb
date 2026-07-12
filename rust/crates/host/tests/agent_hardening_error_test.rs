//! Slice D of agent-loop-hardening, at the LOOP level: scripted `MockProvider` faults drive the
//! real invoke → loop → job-persist path (real store, real bus; the provider is the one sanctioned
//! fake, rule 9). Lanes proven here:
//!   - **transient** — a 429 retries below step accounting (one turn, two attempts) and the run
//!     completes normally;
//!   - **transient exhausted** — a persistent 429 turns fatal after the bounded retries: the job is
//!     **Failed** (not a fake `Done`) and the answer names the fault;
//!   - **fatal** — a 401 fails the run on the first attempt, honestly.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{invoke, Invocation, Node};
use lb_jobs::JobStatus;
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ProviderFault};

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
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const INVOKE: &str = "mcp:agent.invoke:call";

async fn run(node: &Arc<Node>, ws: &str, job: &str, gw: &AiGateway<MockProvider>) -> String {
    let caller = principal("user:ada", ws, &[INVOKE]);
    invoke(
        node,
        gw,
        &caller,
        &[INVOKE.to_string()],
        ws,
        Invocation {
            job_id: job,
            goal: "do the thing",
            skill: None,
            doc: None,
            tools: &[],
            ts: 1,
        },
    )
    .await
    .expect("invoke settles (a turn fault is an honest terminal, not an Err)")
}

async fn status(node: &Arc<Node>, ws: &str, job: &str) -> JobStatus {
    lb_jobs::load(&node.store, ws, job)
        .await
        .expect("load")
        .expect("job exists")
        .status
}

/// A 429 with `Retry-After: 0` (no test sleep), then a real completion: the retry lane recovers the
/// SAME turn and the run finishes `Done` with the real answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_transient_429_retries_and_the_run_completes() {
    let ws = "hard-transient";
    let node = Arc::new(Node::boot().await.unwrap());
    let gw = AiGateway::new(MockProvider::scripted(vec![
        Err(ProviderFault::http(429, Some(0), "rate limited")),
        Ok(AiResponse::stop("recovered answer", 5)),
    ]));

    let answer = run(&node, ws, "job-transient", &gw).await;
    assert_eq!(answer, "recovered answer");
    assert_eq!(status(&node, ws, "job-transient").await, JobStatus::Done);
}

/// Every attempt 429s: after the bounded retries the run ends honestly — job **Failed**, the
/// answer names the fault. Never a fake `Done` and never an infinite retry spin.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_persistent_transient_fault_fails_the_run_honestly() {
    let ws = "hard-exhaust";
    let node = Arc::new(Node::boot().await.unwrap());
    let fault = || Err(ProviderFault::http(429, Some(0), "rate limited"));
    let gw = AiGateway::new(MockProvider::scripted(vec![
        fault(),
        fault(),
        fault(),
        fault(),
    ]));

    let answer = run(&node, ws, "job-exhaust", &gw).await;
    assert!(
        answer.contains("[run failed:") && answer.contains("rate limited"),
        "the terminal answer names the fault, got: {answer}"
    );
    assert_eq!(status(&node, ws, "job-exhaust").await, JobStatus::Failed);
}

/// A 401 is fatal on the FIRST attempt — no retry (the script's second entry must never be
/// consumed), job Failed, attributed answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_auth_failure_is_fatal_without_retry() {
    let ws = "hard-fatal";
    let node = Arc::new(Node::boot().await.unwrap());
    let gw = AiGateway::new(MockProvider::scripted(vec![
        Err(ProviderFault::http(401, None, "bad key")),
        Ok(AiResponse::stop("must never be reached", 5)),
    ]));

    let answer = run(&node, ws, "job-fatal", &gw).await;
    assert!(
        answer.contains("[run failed:") && answer.contains("bad key"),
        "attributed terminal answer, got: {answer}"
    );
    assert_eq!(status(&node, ws, "job-fatal").await, JobStatus::Failed);

    // Prove no retry consumed the second script entry: a fresh run over the SAME provider state
    // would get entry #2 next — instead assert via a second invoke on a new job that the mock is
    // at entry #2 (the fatal consumed exactly one).
    let answer2 = run(&node, ws, "job-fatal-2", &gw).await;
    assert_eq!(answer2, "must never be reached");
}
