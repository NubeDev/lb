//! The party roster: upsert in place, narrowing, and the three ways a bad or address-less row is refused loudly.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::request_support::*;

// --- The roster -------------------------------------------------------------------------------------

/// `party.upsert` replaces in place and `party.list` narrows; a malformed party is refused before
/// anything is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_roster_upserts_in_place_narrows_and_refuses_a_bad_row() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let admin = principal("user:test", ws, ALL);

    seed_party(&node, &admin, ws, "northern", 24).await;
    seed_party(&node, &admin, ws, "northern", 48).await;
    let roster = call(&node, &admin, ws, "party.list", json!({}))
        .await
        .expect("list ok");
    let rows = roster.as_array().expect("an array");
    assert_eq!(rows.len(), 1, "an upsert replaces, never appends");
    assert_eq!(rows[0]["default_ask_window_h"].as_u64(), Some(48));

    // Narrowing by site and by kind.
    assert_eq!(
        call(&node, &admin, ws, "party.list", json!({ "site": "site-b" }))
            .await
            .expect("list ok")
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        call(&node, &admin, ws, "party.list", json!({ "kind": "client" }))
            .await
            .expect("list ok")
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    // A contact that could never be mailed is refused at the door, not at 03:00 by the relay.
    let err = call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({ "id": "broken", "kind": "fm", "name": "Broken", "contact": { "email": "not-an-address" } }),
    )
    .await
    .expect_err("a malformed email must be refused");
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
}

/// **The silent-drop regression.** A misplaced `email` (top level, not under `contact`) used to be
/// accepted: the write returned `{ "id": … }`, the address went nowhere, and the roster held a
/// contractor nobody could reach — the failure surfaced hours later at `case.request.send`, to a
/// different person. Both halves of the fix are pinned here: the door DENIES the unknown key, and a
/// party with no address is not a roster row at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_party_with_a_misplaced_or_missing_email_is_refused_loudly() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let admin = principal("user:test", ws, ALL);

    // 1. The misplaced key: `email` at the top level, exactly as an admin plausibly types it.
    let err = call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({
            "id": "acme-mech",
            "name": "Acme Mechanical",
            "email": "quotes@acme-mech.example",
            "kind": "contractor",
        }),
    )
    .await
    .expect_err("a misplaced key must be refused, not silently dropped");
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    // Nothing was written — the roster is exactly as it was.
    assert_eq!(
        call(&node, &admin, ws, "party.list", json!({}))
            .await
            .expect("list ok")
            .as_array()
            .map(Vec::len),
        Some(0),
        "a refused upsert must leave the roster untouched"
    );

    // 2. No contact at all: also refused, and the message says where the address goes.
    let err = call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({ "id": "silent", "kind": "fm", "name": "Silent" }),
    )
    .await
    .expect_err("a party nobody can email is not a roster row");
    match err {
        ToolError::BadInput(m) => assert!(m.contains("contact"), "{m}"),
        other => panic!("{other:?}"),
    }

    // 3. Correctly placed, it works — so the deny is about the KEY, not about the value.
    call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({
            "id": "acme-mech",
            "name": "Acme Mechanical",
            "kind": "contractor",
            "contact": { "email": "quotes@acme-mech.example" },
            // A `ts` rides along on every other case verb, so it is accepted and ignored here.
            "ts": T0,
        }),
    )
    .await
    .expect("the correct shape is accepted");
    let roster = call(&node, &admin, ws, "party.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(
        roster[0]["contact"]["email"].as_str(),
        Some("quotes@acme-mech.example")
    );
}

/// Defence in depth: a row written before the roster required an address — or by any path that is
/// not the verb — still cannot be asked anything, and the refusal names the reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_legacy_party_with_no_address_still_cannot_be_asked() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let admin = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &admin, ws, "no-email").await;

    // Straight into the store, bypassing the verb — the shape of a row that predates the rule.
    lb_store::write(
        &node.store,
        ws,
        "party",
        "legacy",
        &json!({ "id": "legacy", "kind": "fm", "name": "Legacy", "contact": {} }),
    )
    .await
    .expect("seed a legacy row");

    let err = call(
        &node,
        &admin,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "legacy", "ask": "quote", "ts": T0 }),
    )
    .await
    .expect_err("an ask nobody receives is not an ask");
    match err {
        ToolError::BadInput(m) => assert!(m.contains("no email address"), "{m}"),
        other => panic!("{other:?}"),
    }
}
