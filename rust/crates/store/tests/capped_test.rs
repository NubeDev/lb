//! The capped-retention primitive's tests — the **load-bearing** new piece of the telemetry console
//! (telemetry-console scope). All four mandatory properties of `capped_insert` are proven here against
//! a real embedded `mem://` store (testing §0/§3.1, CLAUDE §9 — no mocks):
//!
//! 1. **FIFO-cap** — inserting `cap + k` rows for one key leaves exactly `cap`, survivors are the
//!    newest (oldest evicted).
//! 2. **Per-source vs global from one helper** — a per-source key does NOT evict a quiet source's
//!    rows; a global key bounds across sources. Same `capped_insert`, different selector.
//! 3. **Concurrency (the correctness trap)** — concurrent inserts past the cap leave EXACTLY `cap`,
//!    never over-evicted nor overgrown. This is the test that proves the single-transaction design
//!    and would FAIL the racy count-then-delete version.
//! 4. **Zero-cap is clamped** — never deletes the row just written.

use std::time::Duration;

use lb_store::{capped_insert, new_ulid, Store};
use serde_json::json;

/// Count the rows in `table` (in `ws`) whose `cap_key == key`. The trim invariant is read back
/// through the real store, never a mirror.
async fn count_key(store: &Store, ws: &str, table: &str, key: &str) -> usize {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT count() AS c FROM type::table($tb) WHERE cap_key = $key GROUP ALL",
            vec![("tb".into(), json!(table)), ("key".into(), json!(key))],
        )
        .await
        .expect("count query");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take count rows");
    rows.first()
        .and_then(|r| r.get("c"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

/// The seq values (ordered ascending) for `key` in `table` — the insert-seq of the survivors, used
/// to assert WHICH rows survived (the newest).
async fn key_ids(store: &Store, ws: &str, table: &str, key: &str) -> Vec<String> {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT VALUE seq FROM type::table($tb) WHERE cap_key = $key ORDER BY seq ASC",
            vec![("tb".into(), json!(table)), ("key".into(), json!(key))],
        )
        .await
        .expect("seq query");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take seq rows");
    rows.into_iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

#[tokio::test]
async fn fifo_cap_keeps_exactly_cap_newest_rows() {
    let store = Store::memory().await.expect("open store");
    let cap = 5;
    // Insert cap + 3 = 8 rows for one key, each tagged with its insertion ordinal so we can name
    // the survivors. ULIDs guarantee a monotonic insertion order (one async task, awaited in turn).
    for i in 0..8u32 {
        let id = format!("seq-{i:04}");
        capped_insert(&store, "ws", "ring", &id, "k", cap, &json!({ "ord": i }))
            .await
            .expect("capped_insert");
        // ULIDs minted in the same ms can tie-break arbitrarily, so space the inserts so each lands
        // in a strictly-later ms than the last → a deterministic total order for the FIFO assertion.
        tokio::time::sleep(Duration::from_millis(2)).await;
    }

    assert_eq!(
        count_key(&store, "ws", "ring", "k").await,
        cap,
        "exactly cap"
    );

    // Survivors must be the NEWEST — ordinals 3..8 (the oldest 0,1,2 evicted, FIFO).
    let ids = key_ids(&store, "ws", "ring", "k").await;
    let ords: Vec<u32> = ids
        .iter()
        .map(|s| s.trim_start_matches("seq-").parse::<u32>().unwrap())
        .collect();
    assert_eq!(ords, vec![3, 4, 5, 6, 7], "survivors are the newest");
}

#[tokio::test]
async fn per_source_key_does_not_evict_a_quiet_source() {
    let store = Store::memory().await.expect("open store");
    let cap = 3;

    // A chatty source fills its ring to the cap.
    for i in 0..cap + 2 {
        capped_insert(
            &store,
            "ws",
            "ring",
            &new_ulid(),
            "chatty",
            cap,
            &json!({ "i": i }),
        )
        .await
        .unwrap();
    }
    // A quiet source writes ONE row.
    let quiet_id = new_ulid();
    capped_insert(
        &store,
        "ws",
        "ring",
        &quiet_id,
        "quiet",
        cap,
        &json!({ "i": 0 }),
    )
    .await
    .unwrap();

    assert_eq!(count_key(&store, "ws", "ring", "chatty").await, cap);
    assert_eq!(
        count_key(&store, "ws", "ring", "quiet").await,
        1,
        "a quiet source is not evicted by a chatty one under the per-source key"
    );
}

#[tokio::test]
async fn global_key_bounds_across_sources() {
    let store = Store::memory().await.expect("open store");
    let cap = 4;
    let global = "ws"; // the per-workspace backstop key

    // Two sources share one global cap. Together they exceed it; the ring is bounded across sources.
    for i in 0..3 {
        capped_insert(
            &store,
            "ws",
            "ring",
            &new_ulid(),
            global,
            cap,
            &json!({ "src": "a", "i": i }),
        )
        .await
        .unwrap();
    }
    for i in 0..5 {
        capped_insert(
            &store,
            "ws",
            "ring",
            &new_ulid(),
            global,
            cap,
            &json!({ "src": "b", "i": i }),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        count_key(&store, "ws", "ring", global).await,
        cap,
        "the global key bounds the ring across sources (same helper, different selector)"
    );
}

/// THE concurrency test. Fire many concurrent `capped_insert`s well past the cap from independent
/// tasks and assert the final count is EXACTLY `cap`. The racy count-then-delete version would
/// over-evict (two concurrent deletes each see count==cap) or overgrow (no transaction). The single
/// SurrealDB transaction makes the final count exact regardless of contention.
#[tokio::test]
async fn concurrent_inserts_past_cap_leave_exactly_cap() {
    let store = Store::memory().await.expect("open store");
    let cap = 20;
    let total = cap * 5; // 5x over-cap, concurrently

    let mut handles = Vec::new();
    for i in 0..total {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            capped_insert(&s, "ws", "ring", &new_ulid(), "k", cap, &json!({ "i": i }))
                .await
                .expect("capped_insert under contention");
        }));
    }
    for h in handles {
        h.await.expect("task panicked");
    }

    assert_eq!(
        count_key(&store, "ws", "ring", "k").await,
        cap,
        "final count must be EXACTLY cap under concurrency — not over-evicted, not overgrown"
    );
}

#[tokio::test]
async fn zero_cap_is_clamped_to_one() {
    let store = Store::memory().await.expect("open store");
    // cap=0 must not delete the row just written (clamped to 1).
    capped_insert(
        &store,
        "ws",
        "ring",
        &new_ulid(),
        "k",
        0,
        &json!({ "x": 1 }),
    )
    .await
    .unwrap();
    assert_eq!(count_key(&store, "ws", "ring", "k").await, 1);
}

/// `capped_insert` stores only the body + the injected `cap_key` — no extra envelope, no leaking of
/// the key twice, and arbitrary JSON (the redacted event schema) round-trips verbatim.
#[tokio::test]
async fn stores_body_with_injected_cap_key() {
    let store = Store::memory().await.expect("open store");
    let body = json!({ "level": "warn", "msg": "hi", "fields": { "n": 7 } });
    capped_insert(&store, "ws", "ring", "id-1", "k", 10, &body)
        .await
        .unwrap();

    let mut resp = store
        .query_ws(
            "ws",
            "SELECT * OMIT id FROM type::thing($tb, $id)",
            vec![("tb".into(), json!("ring")), ("id".into(), json!("id-1"))],
        )
        .await
        .expect("read back");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take row");
    let row = &rows[0];
    assert_eq!(row["level"], "warn");
    assert_eq!(row["fields"]["n"], 7);
    assert_eq!(
        row["cap_key"], "k",
        "cap_key is injected into the stored body"
    );
}
