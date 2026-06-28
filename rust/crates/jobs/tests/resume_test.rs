//! The store-layer guarantees the agent's resumable session leans on (jobs scope, agent scope
//! offline/sync): a session persists, resumes from its cursor, and re-applying a persisted event is
//! a no-op (idempotent resume). These are pure store verbs — no node/bus, so a plain `tokio::test`
//! (no Zenoh peer) is enough. agent-run Part 0 made the transcript *typed* — the events here are
//! [`TranscriptEvent`]s, not opaque strings.

use lb_jobs::{
    append_event, cancel, complete, create, load, suspend, unsuspend, Job, JobStatus,
    TranscriptEvent,
};
use lb_store::Store;

fn turn(content: &str) -> TranscriptEvent {
    TranscriptEvent::AssistantTurn {
        content: content.into(),
    }
}

#[tokio::test]
async fn a_session_persists_and_resumes_from_its_cursor() {
    let store = Store::memory().await.unwrap();
    let ws = "jobs-resume";

    create(
        &store,
        ws,
        &Job::new("s1", "agent-session", "goal:summarize", 1),
    )
    .await
    .unwrap();

    // Record two events.
    append_event(&store, ws, "s1", 0, turn("loaded doc"))
        .await
        .unwrap();
    append_event(&store, ws, "s1", 1, turn("called summarize tool"))
        .await
        .unwrap();

    // "Resume": re-read the durable record — the cursor points past the landed events.
    let job = load(&store, ws, "s1").await.unwrap().expect("job persists");
    assert_eq!(job.cursor, 2, "cursor advanced past both events");
    assert_eq!(job.steps.len(), 2);
    assert_eq!(job.steps[1].event, turn("called summarize tool"));
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(job.schema_version, lb_jobs::TRANSCRIPT_SCHEMA_VERSION);
}

#[tokio::test]
async fn re_applying_a_persisted_event_is_a_no_op() {
    // The offline/sync property: the edge disconnected after event 1 landed but before the loop
    // advanced; on resume the agent re-records event 1. The slot is upserted, not duplicated, and
    // the cursor does not rewind (jobs scope idempotent resume).
    let store = Store::memory().await.unwrap();
    let ws = "jobs-idempotent";

    create(&store, ws, &Job::new("s1", "agent-session", "g", 1))
        .await
        .unwrap();
    append_event(&store, ws, "s1", 0, turn("step zero"))
        .await
        .unwrap();
    append_event(&store, ws, "s1", 1, turn("step one"))
        .await
        .unwrap();

    // Re-apply event 0 (a resume replay). Same slot, same id — no new row, no rewind.
    append_event(&store, ws, "s1", 0, turn("step zero"))
        .await
        .unwrap();

    let job = load(&store, ws, "s1").await.unwrap().unwrap();
    assert_eq!(job.steps.len(), 2, "replay did NOT duplicate the event");
    assert_eq!(
        job.cursor, 2,
        "replaying an old event did NOT rewind the cursor"
    );
}

#[tokio::test]
async fn complete_sets_the_terminal_status() {
    let store = Store::memory().await.unwrap();
    let ws = "jobs-complete";
    create(&store, ws, &Job::new("s1", "agent-session", "g", 1))
        .await
        .unwrap();

    complete(&store, ws, "s1", JobStatus::Done).await.unwrap();
    let job = load(&store, ws, "s1").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Done);
}

#[tokio::test]
async fn cancel_leaves_a_terminal_non_resumable_state() {
    // agent-run Part 0 cancel hook: a UI stop / ACP session/cancel leaves a terminal, restorable
    // (audit-readable) state that the loop does NOT re-enter.
    let store = Store::memory().await.unwrap();
    let ws = "jobs-cancel";
    create(&store, ws, &Job::new("s1", "agent-session", "g", 1))
        .await
        .unwrap();
    append_event(&store, ws, "s1", 0, turn("did one thing"))
        .await
        .unwrap();

    cancel(&store, ws, "s1").await.unwrap();
    let job = load(&store, ws, "s1").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Cancelled);
    assert!(!job.status.is_resumable(), "cancelled is not resumable");
    assert_eq!(job.steps.len(), 1, "transcript kept for audit/replay");

    // Idempotent; and a finished run cannot be retroactively cancelled.
    cancel(&store, ws, "s1").await.unwrap();
    create(&store, ws, &Job::new("s2", "agent-session", "g", 1))
        .await
        .unwrap();
    complete(&store, ws, "s2", JobStatus::Done).await.unwrap();
    assert!(
        cancel(&store, ws, "s2").await.is_err(),
        "a finished run cannot be cancelled"
    );
}

#[tokio::test]
async fn suspend_then_unsuspend_round_trips() {
    // agent-run Part 2: suspended is terminal-for-the-turn but resumable; the reactor unsuspends.
    let store = Store::memory().await.unwrap();
    let ws = "jobs-suspend";
    create(&store, ws, &Job::new("s1", "agent-session", "g", 1))
        .await
        .unwrap();

    suspend(&store, ws, "s1").await.unwrap();
    let job = load(&store, ws, "s1").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Suspended);
    assert!(job.status.is_resumable(), "suspended IS resumable");
    suspend(&store, ws, "s1").await.unwrap(); // idempotent

    unsuspend(&store, ws, "s1").await.unwrap();
    let job = load(&store, ws, "s1").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Running);
}

#[tokio::test]
async fn a_job_is_invisible_across_the_workspace_wall() {
    // MANDATORY workspace-isolation at the store layer (testing §2.2): a ws-B load can never read a
    // ws-A session, even with the same job id — the namespace is the hard wall (README §7).
    let store = Store::memory().await.unwrap();
    create(
        &store,
        "jobs-iso-a",
        &Job::new("s1", "agent-session", "ws-a secret", 1),
    )
    .await
    .unwrap();

    let from_b = load(&store, "jobs-iso-b", "s1").await.unwrap();
    assert!(from_b.is_none(), "ws-B must not see ws-A's job");
}
