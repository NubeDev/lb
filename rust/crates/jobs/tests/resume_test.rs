//! The store-layer guarantees the agent's resumable session leans on (jobs scope, agent scope
//! offline/sync): a session persists, resumes from its cursor, and re-applying a persisted step is
//! a no-op (idempotent resume). These are pure store verbs — no node/bus, so a plain `tokio::test`
//! (no Zenoh peer) is enough.

use lb_jobs::{append_step, complete, create, load, Job, JobStatus};
use lb_store::Store;

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

    // Run two steps.
    append_step(&store, ws, "s1", 0, "loaded doc")
        .await
        .unwrap();
    append_step(&store, ws, "s1", 1, "called summarize tool")
        .await
        .unwrap();

    // "Resume": re-read the durable record — the cursor points past the landed steps.
    let job = load(&store, ws, "s1").await.unwrap().expect("job persists");
    assert_eq!(job.cursor, 2, "cursor advanced past both steps");
    assert_eq!(job.steps.len(), 2);
    assert_eq!(job.steps[1].result, "called summarize tool");
    assert_eq!(job.status, JobStatus::Running);
}

#[tokio::test]
async fn re_applying_a_persisted_step_is_a_no_op() {
    // The offline/sync property: the edge disconnected after step 1 landed but before the loop
    // advanced; on resume the agent re-runs step 1. The slot is upserted, not duplicated, and the
    // cursor does not rewind (jobs scope idempotent resume).
    let store = Store::memory().await.unwrap();
    let ws = "jobs-idempotent";

    create(&store, ws, &Job::new("s1", "agent-session", "g", 1))
        .await
        .unwrap();
    append_step(&store, ws, "s1", 0, "step zero").await.unwrap();
    append_step(&store, ws, "s1", 1, "step one").await.unwrap();

    // Re-apply step 0 (a resume replay). Same slot, same id — no new row, no rewind.
    append_step(&store, ws, "s1", 0, "step zero").await.unwrap();

    let job = load(&store, ws, "s1").await.unwrap().unwrap();
    assert_eq!(job.steps.len(), 2, "replay did NOT duplicate the step");
    assert_eq!(
        job.cursor, 2,
        "replaying an old step did NOT rewind the cursor"
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
