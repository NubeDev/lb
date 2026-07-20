//! Slice C of agent-loop-hardening: the **dangling-tool-call invariant** at the loop level, on the
//! real store/bus (offline/sync mandatory category §2.3 — "kill a run with pending calls; resume;
//! the replayed transcript is valid").
//!
//! A run that died after proposing calls (crash, kill) is healed on resume: each orphan gets a
//! `ToolCancelled` APPENDED at the cursor — existing step indices are NEVER renumbered (resume
//! idempotency is a step-index lookup) — the watcher projection resolves the call's spinner, and
//! the resumed model turn sees "cancelled", not a silent gap. A suspension-parked call is NOT an
//! orphan and is never cancelled by the heal.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{resume, Node};
use lb_jobs::{append_event, create, load, orphaned_calls, Job, JobStatus, TranscriptEvent};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};
use lb_run_events::{project, RunEvent};

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

/// Seed a job that "died mid-turn": an assistant turn + two proposed calls, no results — written
/// through the REAL append path (`lb_jobs::append_event`), exactly the record a killed process
/// leaves behind.
async fn seed_killed_run(node: &Arc<Node>, ws: &str, job_id: &str) {
    let job = Job::new(job_id, "agent-session", "finish the task", 1);
    create(&node.store, ws, &job).await.unwrap();
    append_event(
        &node.store,
        ws,
        job_id,
        0,
        TranscriptEvent::AssistantTurn {
            content: "let me check two things".into(),
        },
    )
    .await
    .unwrap();
    for (i, id) in ["c1", "c2"].iter().enumerate() {
        append_event(
            &node.store,
            ws,
            job_id,
            (i + 1) as u32,
            TranscriptEvent::ToolCallProposed {
                id: (*id).into(),
                name: "no.such_tool".into(),
                args: "{}".into(),
            },
        )
        .await
        .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_killed_run_with_pending_calls_heals_on_resume_without_renumbering() {
    let ws = "dangling-heal";
    let job_id = "job-killed";
    let node = Arc::new(Node::boot().await.unwrap());
    seed_killed_run(&node, ws, job_id).await;

    // Resume through the real entry; the model just wraps up.
    let gw = AiGateway::new(MockProvider::new(vec![AiResponse::stop("wrapped up", 5)]));
    let caller = principal("user:ada", ws, &[INVOKE]);
    let answer = resume(
        &node,
        &gw,
        &caller,
        &[INVOKE.to_string()],
        ws,
        job_id,
        &[],
        2,
    )
    .await
    .expect("resume completes");
    assert_eq!(answer, "wrapped up");

    let job = load(&node.store, ws, job_id).await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Done);

    // The invariant: no proposal without a resolution — both orphans were cancelled.
    let events: Vec<&TranscriptEvent> = job.events().collect();
    assert!(
        orphaned_calls(&events).is_empty(),
        "no orphan survives a resume"
    );
    let cancelled: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            TranscriptEvent::ToolCancelled { id } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(cancelled, vec!["c1", "c2"], "one ToolCancelled per orphan");

    // NEVER renumbered: the pre-fix events still sit at their original indices, and the heals were
    // APPENDED after them (step addressing is what makes resume idempotent).
    assert!(matches!(
        &job.steps[0].event,
        TranscriptEvent::AssistantTurn { .. }
    ));
    assert!(
        matches!(&job.steps[1].event, TranscriptEvent::ToolCallProposed { id, .. } if id == "c1")
    );
    assert!(
        matches!(&job.steps[2].event, TranscriptEvent::ToolCallProposed { id, .. } if id == "c2")
    );
    assert!(matches!(
        &job.steps[3].event,
        TranscriptEvent::ToolCancelled { id } if id == "c1"
    ));
    for (i, step) in job.steps.iter().enumerate() {
        assert_eq!(step.index, i as u32, "dense, un-renumbered step addressing");
    }

    // The watcher projection resolves the spinners: a `ToolCancelled` run event per call, so a
    // reattaching dock never hangs in "tool running…".
    let projected = project(&job);
    let cancelled_events = projected
        .iter()
        .filter(|e| matches!(e, RunEvent::ToolCancelled { .. }))
        .count();
    assert_eq!(cancelled_events, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_suspension_parked_call_is_not_cancelled_by_the_heal() {
    let ws = "dangling-parked";
    let job_id = "job-parked";
    let node = Arc::new(Node::boot().await.unwrap());

    // A run paused on an Ask: proposed + SuspensionOpened, no decision settled yet.
    let job = Job::new(job_id, "agent-session", "gated task", 1);
    create(&node.store, ws, &job).await.unwrap();
    append_event(
        &node.store,
        ws,
        job_id,
        0,
        TranscriptEvent::ToolCallProposed {
            id: "gated".into(),
            name: "dangerous.tool".into(),
            args: "{}".into(),
        },
    )
    .await
    .unwrap();
    append_event(
        &node.store,
        ws,
        job_id,
        1,
        TranscriptEvent::SuspensionOpened {
            tool_call_id: "gated".into(),
            decision_id: format!("{job_id}:gated"),
        },
    )
    .await
    .unwrap();

    // Resume: the decision is still pending, so the run stays parked — and the heal must NOT have
    // cancelled the gated call (it is awaiting a human, not dangling).
    let gw = AiGateway::new(MockProvider::new(vec![AiResponse::stop("never asked", 5)]));
    let caller = principal("user:ada", ws, &[INVOKE]);
    let _ = resume(
        &node,
        &gw,
        &caller,
        &[INVOKE.to_string()],
        ws,
        job_id,
        &[],
        2,
    )
    .await
    .expect("resume settles as still-pending");

    let job = load(&node.store, ws, job_id).await.unwrap().unwrap();
    assert!(
        !job.steps
            .iter()
            .any(|s| matches!(&s.event, TranscriptEvent::ToolCancelled { .. })),
        "a suspension-parked call must never be healed away"
    );
}
