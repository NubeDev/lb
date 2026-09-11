//! Workspace isolation of the stamp, and (g) the case echo that FOLLOWS the primary insight rather than snapshotting it.
//!
//! Part of the `caveat` suite (see `caveat_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::caveat_support::*;

// --- MANDATORY: workspace isolation --------------------------------------------------------------

/// A gating finding in `ws-a` must not caveat a finding in `ws-b`, even on the identical subject.
/// The vocabulary AND the scan are both ws-scoped; a cross-ws caveat would leak the existence of
/// another workspace's finding through a field on this one's record.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_gating_finding_does_not_caveat_across_workspaces() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_vocab(&node, "ws-a").await;
    seed_vocab(&node, "ws-b").await;
    let a = principal("user:test", "ws-a", &caps());
    let b = principal("user:test", "ws-b", &caps());

    raise(
        &node,
        &a,
        "ws-a",
        raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]),
    )
    .await
    .expect("dq raise in ws-a");

    let out = raise(
        &node,
        &b,
        "ws-b",
        raise_input("dependent", "critical", 2, Some(VALUES[2]), &["point:X"]),
    )
    .await
    .expect("raise in ws-b");
    assert_eq!(out["caveated"], false, "no cross-workspace caveat: {out}");
}

/// The open case whose primary is `insight_id` — found structurally through `case.list`, the way
/// the `reactor_*` files do, rather than assuming `insight.raise` echoes a case id back.
async fn case_for(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    let listed = call(node, p, ws, "case.list", json!({ "lane": "watching" }))
        .await
        .expect("case.list");
    listed["items"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|c| c["primary_insight"].as_str() == Some(insight_id))
        })
        .and_then(|c| c["id"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("no open case for {insight_id}: {listed}"))
}

// --- (g) the case echo follows the insight, it is not a snapshot --------------------------------

/// **The soft-block an operator sees must describe the world now, not the world when the case
/// opened.** `Insight.caveats` is recomputed on every raise on purpose; `Case.caveated` echoes it,
/// and an echo written once at open drifts in whichever direction hurts.
///
/// Both directions, in one case's life: a case opens CLEAN, the sensor it rests on breaks, and the
/// badge must appear; the sensor is fixed, and the badge must go. The second half is the one a
/// snapshot cannot do at all, and it is why this is not merged into the caveat state.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_case_caveat_echo_follows_the_primary_insight_both_ways() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let mut c = caps();
    c.extend([
        "mcp:case.get:call",
        "mcp:case.list:call",
        "mcp:case.open:call",
        "mcp:case.workflow:call",
    ]);
    let p = principal("user:test", ws, &c);

    // The work: a finding resting on one point. No gating finding exists yet, so it opens CLEAN.
    let work = raise(
        &node,
        &p,
        ws,
        raise_input("work-1", "critical", 1_000, Some("device"), &["point:p1"]),
    )
    .await
    .expect("raise work");
    let insight_id = work["id"].as_str().expect("id").to_string();
    let case_id = case_for(&node, &p, ws, &insight_id).await;
    let case = call(&node, &p, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get");
    assert_eq!(case["caveated"], false, "nothing gates it yet: {case}");

    // The sensor breaks — a GATING finding on the same point.
    let gate = raise(
        &node,
        &p,
        ws,
        raise_input("sensor-1", "warning", 2_000, Some(GATE), &["point:p1"]),
    )
    .await
    .expect("raise gate");
    let gate_id = gate["id"].as_str().expect("id").to_string();

    // The work fires again. THIS is the refresh: without it the case still reads clean and the
    // contractor button stays live over a number nobody should trust.
    raise(
        &node,
        &p,
        ws,
        raise_input("work-1", "critical", 3_000, Some("device"), &["point:p1"]),
    )
    .await
    .expect("re-raise work");
    let case = call(&node, &p, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get");
    assert_eq!(
        case["caveated"], true,
        "the case must pick up the caveat its primary now carries: {case}"
    );

    // And the history says why the button changed, because an operator will ask.
    let events = call(&node, &p, ws, "case.events", json!({ "case_id": case_id }))
        .await
        .unwrap_or_else(|_| json!({ "items": [] }));
    let kinds: Vec<String> = events["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| e["kind"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        kinds.iter().any(|k| k == "caveat"),
        "a flag that changes with no event is unanswerable: {kinds:?}"
    );

    // The sensor is FIXED. Resolving the gate and re-firing the work must clear the badge — the
    // half a snapshot can never do, and the one that stops operators learning to ignore caveats.
    call(
        &node,
        &p,
        ws,
        "insight.resolve",
        json!({ "id": gate_id, "ts": 4_000 }),
    )
    .await
    .expect("resolve gate");
    raise(
        &node,
        &p,
        ws,
        raise_input("work-1", "critical", 5_000, Some("device"), &["point:p1"]),
    )
    .await
    .expect("re-raise work again");
    let case = call(&node, &p, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get");
    assert_eq!(
        case["caveated"], false,
        "a fixed sensor must clear the badge — a permanent caveat is one nobody reads: {case}"
    );
}
