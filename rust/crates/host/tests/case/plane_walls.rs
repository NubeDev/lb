//! The three mandatory walls: a capability deny per verb, the POSITIVE gate test for the aliased verbs, and workspace isolation.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::plane_support::*;

// --- MANDATORY: capability deny -----------------------------------------------------------------

/// Every case verb refuses a principal that does not hold its capability. One test over the whole
/// surface, because the interesting property is that there is NO hole: a verb added later without a
/// cap check fails here rather than shipping open.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_case_verb_denies_a_principal_without_its_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &full, "nube", "deny-probe", 1).await;
    let case_id = case_of(&node, &full, "nube", &insight).await;

    // Holds the insight caps and nothing on the case plane — the producer's exact shape.
    let bare = principal("key:nightly-rule", "nube", &[RAISE, I_GET]);

    for (tool, input) in [
        ("case.get", json!({ "id": case_id })),
        ("case.list", json!({ "lane": "mine" })),
        ("case.members", json!({ "case_id": case_id })),
        ("case.events", json!({ "case_id": case_id })),
        (
            "case.open",
            json!({ "title": "t", "primary_insight": insight }),
        ),
        ("case.merge", json!({ "from": case_id, "into": case_id })),
        (
            "case.split",
            json!({ "from": case_id, "insight_ids": [insight], "title": "t" }),
        ),
        (
            "case.workflow",
            json!({ "id": case_id, "workflow": "actioned" }),
        ),
        (
            "case.assign",
            json!({ "id": case_id, "assignee": "user:priya" }),
        ),
        (
            "case.snooze",
            json!({ "id": case_id, "until": 9_999_999, "reason": "r" }),
        ),
        ("case.comment", json!({ "id": case_id, "text": "hi" })),
    ] {
        let err = call(&node, &bare, "nube", tool, input).await.unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{tool} must be Denied without its capability, got {err:?}"
        );
    }
}

// --- MANDATORY: the POSITIVE gate test (the aliased verbs) --------------------------------------

/// The test a missing `tool_gate.rs` alias fails and nothing else does.
///
/// `case.members`/`case.events` gate on `case.get`; `case.merge`/`case.split` on `case.open`;
/// `case.assign`/`case.snooze`/`case.comment` on `case.workflow`. If any alias is missing, the outer
/// gate demands `mcp:case.<verb>:call` — a capability in NO role bundle — and the verb answers a
/// bare `Denied` for every caller including admins. That refusal is indistinguishable from a real
/// authorization failure, so only a call that MUST succeed can catch it
/// (`new-lb-verb-needs-a-gate-alias.md`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_aliased_case_verb_resolves_to_a_cap_a_role_actually_holds() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &full, "nube", "alias-a", 1).await;
    let b = seed_insight(&node, &full, "nube", "alias-b", 2).await;
    let case_a = case_of(&node, &full, "nube", &a).await;
    let case_b = case_of(&node, &full, "nube", &b).await;

    // A token holding ONLY the read cap reaches both aliased reads.
    let reader = principal("user:test", "nube", &[GET]);
    call(
        &node,
        &reader,
        "nube",
        "case.members",
        json!({ "case_id": case_a }),
    )
    .await
    .expect("case.members must resolve through the `case.get` alias");
    call(
        &node,
        &reader,
        "nube",
        "case.events",
        json!({ "case_id": case_a }),
    )
    .await
    .expect("case.events must resolve through the `case.get` alias");

    // A token holding ONLY the triage cap reaches all three triage writes.
    let triager = principal("user:test", "nube", &[WORKFLOW]);
    call(
        &node,
        &triager,
        "nube",
        "case.comment",
        json!({ "id": case_a, "text": "note" }),
    )
    .await
    .expect("case.comment must resolve through the `case.workflow` alias");
    call(
        &node,
        &triager,
        "nube",
        "case.snooze",
        json!({ "id": case_a, "until": 9_999_999_999_999u64, "reason": "next quarter" }),
    )
    .await
    .expect("case.snooze must resolve through the `case.workflow` alias");
    call(
        &node,
        &triager,
        "nube",
        "case.assign",
        json!({ "id": case_a, "assignee": "user:priya" }),
    )
    .await
    .expect("case.assign must resolve through the `case.workflow` alias");

    // A token holding ONLY the grouping cap reaches merge (and, through it, split).
    let grouper = principal("user:test", "nube", &[OPEN]);
    call(
        &node,
        &grouper,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a }),
    )
    .await
    .expect("case.merge must resolve through the `case.open` alias");
    call(
        &node,
        &grouper,
        "nube",
        "case.split",
        json!({ "from": case_a, "insight_ids": [b], "title": "split out" }),
    )
    .await
    .expect("case.split must resolve through the `case.open` alias");
}

/// The caps the aliases point at must EXIST in a shipped role bundle — the other half of the trap.
/// An alias onto a cap nobody carries is just as unreachable as no alias at all.
#[test]
fn the_case_caps_ship_in_a_role_bundle() {
    let viewer = lb_host::viewer_role_caps();
    let member = lb_host::member_role_caps();
    for cap in [GET, LIST] {
        assert!(
            viewer.iter().any(|c| c == cap),
            "{cap} must be a VIEWER cap"
        );
    }
    for cap in [OPEN, WORKFLOW] {
        assert!(
            member.iter().any(|c| c == cap),
            "{cap} must be a MEMBER cap"
        );
        assert!(
            !viewer.iter().any(|c| c == cap),
            "{cap} must NOT be a viewer cap — a bare viewer reads the queue, a member moves it"
        );
    }
}

// --- MANDATORY: workspace isolation -------------------------------------------------------------

/// A ws-B principal cannot see or touch a ws-A case — and the refusal for a REAL ws-A case id is
/// identical to the refusal for a fictional one, so a probe learns nothing about what exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_cannot_see_or_touch_a_ws_a_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "acme").await;
    let a_user = principal("user:test", "acme", ALL);
    let insight = seed_insight(&node, &a_user, "acme", "iso-probe", 1).await;
    let case_id = case_of(&node, &a_user, "acme", &insight).await;

    // Fully-capped, but in another workspace.
    let b_user = principal("user:test", "other", ALL);

    // The outer gate refuses the cross-workspace call before anything is read.
    let err = call(&node, &b_user, "acme", "case.get", json!({ "id": case_id }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "cross-ws get: {err:?}");

    // And inside its OWN workspace, a ws-A case id simply is not there — the same answer a
    // fictional id gives, because the store read is namespace-scoped.
    let real = call(
        &node,
        &b_user,
        "other",
        "case.get",
        json!({ "id": case_id }),
    )
    .await
    .expect("in-ws get is allowed");
    let fake = call(
        &node,
        &b_user,
        "other",
        "case.get",
        json!({ "id": "01ABSENT" }),
    )
    .await
    .expect("in-ws get is allowed");
    assert_eq!(real, Value::Null);
    assert_eq!(
        real, fake,
        "a real ws-A id and a fictional id must be indistinguishable from ws-B"
    );

    // A write is refused the same way, and nothing lands.
    let err = call(
        &node,
        &b_user,
        "other",
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "a ws-A case is simply absent in ws-B: {err:?}"
    );
    let still = call(&node, &a_user, "acme", "case.get", json!({ "id": case_id }))
        .await
        .expect("ws-A still reads its own case");
    assert_eq!(still["workflow"], "to_action", "ws-A's case was untouched");
}
