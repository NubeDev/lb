//! The match ladder, the upsert-by-id, the refusals, and a business calendar round trip.
//!
//! Part of the `sla_policy` suite (see `sla_policy_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::sla_policy_support::*;

// --- the ladder, the upsert, the rejects ---------------------------------------------------------

/// The list order IS the resolution ladder: three pinned axes, then two, then one, then the
/// workspace default; equally specific rows by id ascending. An admin reads precedence off the page.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_list_is_ordered_most_specific_first() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    // Seeded in a deliberately unhelpful order.
    seed(&node, &admin, "nube", "default", json!({})).await;
    seed(&node, &admin, "nube", "zeta-site", json!({ "site": "s1" })).await;
    seed(&node, &admin, "nube", "alpha-site", json!({ "site": "s2" })).await;
    seed(
        &node,
        &admin,
        "nube",
        "all-three",
        json!({ "site": "s1", "category": "c1", "severity": "critical" }),
    )
    .await;
    seed(
        &node,
        &admin,
        "nube",
        "two",
        json!({ "site": "s1", "category": "c1" }),
    )
    .await;

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    assert_eq!(
        ids(&out),
        ["all-three", "two", "alpha-site", "zeta-site", "default"],
        "most specific first; ties by id ascending"
    );
}

/// `set` is an upsert keyed by id: a second write replaces the row rather than adding a second one.
/// This is what lets the settings surface edit in place and a pack re-seed idempotently.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_upserts_by_id_and_defaults_the_hold_down_window() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    seed(&node, &admin, "nube", "contract", json!({})).await;
    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({ "id": "contract", "name": "renegotiated", "match": {}, "respond_h": 2, "resolve_h": 8 }),
    )
    .await
    .expect("second write");

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    assert_eq!(ids(&out), ["contract"], "one row, not two");
    let row = &out
        .as_array()
        .or_else(|| out.get("policies").and_then(Value::as_array))
        .unwrap()[0];
    assert_eq!(row["name"], "renegotiated");
    assert_eq!(row["respond_h"], 2);
    // The scope's decision, surviving the round trip: a policy that says nothing about hold-down
    // holds down for 14 days.
    assert_eq!(row["hold_down_days"], 14);
}

/// A policy that cannot govern work is refused at the door, not stored and discovered later by a
/// reactor computing a nonsense deadline.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unusable_policy_is_refused_and_writes_nothing() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    // Due to be FIXED before anyone is due to have LOOKED at it.
    assert!(call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({ "id": "backwards", "match": {}, "respond_h": 24, "resolve_h": 4 }),
    )
    .await
    .is_err());

    // A timezone no tzdata knows.
    assert!(call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({
            "id": "bad-tz", "match": {}, "respond_h": 1, "resolve_h": 2,
            "calendar": { "kind": "business", "tz": "Mars/Olympus" },
        }),
    )
    .await
    .is_err());

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    assert!(ids(&out).is_empty(), "neither reject was stored");
}

/// A real business calendar round-trips through the store intact — the hours, the Monday-first
/// weekly pattern and the holiday list all survive, because the deadline arithmetic downstream is
/// only as good as what was persisted.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_business_calendar_round_trips() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    let office = json!({
        "kind": "business",
        "tz": "Australia/Brisbane",
        "hours": [
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 0, "close_min": 0 },
            { "open_min": 0, "close_min": 0 },
        ],
        "holidays": ["2026-12-25"],
    });
    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({ "id": "office", "match": {}, "respond_h": 2, "resolve_h": 20, "calendar": office }),
    )
    .await
    .expect("set");

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    let row = &out
        .as_array()
        .or_else(|| out.get("policies").and_then(Value::as_array))
        .unwrap()[0];
    assert_eq!(row["calendar"]["tz"], "Australia/Brisbane");
    assert_eq!(row["calendar"]["kind"], "business");
    // Index 0 is MONDAY and index 5/6 the weekend — the convention every deadline depends on.
    assert_eq!(row["calendar"]["hours"][0]["open_min"], 540);
    assert_eq!(row["calendar"]["hours"][5]["close_min"], 0);
    assert_eq!(row["calendar"]["holidays"][0], "2026-12-25");
}
