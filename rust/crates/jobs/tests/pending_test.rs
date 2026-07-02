//! The `pending` drain-scan verb (run-lifecycle #5): a background reactor lists the still-running
//! jobs of a `kind` to pick them up. These are pure store verbs (no node/bus), so a plain
//! `tokio::test` over an in-memory `Store` is enough (rule 9 — the real store, no mock).
//!
//! Invariants:
//!   - lists only jobs of the requested `kind` (a different kind is not returned);
//!   - lists only *resumable* jobs (a `Done`/`Failed`/`Cancelled` job is drained, never re-driven);
//!   - is workspace-scoped — a ws-B scan never sees a ws-A job (the hard wall, README §7).

use lb_jobs::{complete, create, pending, Job, JobStatus};
use lb_store::Store;

const KIND: &str = "channel-agent-run";

#[tokio::test]
async fn lists_only_running_jobs_of_the_requested_kind() {
    let store = Store::memory().await.unwrap();
    let ws = "drain";

    create(&store, ws, &Job::new("q:run-a", KIND, "{}", 1))
        .await
        .unwrap();
    create(&store, ws, &Job::new("q:run-b", KIND, "{}", 2))
        .await
        .unwrap();
    // A different kind in the same table — must NOT be returned by a KIND scan.
    create(&store, ws, &Job::new("s1", "agent-session", "goal", 3))
        .await
        .unwrap();
    // A terminal job of the right kind — drained, never re-driven.
    create(&store, ws, &Job::new("q:run-done", KIND, "{}", 4))
        .await
        .unwrap();
    complete(&store, ws, "q:run-done", JobStatus::Done)
        .await
        .unwrap();

    let mut ids: Vec<String> = pending(&store, ws, KIND)
        .await
        .unwrap()
        .into_iter()
        .map(|j| j.id)
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        vec!["q:run-a".to_string(), "q:run-b".to_string()],
        "only running jobs of the requested kind are pending"
    );
}

#[tokio::test]
async fn a_pending_scan_is_workspace_scoped() {
    let store = Store::memory().await.unwrap();

    create(&store, "ws-a", &Job::new("q:run-a", KIND, "{}", 1))
        .await
        .unwrap();
    create(&store, "ws-b", &Job::new("q:run-b", KIND, "{}", 2))
        .await
        .unwrap();

    let a = pending(&store, "ws-a", KIND).await.unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].id, "q:run-a");

    // ws-b's scan never sees ws-a's job — the store namespace is the hard wall.
    let b = pending(&store, "ws-b", KIND).await.unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].id, "q:run-b");
}
