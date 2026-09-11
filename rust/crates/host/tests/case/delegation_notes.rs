//! Comments keep the insight thread while the note lands on the case, and a producer grant still buys no triage write.
//!
//! Part of the `delegation` suite (see `delegation_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::delegation_support::*;

/// `insight.comment` returns the insight thread's `seq` as it always did, composes into
/// `insight.get` as it always did, AND lands the note in the case's history.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn insight_comment_keeps_the_thread_and_lands_the_note_on_the_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "deleg-c", 1).await;
    let case_id = case_of(&node, &p, "nube", &id).await;

    let first = call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "attended site", "ts": 2 }),
    )
    .await
    .expect("comment ok");
    assert_eq!(first, json!({ "seq": 1 }), "the shipped `{{seq}}` shape");
    let second = call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "waiting on the PO", "ts": 3 }),
    )
    .await
    .expect("comment ok");
    assert_eq!(second, json!({ "seq": 2 }), "and it still increments");

    // `insight.get` still composes the thread — the drawer's one round-trip.
    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    let thread = insight["comments"].as_array().unwrap();
    assert_eq!(thread.len(), 2, "{insight}");
    assert_eq!(thread[0]["text"], "waiting on the PO", "newest first");
    assert_eq!(thread[0]["author"], "user:test");

    // And the case history has both, as `comment` events with the same author.
    let events = lb_cases::events(&node.store, "nube", &case_id, 50, None)
        .await
        .expect("events ok");
    let comments: Vec<_> = events
        .items
        .iter()
        .filter(|e| e.kind == lb_cases::EventKind::Comment)
        .collect();
    assert_eq!(comments.len(), 2, "{events:?}");
    assert!(comments.iter().all(|c| c.actor == "user:test"));
}

/// A REFUSED note leaves both planes untouched. The insight thread's bounds are the stricter ones
/// (an empty note, a 4 KB cap, a 200-note count cap that refuses rather than evicting), so the
/// thread runs first — the reverse order would land an oversize note on the case and THEN fail the
/// call, which is the one outcome a refusal must not produce.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_refused_note_lands_on_neither_plane() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "deleg-d", 1).await;
    let case_id = case_of(&node, &p, "nube", &id).await;

    for bad in ["", &"x".repeat(5000)] {
        let err = call(
            &node,
            &p,
            "nube",
            "insight.comment",
            json!({ "id": id, "text": bad, "ts": 2 }),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
    }

    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(
        insight["comments"].as_array().unwrap().is_empty(),
        "the insight thread is untouched: {insight}"
    );
    let events = lb_cases::events(&node.store, "nube", &case_id, 50, None)
        .await
        .expect("events ok");
    assert!(
        !events
            .items
            .iter()
            .any(|e| e.kind == lb_cases::EventKind::Comment),
        "no note reached the case history either: {events:?}"
    );
}

/// **A producer grant still buys zero triage write power.** The deny this scope exists to create,
/// re-asserted after the delegation: if `insight.assign` had quietly started riding a case cap — or
/// dropped its own — this is what would catch it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_producer_grant_still_buys_no_triage_write_power() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &full, "nube", "deleg-e", 1).await;

    let producer = principal("key:nightly-rule", "nube", &[RAISE]);
    for (tool, input) in [
        (
            "insight.assign",
            json!({ "id": id, "assignee": "user:priya", "ts": 2 }),
        ),
        (
            "insight.comment",
            json!({ "id": id, "text": "hi", "ts": 2 }),
        ),
    ] {
        let err = call(&node, &producer, "nube", tool, input)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // Conversely, the triage caps alone are still enough — the delegation did not add a second wall.
    let triager = principal("user:test", "nube", &[I_GET, I_ASSIGN, I_COMMENT]);
    call(
        &node,
        &triager,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 3 }),
    )
    .await
    .expect("insight.assign must NOT require a case capability");
    call(
        &node,
        &triager,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "still works", "ts": 4 }),
    )
    .await
    .expect("insight.comment must NOT require a case capability");
}
