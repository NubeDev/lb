//! The three mandatory walls: a capability deny per verb, workspace isolation over party and request, and the POSITIVE gate test per verb.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::request_support::*;

// --- MANDATORY: capability deny -----------------------------------------------------------------

/// Every new verb refuses a principal that does not hold its capability. One test over the whole
/// wave-2 surface, so a verb added later without a gate fails here rather than shipping open.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_request_verb_denies_a_principal_without_its_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "deny-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;

    // A principal with every OTHER case cap and none of the new ones.
    let bare = principal("user:test", ws, &[RAISE, I_GET, GET, OPEN, WORKFLOW]);
    for (tool, input) in [
        (
            "party.upsert",
            json!({ "id": "x", "kind": "contractor", "name": "X" }),
        ),
        ("party.list", json!({})),
        (
            "case.request.send",
            json!({ "case_id": case_id, "party_id": "northern", "ask": "quote" }),
        ),
        ("case.request.withdraw", json!({ "id": "whatever" })),
        (
            "case.request.nudge",
            json!({ "id": "whatever", "stage": "n50" }),
        ),
    ] {
        let err = call(&node, &bare, ws, tool, input)
            .await
            .expect_err(&format!("{tool} must deny"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // The two TOKEN verbs are refused even for a fully-empowered member: their caps are in no role
    // bundle, deliberately, and nothing but a presented link mints them.
    for (tool, input) in [
        ("case.request.view", json!({ "id": "whatever" })),
        (
            "case.request.reply",
            json!({ "id": "whatever", "reply": { "kind": "accept" } }),
        ),
    ] {
        let err = call(&node, &full, ws, tool, input)
            .await
            .expect_err(&format!("{tool} must deny a logged-in caller"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }
}

// --- MANDATORY: workspace isolation -------------------------------------------------------------

/// A ws-B principal cannot see or touch a ws-A party or ask — and a REAL ws-A id is refused
/// identically to a fictional one, so a probe learns nothing about what exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_cannot_reach_a_ws_a_party_or_request() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "acme").await;
    let a = principal("user:test", "acme", ALL);
    let case_id = seed_case(&node, &a, "acme", "iso-probe").await;
    seed_party(&node, &a, "acme", "northern", 24).await;
    let (request_id, token) = send_ask(&node, &a, "acme", &case_id, "northern").await;

    // Fully capped — in another workspace.
    let b = principal("user:test", "other", ALL);
    for (tool, input) in [
        ("party.list", json!({})),
        (
            "case.request.send",
            json!({ "case_id": case_id, "party_id": "northern", "ask": "quote" }),
        ),
        ("case.request.withdraw", json!({ "id": request_id })),
    ] {
        let err = call(&node, &b, "acme", tool, input)
            .await
            .expect_err(&format!("{tool} must deny across the wall"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // The party roster does not leak either: ws `other` sees an empty roster, not acme's.
    let roster = call(&node, &b, "other", "party.list", json!({}))
        .await
        .expect("own-workspace list is allowed");
    assert_eq!(
        roster.as_array().map(Vec::len),
        Some(0),
        "a ws-B roster must never contain a ws-A party"
    );

    // And the TOKEN is workspace-bound: presenting acme's token against `other` resolves nothing.
    let wrong_ws = lb_host::case_request_authenticate(&node.store, "other", &token, T0).await;
    assert!(
        wrong_ws.is_err(),
        "a token must not resolve in another workspace"
    );
}

// --- MANDATORY: the positive gate test per verb --------------------------------------------------

/// Each aliased verb resolves to a cap a role actually holds, and each namesake cap exists in a
/// shipped bundle. A missing `tool_gate.rs` arm is `Denied`, not `NotFound`, so only this catches it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_new_verb_resolves_to_a_cap_a_role_actually_holds() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "gate-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;

    // A token holding ONLY `case.request.send` reaches send, withdraw AND nudge.
    let sender = principal("user:test", ws, &[SEND]);
    let request = call(
        &node,
        &sender,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "northern", "ask": "attend", "ts": T0 }),
    )
    .await
    .expect("case.request.send gates on its own cap");
    let id = request["id"].as_str().unwrap().to_string();
    call(
        &node,
        &sender,
        ws,
        "case.request.nudge",
        json!({ "id": id, "stage": "n50", "ts": T0 + HOUR_MS }),
    )
    .await
    .expect("case.request.nudge must resolve through the `case.request.send` alias");
    call(
        &node,
        &sender,
        ws,
        "case.request.withdraw",
        json!({ "id": id, "ts": T0 + 2 * HOUR_MS }),
    )
    .await
    .expect("case.request.withdraw must resolve through the `case.request.send` alias");

    // A token holding ONLY the case READ cap reaches the drawer's ask list.
    let reader = principal("user:test", ws, &[GET]);
    call(
        &node,
        &reader,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("case.request.list must resolve through the `case.get` alias");
}

/// The other half of the trap: the caps the aliases point at must EXIST in a shipped bundle — and
/// the two TOKEN caps must exist in NONE, which is the design and not an oversight.
#[test]
fn the_request_caps_sit_in_the_right_bundles_and_the_token_caps_in_none() {
    let viewer = lb_host::viewer_role_caps();
    let member = lb_host::member_role_caps();
    let admin = lb_host::workspace_admin_role_caps();

    assert!(
        member.iter().any(|c| c == SEND),
        "{SEND} must be a MEMBER cap"
    );
    assert!(
        !viewer.iter().any(|c| c == SEND),
        "a viewer must not be able to email the outside world"
    );
    for cap in [P_UPSERT, P_LIST] {
        assert!(admin.iter().any(|c| c == cap), "{cap} must be an ADMIN cap");
        assert!(
            !member.iter().any(|c| c == cap),
            "{cap} must NOT be a member cap — the roster is other companies' contact details"
        );
    }
    for cap in [lb_host::VIEW_CAP, lb_host::REPLY_CAP] {
        for (name, bundle) in [("viewer", &viewer), ("member", &member), ("admin", &admin)] {
            assert!(
                !bundle.iter().any(|c| c == cap),
                "{cap} must be in NO bundle ({name}) — it is minted onto the token principal only"
            );
        }
    }
}
