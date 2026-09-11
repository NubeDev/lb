//! Reconcile / backfill — and the resolved insight it deliberately leaves ungrouped.
//!
//! Part of the `reactor` suite (see `reactor_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::reactor_support::*;

// --- Reconcile / backfill -----------------------------------------------------------------------

/// The reconcile pass is the backstop AND the migration: every open, ungrouped insight gets a case,
/// its assignee is carried across, and its comment thread is replayed into the case history with the
/// ORIGINAL author and timestamp. A second pass returns `0`.
///
/// The fixture is built the way an upgrade really produces one — the insight rows are written
/// directly through the real `lb_insights` writers, exactly as a node running before the case plane
/// existed would have left them, so there is no case to find.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reconcile_backfills_a_case_its_assignee_and_its_comment_thread() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    // A pre-case-plane insight: raised through the crate, so no grouping ever ran.
    let outcome = lb_insights::raise(
        &node.store,
        "nube",
        legacy_raise(
            "legacy-1",
            lb_insights::Severity::Warning,
            "a finding from before the case plane",
        ),
        100,
    )
    .await
    .expect("legacy raise ok");
    let id = outcome.id.clone();
    lb_insights::assign(&node.store, "nube", &id, Some("user:priya"))
        .await
        .expect("legacy assign ok");
    lb_insights::append_comment(
        &node.store,
        "nube",
        &id,
        "attended site",
        "user:priya",
        1_100,
    )
    .await
    .expect("legacy comment ok");
    lb_insights::append_comment(
        &node.store,
        "nube",
        &id,
        "waiting on the PO",
        "user:test",
        1_200,
    )
    .await
    .expect("legacy comment ok");

    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        0,
        "the legacy finding has no case yet — the state the backfill exists for"
    );

    let grouped = lb_host::reconcile_cases(&node, "nube", 2_000)
        .await
        .expect("reconcile ok");
    assert_eq!(grouped, 1);
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);

    let case_id = case_of(&node, &p, "nube", &id).await;
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "single");
    assert_eq!(
        case["assigned_to"], "user:priya",
        "the insight's owner must be carried onto the case: {case}"
    );

    // The thread came across — oldest first, original author, original timestamp.
    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let comments: Vec<&Value> = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "comment")
        .collect();
    assert_eq!(comments.len(), 2, "both notes copied: {events}");
    let attended = comments
        .iter()
        .find(|e| e["data"]["text"] == "attended site")
        .unwrap();
    assert_eq!(
        attended["actor"], "user:priya",
        "a migration must not restamp the author: {attended}"
    );
    assert_eq!(
        attended["ts"], 1_100,
        "nor the timestamp — that would destroy the record it was preserving"
    );

    // IDEMPOTENT: a second pass groups nothing and does not duplicate the thread.
    let again = lb_host::reconcile_cases(&node, "nube", 3_000)
        .await
        .expect("reconcile ok");
    assert_eq!(again, 0, "a second pass must be a no-op");
    let events2 = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let n2 = events2["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "comment")
        .count();
    assert_eq!(n2, 2, "the thread must not be copied twice");
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);
}

/// A RESOLVED insight is not open work, so the reconcile pass leaves it alone — no case is minted
/// for a finding nobody is working.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reconcile_leaves_a_resolved_insight_ungrouped() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let outcome = lb_insights::raise(
        &node.store,
        "nube",
        legacy_raise(
            "legacy-resolved",
            lb_insights::Severity::Info,
            "already done",
        ),
        100,
    )
    .await
    .expect("raise ok");
    lb_insights::resolve(&node.store, "nube", &outcome.id, "user:test", None, 1_500)
        .await
        .expect("resolve ok");

    let grouped = lb_host::reconcile_cases(&node, "nube", 2_000)
        .await
        .expect("reconcile ok");
    assert_eq!(grouped, 0);
    assert_eq!(open_case_count(&node, &p, "nube").await, 0);
}
