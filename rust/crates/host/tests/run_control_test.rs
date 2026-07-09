//! **Run lifecycle controls** (agent-dock scope) — STOP / PAUSE / RESUME over the REAL store + bus +
//! the REAL in-house loop + the REAL channel worker path. The only stubbed external is the model
//! provider (`MockProvider`, rule 9). Proves:
//!   - the host verbs move the durable run-job status (`pause_run`→Suspended, `stop_run`→Cancelled,
//!     `resume_run`→Running) and re-arm the channel enqueue job;
//!   - the LOOP honors a pause at its turn boundary — a pre-suspended run drives to a Suspended return
//!     WITHOUT completing (no answer), and the transcript/cursor are intact so a resume continues;
//!   - the WORKER classifies the outcome: a paused run posts NOTHING (no answer of record yet); a
//!     stopped run posts the honest `run stopped` `agent_error`;
//!   - MANDATORY capability-deny: no `mcp:agent.control:call` → opaque `Denied`;
//!   - MANDATORY workspace-isolation: a ws-B principal cannot control a ws-A run.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    drain_channel_agent_runs, history, pause_run, post, resume_run, stop_run, AgentError,
    ErasedModel, Node, RuntimeRegistry,
};
use lb_inbox::Item;
use lb_jobs::{create, load, Job, JobStatus};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};
use std::sync::Arc;

const INVOKE: &str = "mcp:agent.invoke:call";
const CONTROL: &str = "mcp:agent.control:call";

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
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

fn answer_model(answer: &str) -> Arc<dyn ErasedModel> {
    Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
        answer, 1,
    )])))
}

fn install_answer_runtime(node: &Node, answer: &str) {
    node.install_runtimes(RuntimeRegistry::with_default(answer_model(answer)));
}

fn agent_body(goal: &str, job: &str) -> String {
    serde_json::json!({ "kind": "agent", "goal": goal, "job": job }).to_string()
}

async fn worker_item(node: &Node, p: &Principal, ws: &str, cid: &str) -> Option<Item> {
    history(&node.store, p, ws, cid)
        .await
        .expect("history")
        .into_iter()
        .find(|i| i.author == "system:agent-worker")
}

// ── the host verbs move the durable status + re-arm the enqueue ───────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pause_then_resume_moves_the_status_and_rearms_the_enqueue() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "ctl-lifecycle";
    let ctl = principal("user:ada", ws, &[CONTROL]);
    let run_job = "run-ctl-1";

    // A Running run job + its RETIRED (Done) channel enqueue job — the state right after a drive that
    // was paused (the worker retires the enqueue below its drive).
    create(
        &node.store,
        ws,
        &Job::new(run_job, "agent-session", "goal", 1),
    )
    .await
    .unwrap();
    let enqueue_id = format!("q:{run_job}");
    let mut enqueue = Job::new(
        &enqueue_id,
        "channel-agent-run",
        enqueue_payload(run_job),
        1,
    );
    enqueue.status = JobStatus::Done; // retired by the prior drive
    create(&node.store, ws, &enqueue).await.unwrap();

    // PAUSE → the run job is Suspended (resumable).
    pause_run(&node, &ctl, ws, run_job).await.expect("pause ok");
    assert_eq!(status(&node, ws, run_job).await, JobStatus::Suspended);

    // RESUME → the run job is Running again AND the enqueue job is re-armed to Running (reactor re-picks).
    resume_run(&node, &ctl, ws, run_job)
        .await
        .expect("resume ok");
    assert_eq!(status(&node, ws, run_job).await, JobStatus::Running);
    assert_eq!(
        status(&node, ws, &enqueue_id).await,
        JobStatus::Running,
        "resume re-armed the enqueue job so the reactor re-drives it"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn stop_cancels_the_run_terminally() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "ctl-stop";
    let ctl = principal("user:ada", ws, &[CONTROL]);
    let run_job = "run-ctl-stop";
    create(
        &node.store,
        ws,
        &Job::new(run_job, "agent-session", "goal", 1),
    )
    .await
    .unwrap();

    stop_run(&node, &ctl, ws, run_job).await.expect("stop ok");
    assert_eq!(status(&node, ws, run_job).await, JobStatus::Cancelled);
    // Terminal + non-restartable: a resume of a cancelled run is a bad-state error (not a silent revive).
    assert!(matches!(
        resume_run(&node, &ctl, ws, run_job).await,
        Err(AgentError::BadInput(_))
    ));
}

// ── the LOOP honors a pause at its turn boundary (no completion, transcript intact) ───────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_paused_run_drives_to_suspended_without_completing_then_resume_finishes_it() {
    let node = Arc::new(Node::boot().await.unwrap());
    install_answer_runtime(&node, "the finished answer");
    let ws = "ctl-loop";
    let cid = "dock-user-ada-x";
    let run_job = "run-ctl-loop";
    let p = principal(
        "user:ada",
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
            CONTROL,
        ],
    );

    // Post the agent request (enqueues the durable run).
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", agent_body("summarize", run_job), 1),
    )
    .await
    .expect("agent request posts");

    // PAUSE the run BEFORE the reactor drives it: create the run job as Suspended so the loop's first
    // `is_paused` check fires. (In production a pause lands between turns; here we make the boundary
    // deterministic.) The enqueue job already exists from `post`.
    create(&node.store, ws, &suspended_run(run_job))
        .await
        .unwrap();

    // Drive: the loop loads a Suspended (resumable) job, hits the turn-boundary pause check, emits
    // Suspended, and returns WITHOUT completing. The worker classifies it as paused → posts NOTHING.
    drain_channel_agent_runs(&node, ws).await;
    assert!(
        worker_item(&node, &p, ws, cid).await.is_none(),
        "a paused run posts no answer of record (it is not done)"
    );
    assert_eq!(
        status(&node, ws, run_job).await,
        JobStatus::Suspended,
        "the run stays paused after the drive"
    );

    // RESUME → re-arms the enqueue; the next drain re-drives the run, which now completes and posts the
    // durable answer.
    resume_run(&node, &p, ws, run_job).await.expect("resume ok");
    drain_channel_agent_runs(&node, ws).await;
    let result = worker_item(&node, &p, ws, cid)
        .await
        .expect("the resumed run posts its answer");
    let parsed: serde_json::Value = serde_json::from_str(&result.body).unwrap();
    assert_eq!(parsed["kind"], "agent_result");
    assert_eq!(parsed["answer"], "the finished answer");
}

// ── the WORKER classifies a STOPPED run → the honest `run stopped` error ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_stopped_run_posts_the_honest_run_stopped_error() {
    let node = Arc::new(Node::boot().await.unwrap());
    install_answer_runtime(&node, "unused answer");
    let ws = "ctl-stopped";
    let cid = "dock-user-ada-y";
    let run_job = "run-ctl-stopped";
    let p = principal(
        "user:ada",
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            INVOKE,
            CONTROL,
        ],
    );
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", agent_body("go", run_job), 1),
    )
    .await
    .expect("posts");
    // Cancel the run job before the drive → the loop returns at the boundary; the worker classifies it.
    create(&node.store, ws, &cancelled_run(run_job))
        .await
        .unwrap();

    drain_channel_agent_runs(&node, ws).await;
    let item = worker_item(&node, &p, ws, cid)
        .await
        .expect("an item is posted");
    let parsed: serde_json::Value = serde_json::from_str(&item.body).unwrap();
    assert_eq!(parsed["kind"], "agent_error");
    assert_eq!(
        parsed["error"], "run stopped",
        "a stopped run posts the honest terminal error"
    );
}

// ── MANDATORY deny + isolation ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn control_without_the_cap_is_denied() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "ctl-deny";
    let run_job = "run-deny";
    create(&node.store, ws, &Job::new(run_job, "agent-session", "g", 1))
        .await
        .unwrap();
    // A principal WITHOUT mcp:agent.control:call — every control verb is an opaque Denied.
    let nope = principal("user:bob", ws, &["mcp:series.latest:call"]);
    assert!(matches!(
        pause_run(&node, &nope, ws, run_job).await,
        Err(AgentError::Denied)
    ));
    assert!(matches!(
        stop_run(&node, &nope, ws, run_job).await,
        Err(AgentError::Denied)
    ));
    assert!(matches!(
        resume_run(&node, &nope, ws, run_job).await,
        Err(AgentError::Denied)
    ));
    // And the run is untouched (still Running) — a denied control never moved the status.
    assert_eq!(status(&node, ws, run_job).await, JobStatus::Running);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_cannot_control_a_ws_a_run() {
    let node = Arc::new(Node::boot().await.unwrap());
    // The run lives in ws-A.
    create(
        &node.store,
        "ws-a",
        &Job::new("run-a", "agent-session", "g", 1),
    )
    .await
    .unwrap();
    // A ws-B principal WITH the control cap — but its ws is B, so it can't reach ws-A's job row: the
    // control verbs run against ws-B (the token's ws, the hard wall), where the run does not exist →
    // a BadInput (no such job), and ws-A's run is never touched.
    let b = principal("user:bob", "ws-b", &[CONTROL]);
    assert!(pause_run(&node, &b, "ws-b", "run-a").await.is_err());
    // The ws-A run is still Running — untouched by the ws-B control attempt (the workspace wall).
    assert_eq!(status(&node, "ws-a", "run-a").await, JobStatus::Running);
}

// ── helpers ───────────────────────────────────────────────────────────────────────────────────────

async fn status(node: &Node, ws: &str, id: &str) -> JobStatus {
    load(&node.store, ws, id).await.unwrap().unwrap().status
}

/// The enqueue payload the worker persists (a `ChannelAgentJob`) — enough for `resume_run` to re-arm it.
fn enqueue_payload(run_job: &str) -> String {
    serde_json::json!({
        "cid": "dock-user-ada-x",
        "goal": "goal",
        "run_job": run_job,
        "poster_sub": "user:ada",
        "poster_caps": ["mcp:agent.invoke:call"],
        "ts": 1
    })
    .to_string()
}

fn suspended_run(id: &str) -> Job {
    let mut j = Job::new(id, "agent-session", "summarize", 1);
    j.status = JobStatus::Suspended;
    j
}

fn cancelled_run(id: &str) -> Job {
    let mut j = Job::new(id, "agent-session", "go", 1);
    j.status = JobStatus::Cancelled;
    j
}
