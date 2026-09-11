//! The send end to end — the minted link, the ask window, the ball moving — and the party window that falls back to the policy.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::request_support::*;

// --- The send, end to end ------------------------------------------------------------------------

/// One send does all six things: the row, the mail with a working link, the window, the ladder, the
/// case's `waiting_on`, and the `request_sent` event.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_send_mints_a_link_sets_the_window_and_moves_the_ball() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "send-probe").await;
    seed_party(&node, &full, ws, "northern", 10).await;

    let request = call(
        &node,
        &full,
        ws,
        "case.request.send",
        json!({
            "case_id": case_id,
            "party_id": "northern",
            "ask": "quote",
            "brief": {
                "ask_text": "Please price replacing the actuator.",
                "currency": "AUD",
                "asset": "AHU-3",
                "evidence": {
                    "series": [ { "name": "supply temp", "points": [[1.0, 21.5], [2.0, 24.0]] } ],
                    "threshold": 22.0,
                    "unit": "degC"
                }
            },
            "ts": T0,
        }),
    )
    .await
    .expect("send ok");

    // The window came from the PARTY's own 10 hours, and both deadlines are set from it.
    assert_eq!(request["expires_ts"].as_u64(), Some(T0 + 10 * HOUR_MS));
    assert_eq!(request["respond_by"].as_u64(), Some(T0 + 10 * HOUR_MS));
    assert_eq!(request["status"].as_str(), Some("sent"));
    assert_eq!(
        request["delivery"].as_str(),
        Some("queued"),
        "nothing has been attempted yet — `queued`, never `sent`"
    );
    // Only the hash is stored, and the response does not hand back a working link.
    assert_eq!(request["token_hash"].as_str().map(str::len), Some(64));
    assert!(
        !request.to_string().contains("lbr_"),
        "the raw token must never appear in the send reply: {request}"
    );

    // The ball moved, and the history says so.
    let case = call(&node, &full, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["waiting_on"].as_str(), Some("contractor"));
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "request_sent"), 1);

    // The ladder: three one-shot reminders at 50 % / 80 % / breach of the 10-hour window.
    let request_id = request["id"].as_str().unwrap();
    let expected = [
        (T0 + 5 * HOUR_MS) / 1000,
        (T0 + 8 * HOUR_MS) / 1000,
        (T0 + 10 * HOUR_MS) / 1000,
    ];
    for (stage, at) in ["n50", "n80", "breach"].iter().zip(expected.iter()) {
        let reminder =
            lb_reminders::load(&node.store, ws, &format!("case-nudge-{request_id}-{stage}"))
                .await
                .expect("load ok")
                .unwrap_or_else(|| panic!("no {stage} nudge scheduled"));
        assert_eq!(
            reminder.next_attempt_ts, *at,
            "the {stage} rung must be pinned to the exact instant, not the next cron slot"
        );
        assert_eq!(reminder.max_runs, Some(1), "a nudge is a one-shot");
        assert_eq!(
            reminder.principal_sub, "user:test",
            "the ladder fires under the SENDER, so revoking their grant stops it"
        );
    }

    // And the mail itself carries a link the gateway can resolve.
    let provider = drain_recording(&node, ws).await;
    let sends = provider.sends();
    assert_eq!(sends[0].to, "northern@example.com");
    assert!(
        !sends[0].subject.is_empty(),
        "the catalog supplies a subject"
    );
    let token = token_from(&sends[0].body);
    assert!(token.starts_with("lbr_nube."), "{token}");
    lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("the mailed token must actually work");
}

/// The window falls back to the POLICY when the party has no opinion of its own.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_party_with_no_window_falls_back_to_the_policy() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let admin = principal("user:test", ws, &["mcp:policy.sla.set:call"]);
    let case_id = seed_case(&node, &full, ws, "window-probe").await;
    seed_party(&node, &full, ws, "quiet", 0).await;

    call(
        &node,
        &admin,
        ws,
        "policy.sla.set",
        json!({
            "id": "default",
            "name": "workspace default",
            "match": {},
            "respond_h": 4,
            "resolve_h": 48,
            "party_window_h": 6,
            "hold_down_days": 14,
        }),
    )
    .await
    .expect("policy.sla.set ok");

    let request = call(
        &node,
        &full,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "quiet", "ask": "info", "ts": T0 }),
    )
    .await
    .expect("send ok");
    assert_eq!(
        request["expires_ts"].as_u64(),
        Some(T0 + 6 * HOUR_MS),
        "the policy's party_window_h must win when the party states none"
    );
}
