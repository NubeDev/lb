//! The **resolution reactor** — `react_to_approvals` auto-starts the coding job the moment its
//! approval lands `Approved`, closing webhook → triage → approval → JOB → outbox without a manual
//! `start_job` step (coding-workflow + outbox scope, the "close the loop" slice).
//!
//! These cover the reactor's contract through real host seams (real embedded SurrealDB + in-proc
//! Zenoh — a `Node` is booted, so multi-thread + a unique workspace per test): the enriched
//! `{repo,head,base,title,body}` payload reaches the outbox; the pass is idempotent (re-resolving
//! starts ONE job, never a second PR); the mandatory capability-deny and workspace-isolation
//! categories. The full loop **over a real GitHub socket** lives in
//! `role/github-target/tests/github_reactor_loop_test.rs` (it owns the fake-origin harness).
//!
//! The GitHub sink is the only external stubbed — a local recording `Target` (the relay's seam),
//! same shape as `workflow_test.rs`'s.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    react_to_approvals, reactor_job_id, relay_outbox, request_approval, resolve_approval, Node,
    PrSpec, Target,
};
use lb_inbox::Decision;
use lb_outbox::Effect;

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

const REQ: &str = "mcp:workflow.request_approval:call";
const RESOLVE: &str = "mcp:workflow.resolve_approval:call";
const START: &str = "mcp:workflow.start_job:call";
const CHAN_PUB: &str = "bus:chan/*:pub";

/// The full grant the workflow service principal (and a reviewer) needs to drive the loop.
fn service_caps() -> Vec<&'static str> {
    vec![REQ, RESOLVE, START, CHAN_PUB]
}

fn a_pr() -> PrSpec {
    PrSpec::new("acme/api", "fix/2451", "main", "Fix race", "the body")
}

/// The GitHub sink (the only external stubbed): records every delivered effect's payload + key.
#[derive(Default)]
struct RecordingTarget {
    delivered: std::sync::Mutex<Vec<(String, String)>>,
}
impl Target for RecordingTarget {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        self.delivered
            .lock()
            .unwrap()
            .push((effect.idempotency_key.clone(), effect.payload.clone()));
        Ok(())
    }
}

/// Request approval for a coding job (recording its PR spec) and approve it — the state the reactor
/// reacts to. Returns the approval id.
async fn approve(node: &Node, p: &Principal, ws: &str, approval_id: &str) {
    request_approval(
        &node.store,
        p,
        ws,
        approval_id,
        "scope",
        "reviewers",
        &a_pr(),
        1,
    )
    .await
    .unwrap();
    resolve_approval(&node.store, p, ws, approval_id, Decision::Approved, 2)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_approval_auto_starts_the_job_with_the_enriched_pr_payload() {
    // THE CLOSE-THE-LOOP HEADLINE: no manual start_job. A reactor pass over an `Approved` resolution
    // starts the durable job and queues the PR through the outbox — with the structured
    // {repo,head,base,title,body} payload github-target can actually map (was {scope_doc}).
    let ws = "react-happy";
    let node = Arc::new(Node::boot().await.unwrap());
    let svc = principal("ext:coding-workflow", ws, &service_caps());
    approve(&node, &svc, ws, "ap1").await;

    let pass = react_to_approvals(&node, &svc, ws, "issue-2451", 10)
        .await
        .unwrap();
    assert_eq!(pass.started, 1, "the approved job auto-started");
    assert_eq!(pass.already_started, 0);

    // The durable job exists at the reactor's deterministic id.
    let job_id = reactor_job_id("ap1");
    assert!(
        lb_jobs::load(&node.store, ws, &job_id)
            .await
            .unwrap()
            .is_some(),
        "the reactor created the durable job"
    );

    // The PR effect is queued with the ENRICHED payload (structured, not {scope_doc}).
    let pending = lb_outbox::pending(&node.store, ws).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].idempotency_key, "pr:ap1");
    let payload: serde_json::Value = serde_json::from_str(&pending[0].payload).unwrap();
    assert_eq!(payload["repo"], "acme/api");
    assert_eq!(payload["head"], "fix/2451");
    assert_eq!(payload["base"], "main");
    assert_eq!(payload["title"], "Fix race");

    // And it delivers through the relay — the enriched payload rides out to the (stubbed) target.
    let target = RecordingTarget::default();
    let rp = relay_outbox(&node.store, ws, &target, 11).await.unwrap();
    assert_eq!(rp.delivered, 1);
    let log = target.delivered.lock().unwrap();
    assert_eq!(log[0].0, "pr:ap1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_resolving_the_same_approval_starts_exactly_one_job() {
    // IDEMPOTENCY (mandatory): re-running the reactor (or re-resolving a deferred→approved item)
    // must start ONE job and queue ONE PR — never a second job, never a double PR.
    let ws = "react-idem";
    let node = Arc::new(Node::boot().await.unwrap());
    let svc = principal("ext:coding-workflow", ws, &service_caps());
    approve(&node, &svc, ws, "ap1").await;

    // First pass starts it.
    let p1 = react_to_approvals(&node, &svc, ws, "ch", 10).await.unwrap();
    assert_eq!(p1.started, 1);

    // A second pass (e.g. a re-resolve, or just the next scan) finds the job present → no-op.
    let p2 = react_to_approvals(&node, &svc, ws, "ch", 11).await.unwrap();
    assert_eq!(p2.started, 0, "no second job");
    assert_eq!(p2.already_started, 1, "the approval was already reacted to");

    // Re-resolving the same approval (deferred→approved is a valid path) still does not fork a job.
    resolve_approval(&node.store, &svc, ws, "ap1", Decision::Approved, 12)
        .await
        .unwrap();
    let p3 = react_to_approvals(&node, &svc, ws, "ch", 13).await.unwrap();
    assert_eq!(p3.started, 0);

    // Exactly one PR effect was ever queued (the create_pr dedups on the stable pr:ap1 key).
    let pending = lb_outbox::pending(&node.store, ws).await.unwrap();
    assert_eq!(pending.len(), 1, "one PR effect, never a duplicate");
    assert_eq!(pending[0].idempotency_key, "pr:ap1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_reactor_is_denied_without_the_start_job_grant() {
    // MANDATORY capability-deny (§2.1): the reactor runs `start_coding_job`, which re-checks
    // `mcp:workflow.start_job:call`. A service principal lacking that grant cannot start the job —
    // the loop does not close behind the capability wall, even with a landed approval.
    let ws = "react-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Has request/resolve but NOT start_job.
    let reviewer = principal("user:rev", ws, &[REQ, RESOLVE, CHAN_PUB]);
    approve(&node, &reviewer, ws, "ap1").await;

    // The reactor as the under-granted principal: the start_job gate refuses.
    let denied = react_to_approvals(&node, &reviewer, ws, "ch", 10).await;
    assert!(
        matches!(denied, Err(lb_host::WorkflowError::Denied)),
        "the reactor is refused at the start_job capability gate"
    );
    assert!(
        lb_jobs::load(&node.store, ws, &reactor_job_id("ap1"))
            .await
            .unwrap()
            .is_none(),
        "no job started without the grant"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_reactor_cannot_start_a_ws_a_job() {
    // MANDATORY workspace-isolation (§2.2): ws-A approves a job; a reactor pass over ws-B starts
    // nothing in ws-A (its `approved` scan selects ws-B's namespace — it never sees ws-A's
    // resolution). The hard wall holds at the scan, the spec read, and the gate.
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal("ext:coding-workflow", "react-iso-a", &service_caps());
    let b = principal("ext:coding-workflow", "react-iso-b", &service_caps());

    approve(&node, &a, "react-iso-a", "ap1").await;

    // A ws-B reactor pass sees no ws-A approval → starts nothing anywhere.
    let pass_b = react_to_approvals(&node, &b, "react-iso-b", "ch", 10)
        .await
        .unwrap();
    assert_eq!(pass_b.started, 0, "ws-B reactor sees no ws-A approval");
    assert!(
        lb_jobs::load(&node.store, "react-iso-a", &reactor_job_id("ap1"))
            .await
            .unwrap()
            .is_none(),
        "ws-A's job is untouched by a ws-B reactor"
    );

    // ws-A's own reactor does start it — proving the approval was genuinely there to react to.
    let pass_a = react_to_approvals(&node, &a, "react-iso-a", "ch", 11)
        .await
        .unwrap();
    assert_eq!(pass_a.started, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_approved_item_without_a_pr_spec_is_skipped() {
    // Not every approved inbox item is a coding-job request. An approval with no recorded PrSpec is
    // skipped silently — the reactor is safe to run over a workspace's whole resolution set.
    let ws = "react-nospec";
    let node = Arc::new(Node::boot().await.unwrap());
    let svc = principal("ext:coding-workflow", ws, &service_caps());

    // Resolve an approval that was never `request_approval`'d (so no PrSpec exists).
    resolve_approval(&node.store, &svc, ws, "bare", Decision::Approved, 1)
        .await
        .unwrap();

    let pass = react_to_approvals(&node, &svc, ws, "ch", 10).await.unwrap();
    assert_eq!(
        pass.started, 0,
        "no spec → not a coding-job request → skipped"
    );
    assert!(lb_outbox::pending(&node.store, ws)
        .await
        .unwrap()
        .is_empty());
}
