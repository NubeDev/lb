//! The nudge ladder — it fires under the sender LIVE grant and stops without it — and the reply that cancels the rest.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::request_support::*;

// --- The nudge ladder ------------------------------------------------------------------------------

/// The scheduled nudges **actually fire** when the real reminder reactor is driven — and the whole
/// thing hangs off the sender's live grant, so the test first proves it goes RED without it
/// (`green-while-broken-reactor-tests.md`: a reactor test that never turns the cap off is green
/// over a broken wall).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_ladder_fires_under_the_senders_live_grant_and_stops_without_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "nudge-probe").await;
    seed_party(&node, &full, ws, "northern", 10).await;
    let (id, _token) = send_ask(&node, &full, ws, &case_id, "northern").await;

    // RED FIRST: the sender holds NO durable grant, so the fire-time re-resolve finds nothing and
    // the nudge is denied. Nothing is chased and nothing is counted.
    let at_50 = (T0 + 5 * HOUR_MS) / 1000;
    let pass = lb_host::react_to_reminders(&node, ws, at_50)
        .await
        .expect("a reactor pass");
    assert_eq!(
        pass.denied, 1,
        "an ungranted sender must be DENIED at fire time"
    );
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
        listed.as_array().unwrap()[0]["nudges_sent"].as_u64(),
        Some(0)
    );

    // GREEN: grant the sender the cap the nudge rides on, and drive the 80 % rung.
    grant(&node.store, ws, "user:test", SEND).await;
    let at_80 = (T0 + 8 * HOUR_MS) / 1000;
    let pass = lb_host::react_to_reminders(&node, ws, at_80)
        .await
        .expect("a reactor pass");
    assert_eq!(pass.fired, 1, "the 80 % rung must fire: {pass:?}");

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
        listed.as_array().unwrap()[0]["nudges_sent"].as_u64(),
        Some(1),
        "a fired nudge is counted on the row"
    );
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "nudge"), 1);

    // The chase is a real email, staged through the real outbox.
    let provider = drain_recording(&node, ws).await;
    let sends = provider.sends();
    assert_eq!(sends.len(), 1, "one nudge mail");
    assert!(
        !sends[0].body.contains("/r/"),
        "a nudge carries no link — the raw token is unrecoverable and re-minting would kill the \
         one already in their inbox: {}",
        sends[0].body
    );

    // The BREACH rung escalates rather than chasing: an event, no third email.
    let at_breach = (T0 + 10 * HOUR_MS) / 1000;
    lb_host::react_to_reminders(&node, ws, at_breach)
        .await
        .expect("a reactor pass");
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "breach"), 1, "{events:#?}");
    let provider = drain_recording(&node, ws).await;
    assert!(
        provider.sends().is_empty(),
        "the breach rung must not mail the party a third time"
    );

    // And the ask itself now reads expired rather than still waiting.
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
        listed.as_array().unwrap()[0]["status"].as_str(),
        Some("expired")
    );
    let _ = id;
}

/// Answering cancels the ladder: a nudge that fires afterwards chases nobody.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reply_cancels_the_remaining_nudges() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    grant(&node.store, ws, "user:test", SEND).await;
    let case_id = seed_case(&node, &full, ws, "cancel-probe").await;
    seed_party(&node, &full, ws, "northern", 10).await;
    let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
    let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("resolves");
    call(
        &node,
        &party,
        ws,
        "case.request.reply",
        json!({ "id": id, "reply": { "kind": "accept" }, "ts": T0 + 2 }),
    )
    .await
    .expect("reply ok");

    let pass = lb_host::react_to_reminders(&node, ws, (T0 + 9 * HOUR_MS) / 1000)
        .await
        .expect("a reactor pass");
    assert_eq!(
        pass.fired, 0,
        "an answered ask must have no live rungs left: {pass:?}"
    );
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "nudge"), 0);
}
