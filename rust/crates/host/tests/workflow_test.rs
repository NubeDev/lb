//! S6 EXIT GATE (the worked example, end to end): a GitHub issue → inbox `needs:triage` → the S5
//! agent triages + drafts a shared scope doc → a `needs:approval` inbox item that GENUINELY gates a
//! durable coding job → progress to a channel → every external effect through the outbox with retry.
//!
//! Plus the headline behaviors: the approval gate is real (no job before approval; exactly one
//! after), and the mandatory **capability-deny** category — each `workflow.*` verb refused without
//! its grant.
//!
//! The model provider and the GitHub `Target` are the only externals stubbed (testing §3); store +
//! bus + wasm are real. Multi-thread flavor + a UNIQUE workspace id per test (a node boots a Zenoh
//! peer; in-process peers share a workspace's keyspace — carry-forward from S3).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    ingest_issue, relay_outbox, request_approval, resolve_approval, start_coding_job, triage,
    CodingJob, Node, Target, WorkflowError, APPROVAL_CHANNEL, TRIAGE_CHANNEL,
};
use lb_inbox::Decision;
use lb_outbox::Effect;
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

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

// The full workflow grant + the substrate caps the agent needs at triage.
const INGEST: &str = "mcp:workflow.ingest_issue:call";
const TRIAGE: &str = "mcp:workflow.triage:call";
const REQ_APPROVAL: &str = "mcp:workflow.request_approval:call";
const RESOLVE: &str = "mcp:workflow.resolve_approval:call";
const START: &str = "mcp:workflow.start_job:call";
const AGENT_INVOKE: &str = "mcp:agent.invoke:call";
const DOC_R: &str = "store:doc/*:read";
const DOC_W: &str = "store:doc/*:write";
// The workflow posts progress + the triage summary to channels (motion) — that needs a bus pub
// grant, the same `bus:chan/*:pub` any channel poster holds.
const CHAN_PUB: &str = "bus:chan/*:pub";

fn full_caps() -> Vec<&'static str> {
    vec![
        INGEST,
        TRIAGE,
        REQ_APPROVAL,
        RESOLVE,
        START,
        AGENT_INVOKE,
        DOC_R,
        DOC_W,
        CHAN_PUB,
    ]
}

/// A gateway whose agent immediately answers with a drafted scope body (no tool calls — triage just
/// needs the draft). The only external model, deterministic (testing §3).
fn drafting_gateway() -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(vec![AiResponse::stop(
        "## Scope\nFix the token refresh race in acme/api.",
        7,
    )]))
}

/// The GitHub delivery target (the only external sink stubbed). Records every delivered effect and,
/// optionally, fails the first N attempts to exercise the retry path. Dedups on idempotency key.
struct GithubTarget {
    delivered: std::sync::Mutex<Vec<String>>,
    fail_first: std::sync::Mutex<u32>,
}
impl GithubTarget {
    fn new(fail_first: u32) -> Self {
        Self {
            delivered: std::sync::Mutex::new(Vec::new()),
            fail_first: std::sync::Mutex::new(fail_first),
        }
    }
    fn keys(&self) -> Vec<String> {
        self.delivered.lock().unwrap().clone()
    }
}
impl Target for GithubTarget {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        let mut remaining = self.fail_first.lock().unwrap();
        if *remaining > 0 {
            *remaining -= 1;
            return Err("github down".into());
        }
        let mut log = self.delivered.lock().unwrap();
        // Dedup on idempotency key — an at-least-once re-delivery is a no-op (outbox scope).
        if !log.contains(&effect.idempotency_key) {
            log.push(effect.idempotency_key.clone());
        }
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_full_coding_workflow_runs_end_to_end() {
    // THE S6 EXIT GATE: issue → triage + shared scope doc → approval gates → durable job → outbox.
    let ws = "wf-exit-gate";
    let node = Arc::new(Node::boot().await.unwrap());
    let user = principal("user:ada", ws, &full_caps());
    let gw = drafting_gateway();
    let agent_caps: Vec<String> = vec![DOC_R.into(), DOC_W.into()];

    // Step 1 — a GitHub issue arrives as an inbox `needs:triage` item.
    let issue = ingest_issue(
        &node.store,
        &user,
        ws,
        "issue-2451",
        "token refresh race",
        1,
    )
    .await
    .unwrap();
    assert_eq!(issue.channel, TRIAGE_CHANNEL);
    assert!(issue.body.contains("needs:triage"));

    // Step 2-4 — the agent triages + drafts a scope doc, shared to team `backend`.
    let triaged = triage(
        &node,
        &gw,
        &user,
        &agent_caps,
        ws,
        "issue-2451",
        "issue-2451",
        "scope-2451",
        "backend",
        None,
        &[],
        2,
    )
    .await
    .unwrap();
    assert_eq!(triaged.scope_doc, "scope-2451");

    // Step 5 — an approval inbox item is created, routed to `reviewers`.
    let approval = request_approval(
        &node.store,
        &user,
        ws,
        "approve-2451",
        "scope-2451",
        "reviewers",
        3,
    )
    .await
    .unwrap();
    assert_eq!(approval.channel, APPROVAL_CHANNEL);

    // The gate is REAL: before approval, the job cannot start and NO job record exists.
    let blocked = start_coding_job(
        &node,
        &user,
        ws,
        CodingJob {
            job_id: "job-2451",
            approval_id: "approve-2451",
            scope_doc: "scope-2451",
            channel: "issue-2451",
            pr_key: "pr:2451",
            ts: 4,
        },
    )
    .await;
    assert!(
        matches!(blocked, Err(WorkflowError::AwaitingApproval)),
        "the job is gated on approval"
    );
    assert!(
        lb_jobs::load(&node.store, ws, "job-2451")
            .await
            .unwrap()
            .is_none(),
        "no job record exists before approval — the gate is genuine, not cosmetic"
    );

    // A reviewer approves.
    resolve_approval(
        &node.store,
        &user,
        ws,
        "approve-2451",
        Decision::Approved,
        5,
    )
    .await
    .unwrap();

    // Step 6-8 — now the job starts, streams progress, and queues the PR through the outbox.
    let job_id = start_coding_job(
        &node,
        &user,
        ws,
        CodingJob {
            job_id: "job-2451",
            approval_id: "approve-2451",
            scope_doc: "scope-2451",
            channel: "issue-2451",
            pr_key: "pr:2451",
            ts: 6,
        },
    )
    .await
    .expect("approved → the job starts");
    assert_eq!(job_id, "job-2451");

    let job = lb_jobs::load(&node.store, ws, "job-2451")
        .await
        .unwrap()
        .expect("the durable job exists after approval");
    assert_eq!(job.status, lb_jobs::JobStatus::Done);
    assert_eq!(
        job.steps.len(),
        1,
        "the PR-queue step landed transactionally"
    );

    // The PR effect is in the outbox, pending — NOT sent directly.
    let pending = lb_outbox::pending(&node.store, ws).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].target, "github");
    assert_eq!(pending[0].idempotency_key, "pr:2451");

    // The relay delivers it (at-least-once). After one pass it is delivered, exactly once.
    let target = GithubTarget::new(0);
    let pass = relay_outbox(&node.store, ws, &target, 1).await.unwrap();
    assert_eq!(pass.delivered, 1);
    assert_eq!(target.keys(), vec!["pr:2451".to_string()]);
    assert!(
        lb_outbox::pending(&node.store, ws)
            .await
            .unwrap()
            .is_empty(),
        "the delivered effect is no longer scheduled"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_workflow_verb_is_denied_without_its_grant() {
    // MANDATORY capability-deny (§2.1): every workflow surface refuses without its specific grant.
    let ws = "wf-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // A caller with NO workflow caps at all.
    let nobody = principal("user:mallory", ws, &[]);

    let ingest = ingest_issue(&node.store, &nobody, ws, "i", "p", 1).await;
    assert!(matches!(ingest, Err(WorkflowError::Denied)));

    let req = request_approval(&node.store, &nobody, ws, "a", "d", "t", 1).await;
    assert!(matches!(req, Err(WorkflowError::Denied)));

    let res = resolve_approval(&node.store, &nobody, ws, "a", Decision::Approved, 1).await;
    assert!(matches!(res, Err(WorkflowError::Denied)));

    let start = start_coding_job(
        &node,
        &nobody,
        ws,
        CodingJob {
            job_id: "j",
            approval_id: "a",
            scope_doc: "d",
            channel: "c",
            pr_key: "k",
            ts: 1,
        },
    )
    .await;
    assert!(
        matches!(start, Err(WorkflowError::Denied)),
        "start_job is denied at the capability gate, before the approval gate"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_rejected_approval_never_starts_the_job() {
    // The gate, the other direction: a rejected (or deferred) resolution keeps the job unstarted.
    let ws = "wf-rejected";
    let node = Arc::new(Node::boot().await.unwrap());
    let user = principal("user:ada", ws, &full_caps());

    request_approval(&node.store, &user, ws, "a", "scope", "rev", 1)
        .await
        .unwrap();
    resolve_approval(&node.store, &user, ws, "a", Decision::Rejected, 2)
        .await
        .unwrap();

    let blocked = start_coding_job(
        &node,
        &user,
        ws,
        CodingJob {
            job_id: "j",
            approval_id: "a",
            scope_doc: "scope",
            channel: "c",
            pr_key: "k",
            ts: 3,
        },
    )
    .await;
    assert!(matches!(blocked, Err(WorkflowError::AwaitingApproval)));
    assert!(
        lb_jobs::load(&node.store, ws, "j").await.unwrap().is_none(),
        "a rejected approval leaves no job"
    );
}
