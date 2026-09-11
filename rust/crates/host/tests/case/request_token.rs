//! The token principal: it reaches its own request and nothing else, a dead token never says which kind of dead, a replay writes one event, and every reply kind moves the case the documented way.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::request_support::*;

// --- The token principal --------------------------------------------------------------------------

/// A token views and replies on ITS OWN request only; any other request, and any other verb, is
/// `Denied`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_token_reaches_its_own_request_and_nothing_else() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_a = seed_case(&node, &full, ws, "token-a").await;
    let case_b = seed_case(&node, &full, ws, "token-b").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    seed_party(&node, &full, ws, "southern", 24).await;

    let (id_a, token_a) = send_ask(&node, &full, ws, &case_a, "northern").await;
    let (id_b, _token_b) = send_ask(&node, &full, ws, &case_b, "southern").await;

    let (party, _row) = lb_host::case_request_authenticate(&node.store, ws, &token_a, T0 + 1)
        .await
        .expect("a live token resolves");
    assert_eq!(party.sub(), "party:northern", "attributed to the PARTY");

    // Its own request: fine.
    let view = call(
        &node,
        &party,
        ws,
        "case.request.view",
        json!({ "id": id_a, "ts": T0 + 1 }),
    )
    .await
    .expect("a token views its own request");
    assert_eq!(view["request_id"].as_str(), Some(id_a.as_str()));
    assert_eq!(view["party_name"].as_str(), Some("northern services"));
    assert!(
        view.get("case_id").is_none() && view.get("token_hash").is_none(),
        "the view must not carry the case id or the credential: {view}"
    );

    // Another party's request: denied, on both verbs.
    for (tool, input) in [
        ("case.request.view", json!({ "id": id_b, "ts": T0 + 1 })),
        (
            "case.request.reply",
            json!({ "id": id_b, "reply": { "kind": "accept" }, "ts": T0 + 1 }),
        ),
    ] {
        let err = call(&node, &party, ws, tool, input)
            .await
            .expect_err(&format!("{tool} on another request must deny"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // Every other verb on the node: denied.
    for (tool, input) in [
        ("case.get", json!({ "id": case_a })),
        ("case.list", json!({ "lane": "watching" })),
        ("case.events", json!({ "case_id": case_a })),
        (
            "case.workflow",
            json!({ "id": case_a, "workflow": "actioned" }),
        ),
        (
            "case.request.send",
            json!({ "case_id": case_a, "party_id": "southern", "ask": "quote" }),
        ),
        ("party.list", json!({})),
        (
            "insight.raise",
            json!({ "dedup_key": "x", "severity": "info", "title": "x" }),
        ),
    ] {
        let err = call(&node, &party, ws, tool, input)
            .await
            .expect_err(&format!("{tool} must deny a token principal"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }
}

/// A withdrawn ask, and an expired one, are both `Gone` — and the caller cannot tell which.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_withdrawn_or_expired_token_is_gone_and_never_says_which() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_a = seed_case(&node, &full, ws, "gone-a").await;
    let case_b = seed_case(&node, &full, ws, "gone-b").await;
    seed_party(&node, &full, ws, "northern", 2).await;
    seed_party(&node, &full, ws, "southern", 2).await;

    let (id_a, token_a) = send_ask(&node, &full, ws, &case_a, "northern").await;
    let (_id_b, token_b) = send_ask(&node, &full, ws, &case_b, "southern").await;

    // Withdrawn.
    call(
        &node,
        &full,
        ws,
        "case.request.withdraw",
        json!({ "id": id_a, "ts": T0 + 1 }),
    )
    .await
    .expect("withdraw ok");
    let withdrawn = lb_host::case_request_authenticate(&node.store, ws, &token_a, T0 + 2).await;
    assert_eq!(withdrawn.err(), Some(lb_host::RequestTokenError::Gone));

    // Expired — the same error value, byte for byte, so nothing downstream can distinguish them.
    let expired =
        lb_host::case_request_authenticate(&node.store, ws, &token_b, T0 + 3 * HOUR_MS).await;
    assert_eq!(expired.err(), Some(lb_host::RequestTokenError::Gone));

    // And a token that never existed is a different, equally opaque answer.
    let unknown =
        lb_host::case_request_authenticate(&node.store, ws, "lbr_nube.nonsense", T0).await;
    assert_eq!(unknown.err(), Some(lb_host::RequestTokenError::NotFound));
}

/// A replayed reply produces ONE event, one transition and one quote — the property a contractor on
/// a flaky signal depends on.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_replayed_reply_produces_exactly_one_event() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "replay-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
    let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("resolves");

    let reply =
        json!({ "kind": "quote", "amount": 1180.0, "currency": "AUD", "text": "parts + labour" });
    let first = call(
        &node,
        &party,
        ws,
        "case.request.reply",
        json!({ "id": id, "reply": reply, "ts": T0 + 2 }),
    )
    .await
    .expect("first reply ok");
    assert_eq!(first["recorded"].as_bool(), Some(true));

    for _ in 0..3 {
        let again = call(
            &node,
            &party,
            ws,
            "case.request.reply",
            json!({ "id": id, "reply": reply, "ts": T0 + 3 }),
        )
        .await
        .expect("a replay must succeed, not error — the party retried a POST");
        assert_eq!(again["recorded"].as_bool(), Some(false));
    }

    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(
        count_kind(&events, "reply"),
        1,
        "four calls, one reply event: {events:#?}"
    );

    let case = call(&node, &full, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["cost_to_fix"].as_f64(), Some(1180.0));
}

/// Each of the six reply kinds produces the documented `workflow` / `waiting_on` and an event
/// attributed to `party:{id}` — not to a login the contractor never had.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_reply_kind_moves_the_case_the_documented_way() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    seed_party(&node, &full, ws, "northern", 24).await;

    let expected = [
        (
            "accept",
            json!({ "kind": "accept" }),
            "actioned",
            "contractor",
        ),
        (
            "quote",
            json!({ "kind": "quote", "amount": 900.0, "currency": "AUD" }),
            "waiting_on_po",
            "client",
        ),
        (
            "eta",
            json!({ "kind": "eta", "eta_ts": T0 + 48 * HOUR_MS }),
            "actioned",
            "contractor",
        ),
        ("done", json!({ "kind": "done" }), "actioned", "internal"),
        (
            "need_info",
            json!({ "kind": "need_info", "text": "which floor?" }),
            "to_action",
            "internal",
        ),
        (
            "decline",
            json!({ "kind": "decline" }),
            "to_action",
            "internal",
        ),
    ];

    for (name, reply, workflow, waiting_on) in expected {
        let case_id = seed_case(&node, &full, ws, &format!("reply-{name}")).await;
        let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
        let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
            .await
            .expect("resolves");
        call(
            &node,
            &party,
            ws,
            "case.request.reply",
            json!({ "id": id, "reply": reply, "ts": T0 + 2 }),
        )
        .await
        .unwrap_or_else(|e| panic!("{name} reply failed: {e:?}"));

        let case = call(&node, &full, ws, "case.get", json!({ "id": case_id }))
            .await
            .expect("get ok");
        assert_eq!(case["workflow"].as_str(), Some(workflow), "{name}");
        assert_eq!(case["waiting_on"].as_str(), Some(waiting_on), "{name}");
        assert_ne!(
            case["workflow"].as_str(),
            Some("resolved"),
            "{name}: no reply may close a case — that needs a resolution a party cannot give"
        );

        let events = events_of(&node, &full, ws, &case_id).await;
        let reply_event = events
            .iter()
            .find(|e| e["kind"].as_str() == Some("reply"))
            .unwrap_or_else(|| panic!("{name}: no reply event in {events:#?}"));
        assert_eq!(
            reply_event["actor"].as_str(),
            Some("party:northern"),
            "{name}: the reply is the PARTY's, not a login's"
        );
        assert_eq!(reply_event["data"]["kind"].as_str(), Some(name));
    }
}
