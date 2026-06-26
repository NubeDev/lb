//! The background driver, end to end: one tick auto-starts every approved job (reactor) and delivers
//! every due PR effect (relay), per workspace, with the workspace wall held structurally. Real
//! embedded SurrealDB + in-proc Zenoh (a `Node` is booted → multi-thread + a unique ws per test); the
//! GitHub sink is the only external (a recording `Target`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{request_approval, resolve_approval, Node, PrSpec, Target};
use lb_inbox::Decision;
use lb_outbox::Effect;
use lb_role_github_workflow::{drive_once, WorkflowBinding};

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

fn caps() -> Vec<&'static str> {
    vec![
        "mcp:workflow.request_approval:call",
        "mcp:workflow.resolve_approval:call",
        "mcp:workflow.start_job:call",
        "bus:chan/*:pub",
    ]
}

/// The GitHub sink (the only external): records every delivered effect's idempotency key.
#[derive(Default)]
struct RecordingTarget {
    keys: std::sync::Mutex<Vec<String>>,
}
impl Target for RecordingTarget {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        self.keys
            .lock()
            .unwrap()
            .push(effect.idempotency_key.clone());
        Ok(())
    }
}

/// A binding driving `ws` as a freshly-minted service principal.
fn binding(ws: &str) -> WorkflowBinding {
    WorkflowBinding::new(
        ws,
        principal("ext:coding-workflow", ws, &caps()),
        "progress",
    )
}

/// Request + approve a coding job in `ws` (the state the driver reacts to).
async fn approve(node: &Node, b: &WorkflowBinding, approval_id: &str) {
    let pr = PrSpec::new("acme/api", "fix", "main", "Fix it", "body");
    request_approval(
        &node.store,
        &b.principal,
        &b.ws,
        approval_id,
        "scope",
        "rev",
        &pr,
        1,
    )
    .await
    .unwrap();
    resolve_approval(
        &node.store,
        &b.principal,
        &b.ws,
        approval_id,
        Decision::Approved,
        2,
    )
    .await
    .unwrap();
}

fn no_errors(ws: &str, e: String) {
    panic!("unexpected driver error in {ws}: {e}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_tick_starts_the_job_and_delivers_the_pr() {
    // THE DRIVER HEADLINE: a single tick closes the loop — reactor starts the approved job, relay
    // delivers its PR effect — with no manual call to either verb.
    let node = Arc::new(Node::boot().await.unwrap());
    let b = binding("drv-happy");
    approve(&node, &b, "ap1").await;

    let target = RecordingTarget::default();
    let tick = drive_once(&node, &[b], &target, 10, no_errors).await;

    assert_eq!(tick.started, 1, "the reactor started the approved job");
    assert_eq!(
        tick.delivered, 1,
        "the relay delivered the PR in the SAME tick"
    );
    assert_eq!(
        target.keys.lock().unwrap().as_slice(),
        &["pr:ap1".to_string()]
    );
    assert!(
        lb_outbox::pending(&node.store, "drv-happy")
            .await
            .unwrap()
            .is_empty(),
        "nothing left owed after the tick"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_second_tick_is_a_no_op() {
    // Idempotency at the loop level: re-ticking does not re-start the job or re-deliver the PR.
    let node = Arc::new(Node::boot().await.unwrap());
    let b = binding("drv-idem");
    approve(&node, &b, "ap1").await;
    let target = RecordingTarget::default();

    let t1 = drive_once(&node, std::slice::from_ref(&b), &target, 10, no_errors).await;
    assert_eq!((t1.started, t1.delivered), (1, 1));

    let t2 = drive_once(&node, &[b], &target, 11, no_errors).await;
    assert_eq!(
        (t2.started, t2.delivered),
        (0, 0),
        "the second tick does nothing"
    );
    assert_eq!(
        target.keys.lock().unwrap().len(),
        1,
        "exactly one PR ever delivered"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_tick_over_one_workspace_never_touches_another() {
    // MANDATORY workspace-isolation (§2.2): ws-A and ws-B both have an approved job, but a tick whose
    // bindings list only ws-A starts/delivers A's and leaves B's entirely untouched.
    let node = Arc::new(Node::boot().await.unwrap());
    let a = binding("drv-iso-a");
    let b = binding("drv-iso-b");
    approve(&node, &a, "ap1").await;
    approve(&node, &b, "ap1").await;

    let target = RecordingTarget::default();
    // The tick services ONLY ws-A.
    let tick = drive_once(&node, &[a], &target, 10, no_errors).await;
    assert_eq!((tick.started, tick.delivered), (1, 1));
    assert_eq!(
        target.keys.lock().unwrap().as_slice(),
        &["pr:ap1".to_string()]
    );

    // ws-B still has its approval owed — the driver never crossed the wall.
    assert!(
        lb_jobs::load(&node.store, "drv-iso-b", "job:ap1")
            .await
            .unwrap()
            .is_none(),
        "ws-B's job was NOT started by a ws-A tick"
    );
    assert!(
        lb_outbox::pending(&node.store, "drv-iso-b")
            .await
            .unwrap()
            .is_empty(),
        "ws-B has no effect yet (its job never started)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_injected_clock_advances_each_tick() {
    // The clock is injected (no wall-clock in the crate): two bindings serviced across ticks with a
    // deterministic counter. Proves the driver threads `now` through to both verbs.
    let node = Arc::new(Node::boot().await.unwrap());
    let a = binding("drv-clk-a");
    let b = binding("drv-clk-b");
    approve(&node, &a, "ap1").await;
    let target = RecordingTarget::default();
    let clock = AtomicU64::new(100);

    // Tick 1 at now=100: A is approved → starts + delivers; B has nothing.
    let now = clock.fetch_add(1, Ordering::SeqCst);
    let t1 = drive_once(&node, &[a, b.clone()], &target, now, no_errors).await;
    assert_eq!((t1.started, t1.delivered), (1, 1));

    // B approves between ticks; tick 2 at now=101 picks it up.
    approve(&node, &b, "ap2").await;
    let now = clock.fetch_add(1, Ordering::SeqCst);
    let t2 = drive_once(&node, &[b], &target, now, no_errors).await;
    assert_eq!((t2.started, t2.delivered), (1, 1));
    assert_eq!(
        clock.load(Ordering::SeqCst),
        102,
        "the clock advanced two ticks"
    );
}
