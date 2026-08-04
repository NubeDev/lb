//! `series.rollup.read` at the MCP surface — the STORED rollup rows of one series, verbatim.
//!
//! **Why a verb distinct from `series.read {mode:"buckets"}`.** The bucketed read decimates live raw
//! and merges the stored tail beneath it, and it SUPPRESSES a stored row while the raw underneath it
//! survives (else every sample folds twice). So a window can hold rollup rows and the bucketed read
//! will honestly report none of them. That makes two operator questions unanswerable through it:
//! *did the GC actually fold anything*, and *what is physically on disc for this tier*.
//!
//! **Load-bearing: this verb never falls back.** An empty result means "nothing is folded in this
//! window" — a real answer. Serving decimated raw instead would make a rollup panel silently
//! indistinguishable from a raw one, which is the exact confusion the whole feature exists to end.
//! The test below pins that by asking for a window where raw is live and rollups are absent, and
//! asserting the reply is EMPTY rather than a decimated raw payload.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::call_ingest_tool;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::json;

const READ: &str = "mcp:series.read:call";

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

fn sample(series: &str, seq: u64, payload: serde_json::Value) -> serde_json::Value {
    json!({ "series": series, "producer": "edge-a", "ts": seq * 1000, "seq": seq,
            "payload": payload, "qos": "best-effort" })
}

/// Seed `n` samples on `cpu` at 1s spacing and return a principal holding every verb used here.
async fn seed(store: &Store, ws: &str, n: u64) -> Principal {
    let p = principal(
        "client:pi-7",
        ws,
        &[
            "mcp:ingest.write:call",
            "mcp:series.retention.set:call",
            "mcp:series.retention.gc:call",
            READ,
        ],
    );
    let samples: Vec<_> = (1..=n).map(|s| sample("cpu", s, json!(s as f64))).collect();
    call_ingest_tool(
        store,
        &p,
        ws,
        "ingest.write",
        &json!({ "samples": samples }),
    )
    .await
    .unwrap();
    p
}

/// Fold `cpu` into a 10s tier and evict the raw beneath it.
async fn fold(store: &Store, p: &Principal, ws: &str, now_ms: u64) {
    call_ingest_tool(
        store,
        p,
        ws,
        "series.retention.set",
        &json!({ "prefix": "cpu", "raw_for_ms": 1, "max_samples": 0,
                 "tiers": [{ "width_ms": 10_000, "keep_for_ms": 0 }] }),
    )
    .await
    .unwrap();
    call_ingest_tool(
        store,
        p,
        ws,
        "series.retention.gc",
        &json!({ "now_ms": now_ms }),
    )
    .await
    .unwrap();
}

/// Stored rows come back on the TIER's grid with the full stat set — the long-horizon view.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reads_the_stored_rows_with_their_full_stat_set() {
    let store = Store::memory().await.unwrap();
    let p = seed(&store, "acme", 5).await;
    fold(&store, &p, "acme", 60_000).await;

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.rollup.read",
        &json!({ "series": "cpu", "from": 0, "to": 60_000 }),
    )
    .await
    .unwrap();

    let rows = out["rows"].as_array().expect("rows array");
    assert_eq!(rows.len(), 1, "one 10s bucket holds the five samples");
    let r = &rows[0];
    assert_eq!(r["width_ms"].as_u64(), Some(10_000), "the tier's own width");
    assert_eq!(r["t"].as_u64(), Some(0), "on the tier's own grid");
    assert_eq!(r["count"].as_u64(), Some(5));
    assert_eq!(r["num_count"].as_u64(), Some(5));
    // sum + count, NOT a stored mean — this is what keeps a wider re-aggregation exact.
    assert_eq!(r["sum"].as_f64(), Some(15.0));
    assert_eq!(r["min"].as_f64(), Some(1.0));
    assert_eq!(r["max"].as_f64(), Some(5.0));
    // The widths present ride along so a caller knows the grid it is plotting on.
    assert_eq!(out["widths"].as_array().unwrap(), &[json!(10_000)]);
}

/// The no-fallback guarantee: a window with LIVE raw and no stored rows reads EMPTY. If this verb
/// ever quietly served decimated raw, this is the assertion that would catch it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_window_with_no_stored_rows_is_empty_never_decimated_raw() {
    let store = Store::memory().await.unwrap();
    // Seeded but NEVER folded — five raw samples are on disc, `series_rollup` is untouched.
    let p = seed(&store, "acme", 5).await;

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.rollup.read",
        &json!({ "series": "cpu", "from": 0, "to": 60_000 }),
    )
    .await
    .unwrap();

    assert_eq!(
        out["rows"].as_array().map(|r| r.len()),
        Some(0),
        "raw is live and nothing is folded — the honest answer is nothing, not the raw decimated"
    );
    assert_eq!(out["widths"].as_array().map(|w| w.len()), Some(0));
}

/// Rows outside the requested window are not returned — the window is a real half-open filter, not
/// a suggestion, so "past the tier's keep_for_ms" reads as empty rather than as stale rows.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_window_actually_bounds_the_result() {
    let store = Store::memory().await.unwrap();
    let p = seed(&store, "acme", 5).await;
    fold(&store, &p, "acme", 60_000).await;

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.rollup.read",
        // The stored row starts at t=0; ask strictly after it.
        &json!({ "series": "cpu", "from": 10_000, "to": 60_000 }),
    )
    .await
    .unwrap();
    assert_eq!(out["rows"].as_array().map(|r| r.len()), Some(0));
}

/// Same wall as every other projection of this series: no `mcp:series.read:call`, no rows. Denials
/// are opaque — a refused caller learns nothing about whether the series exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_read_cap_it_denies() {
    let store = Store::memory().await.unwrap();
    let p = seed(&store, "acme", 5).await;
    fold(&store, &p, "acme", 60_000).await;

    let capless = principal("client:nope", "acme", &[]);
    let err = call_ingest_tool(
        &store,
        &capless,
        "acme",
        "series.rollup.read",
        &json!({ "series": "cpu", "from": 0, "to": 60_000 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

/// Workspace-first: a ws-B principal cannot read ws-A's stored rollups even holding the cap.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn it_cannot_reach_across_workspaces() {
    let store = Store::memory().await.unwrap();
    let pa = seed(&store, "ws-a", 5).await;
    fold(&store, &pa, "ws-a", 60_000).await;

    let pb = principal("client:other", "ws-b", &[READ]);
    let err = call_ingest_tool(
        &store,
        &pb,
        "ws-a",
        "series.rollup.read",
        &json!({ "series": "cpu", "from": 0, "to": 60_000 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}
