//! Retiring and deleting a party, and the same for a policy (case-plane scope §Wave 5).
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures). One binary:
//! `case_suite.rs`.
//!
//! **Disable and delete answer different questions**, and the tests below are mostly about the line
//! between them. `active: false` retires a party that has been USED: it leaves the roster's default
//! read and takes no new asks, while every request already sent to it still resolves its name for
//! the history and the nudge ladder. `party.delete` erases a row that has nothing to preserve — and
//! REFUSES one that does, because a `case_request` pointing at nothing turns "who did we send this
//! to?" into a dangling id.

use super::request_support::*;

/// The whole lifecycle in one walk: retire → invisible to the picker, visible to the admin, refused
/// by the ask plane, still readable by id → re-enable → sendable again.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_retired_party_leaves_the_picker_but_not_the_history() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());
    seed_party(&node, &admin, "nube", "acme", 0).await;

    // Visible while active.
    let listed = call(&node, &admin, "nube", "party.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(listed.as_array().map(Vec::len), Some(1), "{listed}");

    // Retire it — through the SAME upsert verb, because retirement is an edit, not a deletion.
    call(
        &node,
        &admin,
        "nube",
        "party.upsert",
        party_input("acme", false),
    )
    .await
    .expect("retired");

    // GONE from the default read — the picker must not offer a company you stopped using…
    let listed = call(&node, &admin, "nube", "party.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(
        listed.as_array().map(Vec::len),
        Some(0),
        "a retired party is not offered: {listed}"
    );

    // …and PRESENT when the settings surface asks, because an admin cannot re-enable a row they
    // cannot see. That asymmetry is the point of the flag.
    let all = call(
        &node,
        &admin,
        "nube",
        "party.list",
        json!({ "include_disabled": true }),
    )
    .await
    .expect("list ok");
    assert_eq!(all.as_array().map(Vec::len), Some(1), "{all}");
    assert_eq!(all[0]["active"], json!(false), "{all}");

    // Re-enabling is the same edit in reverse, and it is offered again.
    call(
        &node,
        &admin,
        "nube",
        "party.upsert",
        party_input("acme", true),
    )
    .await
    .expect("re-enabled");
    let listed = call(&node, &admin, "nube", "party.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(listed.as_array().map(Vec::len), Some(1), "{listed}");
}

/// **The ask plane REFUSES a retired party** — and still resolves one for a request already sent.
///
/// This is the load-bearing pair. Without the first half `active: false` is a label the roster read
/// honours and `case.request.send` ignores, which is worse than not having the flag; without the
/// second, retiring a contractor would blank the history of every job they ever did.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_retired_party_is_refused_a_new_ask_but_still_resolves_for_an_old_one() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());
    seed_roster(&node, "nube").await;
    seed_party(&node, &admin, "nube", "acme", 0).await;

    // One ask BEFORE retirement — this is the history that must survive.
    let first = seed_case(&node, &admin, "nube", "retire-a").await;
    let sent = call(
        &node,
        &admin,
        "nube",
        "case.request.send",
        json!({ "case_id": first, "party_id": "acme", "ask": "quote", "ts": T0 }),
    )
    .await
    .expect("sent while active");
    let request_id = sent["id"].as_str().expect("a request id").to_string();

    call(
        &node,
        &admin,
        "nube",
        "party.upsert",
        party_input("acme", false),
    )
    .await
    .expect("retired");

    // A NEW ask is refused, and the refusal says what to do about it.
    let second = seed_case(&node, &admin, "nube", "retire-b").await;
    let err = call(
        &node,
        &admin,
        "nube",
        "case.request.send",
        json!({ "case_id": second, "party_id": "acme", "ask": "quote", "ts": T0 + HOUR_MS }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, ToolError::BadInput(m) if m.contains("retired")),
        "a retired party takes no new asks: {err:?}"
    );

    // …and the request already sent to them still resolves the party, by id. `party_get` is
    // deliberately unfiltered; only the LIST and the send care about the flag.
    let listed = call(
        &node,
        &admin,
        "nube",
        "case.request.list",
        json!({ "case_id": first }),
    )
    .await
    .expect("request list ok");
    assert!(
        listed.to_string().contains(&request_id),
        "the earlier request survives its party's retirement: {listed}"
    );
}

/// **A row written before `active` existed reads as ACTIVE.** The bare `bool` default is `false`,
/// so the obvious attribute would have retired every party in every workspace on upgrade and
/// emptied every roster. This is the test that would have caught it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_party_row_without_the_flag_is_active() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());

    // Written straight to the store WITHOUT `active`, exactly as a pre-upgrade row or an old pack
    // seed looks on disk.
    lb_store::write(
        &node.store,
        "nube",
        "party",
        "legacy",
        &json!({
            "id": "legacy", "kind": "contractor", "name": "Legacy Co",
            "contact": { "email": "legacy@example.invalid" }, "sites": ["site-001"]
        }),
    )
    .await
    .expect("seeded");

    let listed = call(&node, &admin, "nube", "party.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(
        listed.as_array().map(Vec::len),
        Some(1),
        "a row with no `active` field must read as ACTIVE, not vanish: {listed}"
    );
    assert_eq!(listed[0]["active"], json!(true), "{listed}");
}

/// Delete refuses a party with history, and NAMES the alternative. The refusal is the feature: the
/// admin reaching for delete on a party that has been used almost always wants retirement.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_refuses_a_party_that_has_been_asked_and_points_at_retiring() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());
    seed_roster(&node, "nube").await;
    seed_party(&node, &admin, "nube", "acme", 0).await;
    let case_id = seed_case(&node, &admin, "nube", "lifecycle-a").await;
    call(
        &node,
        &admin,
        "nube",
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "acme", "ask": "quote", "ts": T0 }),
    )
    .await
    .expect("sent");

    let err = call(
        &node,
        &admin,
        "nube",
        "party.delete",
        json!({ "id": "acme" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, ToolError::BadInput(m)
            if m.contains("request") && m.contains("active")),
        "the refusal must name the reason AND the alternative: {err:?}"
    );

    // REFUSED MEANS UNCHANGED — the row is still there and still usable.
    let all = call(
        &node,
        &admin,
        "nube",
        "party.list",
        json!({ "include_disabled": true }),
    )
    .await
    .expect("list ok");
    assert_eq!(all.as_array().map(Vec::len), Some(1), "{all}");
}

/// Delete erases a party with NO history, and is idempotent on a row that is already gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_erases_an_unused_party_and_is_idempotent() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());
    seed_party(&node, &admin, "nube", "typo", 0).await;

    let out = call(
        &node,
        &admin,
        "nube",
        "party.delete",
        json!({ "id": "typo" }),
    )
    .await
    .expect("deleted");
    assert_eq!(out["removed"], json!(true), "{out}");

    // Deleting nothing is not an error — a caller retrying a delete it already made must not be
    // told it failed.
    let again = call(
        &node,
        &admin,
        "nube",
        "party.delete",
        json!({ "id": "typo" }),
    )
    .await
    .expect("idempotent");
    assert_eq!(again["removed"], json!(false), "{again}");
}

/// The cap wall on both destructive verbs, and the deny is opaque.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_authoring_caps_do_not_buy_the_destructive_ones() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // Everything an admin has EXCEPT the two delete caps.
    let author = principal("user:author", "nube", &author_caps());

    for (tool, id) in [("party.delete", "acme"), ("policy.sla.delete", "p1")] {
        let err = call(&node, &author, "nube", tool, json!({ "id": id }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{tool} must be Denied without its own cap, got {err:?}"
        );
    }
}

/// A retired POLICY governs no new case — enforced in the ladder, not only in the settings list. A
/// flag the resolver ignored would be disabled in name only.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_retired_policy_is_skipped_by_the_ladder() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());

    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        policy_input("p1", true),
    )
    .await
    .expect("set");
    let listed = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("list");
    assert_eq!(listed.as_array().map(Vec::len), Some(1), "{listed}");

    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        policy_input("p1", false),
    )
    .await
    .expect("retired");
    let listed = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("list");
    assert_eq!(
        listed.as_array().map(Vec::len),
        Some(0),
        "a retired policy leaves the default read: {listed}"
    );
    let all = call(
        &node,
        &admin,
        "nube",
        "policy.sla.list",
        json!({ "include_disabled": true }),
    )
    .await
    .expect("list");
    assert_eq!(all.as_array().map(Vec::len), Some(1), "{all}");
}

/// A policy delete is UNCONDITIONAL — unlike a party's — because a case carries the deadlines it was
/// stamped with rather than re-deriving them, so nothing is orphaned.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_policy_deletes_unconditionally() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());
    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        policy_input("p1", true),
    )
    .await
    .expect("set");

    let out = call(
        &node,
        &admin,
        "nube",
        "policy.sla.delete",
        json!({ "id": "p1" }),
    )
    .await
    .expect("deleted");
    assert_eq!(out["removed"], json!(true), "{out}");
    let listed = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("list");
    assert_eq!(listed.as_array().map(Vec::len), Some(0), "{listed}");
}
