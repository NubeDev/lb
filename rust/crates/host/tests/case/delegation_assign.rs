//! Assignment delegates to the case — and assigning one detection assigns every detection its case cites.
//!
//! Part of the `delegation` suite (see `delegation_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::delegation_support::*;

// --- The delegation, both directions ------------------------------------------------------------

/// `insight.assign` writes the CASE and echoes onto the insight. The return shape
/// (`{ assigned_to }`) and the cap are unchanged, and `insight.get`/`insight.list` still answer from
/// the insight — which is what keeps every shipped reader working.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn insight_assign_writes_the_case_and_echoes_back_onto_the_insight() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "deleg-a", 1).await;
    let case_id = case_of(&node, &p, "nube", &id).await;

    // The SHIPPED shape: `{ assigned_to }` for a single-id call.
    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "team:mechanical", "ts": 2 }),
    )
    .await
    .expect("assign ok");
    assert_eq!(
        out,
        json!({ "assigned_to": "team:mechanical" }),
        "the return shape must not change"
    );

    // The CASE is the authority.
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["assigned_to"], "team:mechanical");
    // …and it is in the case's history, attributed to the caller.
    let events = lb_cases::events(&node.store, "nube", &case_id, 50, None)
        .await
        .expect("events ok");
    let assigned = events
        .items
        .iter()
        .find(|e| e.kind == lb_cases::EventKind::Assigned)
        .expect("an `assigned` event");
    assert_eq!(assigned.actor, "user:test");

    // The INSIGHT carries the echo, so every shipped reader still answers.
    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(insight["assigned_to"], "team:mechanical");
    let page = call(
        &node,
        &p,
        "nube",
        "insight.list",
        json!({ "filter": { "assigned_to": "team:mechanical" } }),
    )
    .await
    .expect("list ok");
    assert_eq!(page["items"].as_array().unwrap().len(), 1, "{page}");

    // Un-assign clears both planes.
    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": id, "ts": 3 }),
    )
    .await
    .expect("un-assign ok");
    assert_eq!(out, json!({ "assigned_to": Value::Null }));
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert!(case["assigned_to"].is_null(), "{case}");
    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(insight["assigned_to"].is_null(), "{insight}");
}

/// **Assigning one detection assigns the whole job.** The behaviour change the plane exists to
/// produce, asserted rather than discovered: three symptoms of one fault, merged into one case, take
/// one owner together.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assigning_one_detection_assigns_every_detection_the_case_cites() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal(
        "user:test",
        "nube",
        &[
            RAISE,
            I_GET,
            I_LIST,
            I_ASSIGN,
            GET,
            LIST,
            "mcp:case.open:call",
        ],
    );
    let a = seed_insight(&node, &p, "nube", "fan-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "fan-b", 2).await;
    let case_a = case_of(&node, &p, "nube", &a).await;
    let case_b = case_of(&node, &p, "nube", &b).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a, "ts": 3 }),
    )
    .await
    .expect("merge ok");

    call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": a, "assignee": "user:priya", "ts": 4 }),
    )
    .await
    .expect("assign ok");

    for id in [&a, &b] {
        let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
            .await
            .expect("get ok");
        assert_eq!(
            insight["assigned_to"], "user:priya",
            "every detection the case cites takes the owner: {insight}"
        );
    }
}
