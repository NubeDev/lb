//! MANDATORY offline/sync (testing §2.3) for the must-deliver outbox — the S6 headline: an external
//! effect **survives a disconnect and is delivered at-least-once, idempotently** — never lost, never
//! double-sent. Three properties:
//!   1. a target outage does not lose the effect — it re-delivers on the next relay pass;
//!   2. a duplicate delivery is a no-op on the receiver (dedup on `idempotency_key`);
//!   3. the transactional enqueue is atomic — the job step and the effect commit together.
//!
//! Node-booting (the job-start path posts progress on the bus) → multi-thread flavor + a UNIQUE
//! workspace id per test. The GitHub `Target` is the only external stubbed (testing §3).

use std::sync::Arc;
use std::sync::Mutex;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    relay_outbox, request_approval, resolve_approval, start_coding_job, CodingJob, Node, PrSpec,
    Target,
};
use lb_inbox::Decision;
use lb_outbox::{Effect, EffectStatus};

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

const CAPS: &[&str] = &[
    "mcp:workflow.request_approval:call",
    "mcp:workflow.resolve_approval:call",
    "mcp:workflow.start_job:call",
    "bus:chan/*:pub", // start_job streams progress to the channel (motion)
];

/// A target that fails its first `fail_first` attempts (the disconnect), then succeeds, deduping on
/// the idempotency key so a re-delivery is a no-op (never double-sent).
struct FlakyGithub {
    fail_first: Mutex<u32>,
    delivered: Mutex<Vec<String>>,
    attempts: Mutex<u32>,
}
impl FlakyGithub {
    fn new(fail_first: u32) -> Self {
        Self {
            fail_first: Mutex::new(fail_first),
            delivered: Mutex::new(Vec::new()),
            attempts: Mutex::new(0),
        }
    }
}
impl Target for FlakyGithub {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        *self.attempts.lock().unwrap() += 1;
        let mut remaining = self.fail_first.lock().unwrap();
        if *remaining > 0 {
            *remaining -= 1;
            return Err("github unreachable (disconnect)".into());
        }
        let mut log = self.delivered.lock().unwrap();
        if !log.contains(&effect.idempotency_key) {
            log.push(effect.idempotency_key.clone()); // dedup — at-least-once → effectively once
        }
        Ok(())
    }
}

/// Drive the gated flow to the point where a PR effect is pending in the outbox.
async fn queue_a_pr(node: &Node, user: &Principal, ws: &str) {
    let pr = PrSpec::new("acme/api", "fix", "main", "scope", "");
    request_approval(&node.store, user, ws, "ap", "scope", "rev", &pr, 1)
        .await
        .unwrap();
    resolve_approval(&node.store, user, ws, "ap", Decision::Approved, 2)
        .await
        .unwrap();
    start_coding_job(
        node,
        user,
        ws,
        CodingJob {
            job_id: "job",
            approval_id: "ap",
            scope_doc: "scope",
            channel: "c",
            pr: &pr,
            pr_key: "pr:key",
            ts: 3,
        },
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_effect_survives_an_outage_and_is_delivered_at_least_once() {
    // The disconnect: the target fails the first attempt; the effect is NOT lost — the next pass
    // re-delivers and it ends `delivered`.
    let ws = "wf-offline-survive";
    let node = Arc::new(Node::boot().await.unwrap());
    let user = principal("user:ada", ws, CAPS);
    queue_a_pr(&node, &user, ws).await;

    let target = FlakyGithub::new(1); // first attempt fails (the outage)

    // Pass 1 at now=1: the target is down → the effect stays schedulable (failed), not lost.
    let p1 = relay_outbox(&node.store, ws, &target, 1).await.unwrap();
    assert_eq!(p1.failed, 1);
    assert_eq!(p1.delivered, 0);
    let still = lb_outbox::pending(&node.store, ws).await.unwrap();
    assert_eq!(still.len(), 1, "the effect survived the outage");
    assert_eq!(still[0].status, EffectStatus::Failed);

    // Pass 2 once the backoff has elapsed: the target is back → delivered. Never lost.
    let now2 = 1 + lb_outbox::backoff(1);
    let p2 = relay_outbox(&node.store, ws, &target, now2).await.unwrap();
    assert_eq!(p2.delivered, 1);
    assert!(
        lb_outbox::pending(&node.store, ws)
            .await
            .unwrap()
            .is_empty(),
        "delivered after retry"
    );
    assert_eq!(target.delivered.lock().unwrap().clone(), vec!["pr:key"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_duplicate_delivery_is_a_no_op_on_the_receiver() {
    // At-least-once means the receiver MUST dedup. Force a second relay pass against an
    // already-delivered key (simulating the relay crashing after delivery but before marking) — the
    // target sees the key twice but acts once (never double-sent).
    let ws = "wf-offline-dedup";
    let node = Arc::new(Node::boot().await.unwrap());
    let user = principal("user:ada", ws, CAPS);
    queue_a_pr(&node, &user, ws).await;

    let target = FlakyGithub::new(0);

    // Pass 1 delivers and marks the effect.
    relay_outbox(&node.store, ws, &target, 1).await.unwrap();
    // Re-enqueue the SAME effect id + key (the crash-before-mark replay): it is pending again.
    let replay = Effect::new("job-pr", "github", "create_pr", "{}", "pr:key", 9);
    lb_outbox::enqueue(
        &node.store,
        ws,
        "job",
        "job",
        &serde_json::json!({}),
        &replay,
    )
    .await
    .unwrap();
    // Pass 2 delivers again — the target is hit twice but the key lands once (dedup).
    relay_outbox(&node.store, ws, &target, 10).await.unwrap();

    assert_eq!(
        *target.attempts.lock().unwrap(),
        2,
        "the target was contacted twice (at-least-once)"
    );
    assert_eq!(
        target.delivered.lock().unwrap().clone(),
        vec!["pr:key"],
        "but the effect landed exactly once (dedup on idempotency_key — never double-sent)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_effect_and_the_job_step_commit_together() {
    // The transactional guarantee: after start_coding_job, BOTH the job step (the domain change) and
    // the pending effect are durable — written in one transaction (no orphaned effect, no silent
    // drop of the change).
    let ws = "wf-offline-tx";
    let node = Arc::new(Node::boot().await.unwrap());
    let user = principal("user:ada", ws, CAPS);
    queue_a_pr(&node, &user, ws).await;

    let job = lb_jobs::load(&node.store, ws, "job")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.steps.len(), 1, "the job step committed");
    let pending = lb_outbox::pending(&node.store, ws).await.unwrap();
    assert_eq!(pending.len(), 1, "the effect committed in the same tx");
    assert_eq!(pending[0].idempotency_key, "pr:key");
}
