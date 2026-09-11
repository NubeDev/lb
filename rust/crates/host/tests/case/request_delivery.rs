//! Resolved decision 7: delivery mirrors the PROVIDER. A node with no mailer says logged and never sent.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::request_support::*;

// --- Resolved decision 7: delivery mirrors the PROVIDER ------------------------------------------

/// **The load-bearing one.** A dev node's `LoggingEmailProvider` acknowledges every mail it DROPS,
/// so the outbox row reads `delivered`. `delivery` must still read `logged` — never `sent` — or the
/// drawer tells an operator an email went out that did not
/// (`outbox-delivered-is-not-email-sent.md`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delivery_says_logged_when_the_node_has_no_mailer() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "logged-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    call(
        &node,
        &full,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "northern", "ask": "quote", "ts": T0 }),
    )
    .await
    .expect("send ok");

    // The REAL relay, the REAL email target, the shipped logging provider.
    drain_logging(&node, ws).await;

    let listed = call(
        &node,
        &full,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("list ok");
    let row = &listed.as_array().expect("an array")[0];
    assert_eq!(
        row["delivery"].as_str(),
        Some("logged"),
        "a logging provider must never yield `sent`: {row}"
    );
    assert!(
        row.get("token_hash").is_none(),
        "the drawer must never be handed the credential: {row}"
    );
}

/// The same path with a provider that really hands the message on yields `sent` — so the previous
/// test is testing the PROVIDER KIND and not simply a field that is always `logged`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delivery_says_sent_when_a_real_provider_took_the_message() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "sent-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    let (_id, _token) = send_ask(&node, &full, ws, &case_id, "northern").await;

    let listed = call(
        &node,
        &full,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        listed.as_array().expect("an array")[0]["delivery"].as_str(),
        Some("sent")
    );
}
