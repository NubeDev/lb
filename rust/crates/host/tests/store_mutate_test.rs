//! The generic store-**mutation** surface (`store.write` / `store.delete`) headless, against a real
//! store — the write half of the direct-store contract, gated per-table by the `store:<table>:<action>`
//! grammar. Proves the mandatory categories for a write verb:
//!   - **capability deny (per verb)** — the per-table `store:<table>:write` gate: a caller WITHOUT it
//!     is opaquely `Denied` and NO record is written / NO record is erased (assert the store after).
//!   - **workspace isolation** — a ws-B write with a ws-B token lands in ws-B's namespace only; a
//!     ws-A read of the same `table:id` sees nothing (structural, from the token — never the args).
//!   - **round-trip** — a `store.write` record reads back verbatim through `lb_store::read`, and a
//!     `store.delete` removes it (idempotent: a second delete still succeeds).
//!
//! Real infra: real embedded SurrealDB (`mem://`); records written through the real
//! `store_write_run` path, read back through the real `lb_store::read` — no mock, no fake.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{store_delete_run, store_write_run, StoreMutateError};
use lb_store::Store;
use serde_json::json;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const TABLE: &str = "widget";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_round_trips_and_delete_removes() {
    let ws = "sm-rt";
    let store = Store::memory().await.unwrap();
    let p = principal("user:ada", ws, &["store:widget:write"]);

    let value = json!({ "id": "plant-1", "name": "Plant 1", "mode": "local" });
    let (t, id) = store_write_run(&store, &p, ws, TABLE, "plant-1", &value)
        .await
        .expect("write succeeds");
    assert_eq!((t.as_str(), id.as_str()), (TABLE, "plant-1"));

    // Reads back verbatim through the raw store read (the `{ data: … }` envelope is unwrapped).
    let read = lb_store::read(&store, ws, TABLE, "plant-1")
        .await
        .unwrap()
        .expect("record present");
    assert_eq!(read, value, "round-trips verbatim");

    // Delete removes it; a second delete is idempotent (still Ok).
    store_delete_run(&store, &p, ws, TABLE, "plant-1")
        .await
        .expect("delete succeeds");
    assert!(
        lb_store::read(&store, ws, TABLE, "plant-1")
            .await
            .unwrap()
            .is_none(),
        "record erased"
    );
    store_delete_run(&store, &p, ws, TABLE, "plant-1")
        .await
        .expect("second delete is idempotent");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_without_the_per_table_cap_is_denied_and_nothing_is_written() {
    let ws = "sm-deny";
    let store = Store::memory().await.unwrap();
    // Holds the WRONG table's grant — the per-table gate must still deny `widget`.
    let p = principal("user:mallory", ws, &["store:other_table:write"]);

    let err = store_write_run(&store, &p, ws, TABLE, "x", &json!({ "a": 1 }))
        .await
        .expect_err("denied without store:widget:write");
    assert!(
        matches!(err, StoreMutateError::Denied),
        "opaque deny: {err:?}"
    );

    // The deny happened BEFORE any store write — no record exists.
    assert!(
        lb_store::read(&store, ws, TABLE, "x")
            .await
            .unwrap()
            .is_none(),
        "no record written on a denied call"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_without_the_cap_is_denied_and_nothing_is_erased() {
    let ws = "sm-deny-del";
    let store = Store::memory().await.unwrap();
    let writer = principal("user:ada", ws, &["store:widget:write"]);
    let nocap = principal("user:mallory", ws, &["store:other_table:write"]);

    store_write_run(&store, &writer, ws, TABLE, "keep", &json!({ "a": 1 }))
        .await
        .expect("seed a record to attempt to erase");

    let err = store_delete_run(&store, &nocap, ws, TABLE, "keep")
        .await
        .expect_err("delete denied without the cap");
    assert!(
        matches!(err, StoreMutateError::Denied),
        "opaque deny: {err:?}"
    );

    // The record survives — a denied delete erases nothing.
    assert!(
        lb_store::read(&store, ws, TABLE, "keep")
            .await
            .unwrap()
            .is_some(),
        "record survives a denied delete"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_write_is_invisible_to_ws_a() {
    let store = Store::memory().await.unwrap();
    let a = principal("user:a", "ws-a", &["store:widget:write"]);
    let b = principal("user:b", "ws-b", &["store:widget:write"]);

    // Same table:id, different workspaces (each token carries its own ws).
    store_write_run(
        &store,
        &a,
        "ws-a",
        TABLE,
        "shared",
        &json!({ "owner": "a" }),
    )
    .await
    .expect("ws-a write");
    store_write_run(
        &store,
        &b,
        "ws-b",
        TABLE,
        "shared",
        &json!({ "owner": "b" }),
    )
    .await
    .expect("ws-b write");

    // Each namespace holds ONLY its own record — the wall is structural (README §7).
    let ra = lb_store::read(&store, "ws-a", TABLE, "shared")
        .await
        .unwrap()
        .expect("ws-a record");
    let rb = lb_store::read(&store, "ws-b", TABLE, "shared")
        .await
        .unwrap()
        .expect("ws-b record");
    assert_eq!(ra["owner"], "a");
    assert_eq!(rb["owner"], "b");

    // A ws-A principal cannot even name ws-B in the write (the ws is host-side from the token). A
    // write by the ws-A principal into "ws-b" would be a workspace-mismatch Denied — prove it.
    let err = store_write_run(&store, &a, "ws-b", TABLE, "sneak", &json!({}))
        .await
        .expect_err("ws-a principal cannot write into ws-b");
    assert!(matches!(err, StoreMutateError::Denied), "ws wall: {err:?}");
    assert!(
        lb_store::read(&store, "ws-b", TABLE, "sneak")
            .await
            .unwrap()
            .is_none(),
        "no cross-ws record leaked"
    );
}
