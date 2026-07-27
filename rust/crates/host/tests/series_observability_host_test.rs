//! The OBSERVABILITY half of the series plane at the MCP surface (series-observability scope):
//! `series.stats` (the data-plane read) and `series.retention.status` (the admin-plane read),
//! through `call_ingest_tool` against a real `Store::memory()`.
//!
//! **Load-bearing:** the two verbs carry SEPARATE capabilities on purpose, so a client degrades per
//! fact — counts and freshness without the admin bookkeeping, or vice versa. The deny tests below
//! run in BOTH directions because a single-direction test passes just as happily against a gate that
//! is wired to the wrong capability.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::call_ingest_tool;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::json;

const STATS: &str = "mcp:series.stats:call";
const STATUS: &str = "mcp:series.retention.status:call";

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

/// Seed `n` samples on `cpu` in `ws` and return a principal holding every verb this file drives.
async fn seed_via_mcp(store: &Store, ws: &str, n: u64) -> Principal {
    let p = principal(
        "client:pi-7",
        ws,
        &[
            "mcp:ingest.write:call",
            "mcp:series.retention.set:call",
            "mcp:series.retention.gc:call",
            STATS,
            STATUS,
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

async fn set_policy(store: &Store, p: &Principal, ws: &str, prefix: &str, max_samples: u64) {
    call_ingest_tool(
        store,
        p,
        ws,
        "series.retention.set",
        &json!({ "prefix": prefix, "raw_for_ms": 0, "max_samples": max_samples }),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_two_verbs_carry_separate_capabilities_in_both_directions() {
    let store = Store::memory().await.unwrap();
    seed_via_mcp(&store, "acme", 12).await;

    // A data-console client: stats YES, admin bookkeeping NO.
    let reader = principal("client:console", "acme", &[STATS]);
    let out = call_ingest_tool(
        &store,
        &reader,
        "acme",
        "series.stats",
        &json!({"series": "cpu"}),
    )
    .await
    .unwrap();
    assert_eq!(out["raw_count"].as_u64(), Some(12));
    let err = call_ingest_tool(
        &store,
        &reader,
        "acme",
        "series.retention.status",
        &json!({"series": "cpu"}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "stats grants nothing on the admin plane, got {err:?}"
    );

    // The REVERSE principal — this direction is what catches a gate wired to the wrong cap: a gate
    // that checked `series.stats` for BOTH verbs would pass the block above and fail here.
    let admin = principal("client:ops", "acme", &[STATUS]);
    let out = call_ingest_tool(
        &store,
        &admin,
        "acme",
        "series.retention.status",
        &json!({"series": "cpu"}),
    )
    .await
    .unwrap();
    assert_eq!(out["series"], json!("cpu"));
    let err = call_ingest_tool(
        &store,
        &admin,
        "acme",
        "series.stats",
        &json!({"series": "cpu"}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "retention.status grants nothing on the data plane, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_denial_is_not_an_empty_success() {
    // The failure this test exists to make impossible: a gate that returns an EMPTY result instead
    // of refusing. An empty `SeriesStats` and a refused call are both "no numbers" on screen, so a
    // UI cannot tell them apart — but they must be distinguishable at the TYPE level, because one
    // means "this series has no data" and the other means "you may not ask". `Ok(raw_count: 0)` vs
    // `Err(ToolError::Denied)` is exactly that distinction, asserted side by side below.
    let store = Store::memory().await.unwrap();
    seed_via_mcp(&store, "acme", 5).await;

    // GRANTED, on a series that genuinely holds nothing → Ok with zeroes and null extents.
    let granted = principal("client:console", "acme", &[STATS]);
    let out = call_ingest_tool(
        &store,
        &granted,
        "acme",
        "series.stats",
        &json!({"series": "never.written"}),
    )
    .await
    .expect("an empty series is a valid measurement, never an error");
    assert_eq!(out["raw_count"].as_u64(), Some(0));
    assert_eq!(out["first_ts"], json!(null), "no fabricated 1970 extent");
    assert_eq!(out["last_ts"], json!(null));
    assert_eq!(out["producers"].as_array().unwrap().len(), 0);

    // REFUSED, on the very same subject → Err. Same shape on screen, different type in the wire.
    let denied = principal("client:intruder", "acme", &["mcp:series.read:call"]);
    let err = call_ingest_tool(
        &store,
        &denied,
        "acme",
        "series.stats",
        &json!({"series": "never.written"}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "a refusal is an Err, never an Ok with zeroes, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_longest_matching_prefix_is_resolved_server_side() {
    let store = Store::memory().await.unwrap();
    let p = seed_via_mcp(&store, "acme", 3).await;
    set_policy(&store, &p, "acme", "a.", 100).await;
    set_policy(&store, &p, "acme", "a.b.", 200).await;

    // `a.b.c` matches BOTH rows; the LONGER one governs. Getting this wrong is silent — the UI would
    // confidently name the wrong governing prefix — which is why it is resolved here and not in each
    // client.
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.status",
        &json!({"series": "a.b.c"}),
    )
    .await
    .unwrap();
    assert_eq!(
        out["matched_prefix"],
        json!("a.b."),
        "the LONGER prefix wins"
    );
    assert_eq!(out["policy"]["prefix"], json!("a.b."));
    assert_eq!(out["policy"]["max_samples"].as_u64(), Some(200));
    assert_eq!(
        out["default_max_samples"].as_u64(),
        Some(lb_ingest::DEFAULT_MAX_SAMPLES),
        "the advisory cap is named, not hand-waved"
    );

    // A series matching NEITHER: no policy and no prefix — never a synthesized default row.
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.status",
        &json!({"series": "z.unmatched"}),
    )
    .await
    .unwrap();
    assert_eq!(out["policy"], json!(null), "no policy governs it");
    assert_eq!(
        out["matched_prefix"],
        json!(null),
        "and no prefix is fabricated to go with it"
    );

    // The SUBJECT may be a bare prefix, not just a series id — one verb serves the settings page
    // asking "what governs `a.`" and the detail page asking "what governs this series".
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.status",
        &json!({"series": "a."}),
    )
    .await
    .unwrap();
    assert_eq!(out["series"], json!("a."), "the subject is echoed back");
    assert_eq!(
        out["matched_prefix"],
        json!("a."),
        "a bare prefix resolves to its own row, not to the longer `a.b.`"
    );
    assert_eq!(out["policy"]["max_samples"].as_u64(), Some(100));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_on_demand_gc_verb_updates_the_same_record_the_reactor_would() {
    let store = Store::memory().await.unwrap();
    let p = seed_via_mcp(&store, "acme", 50).await;

    // Before any pass: honestly `null`, not a zero row.
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.status",
        &json!({"series": "cpu"}),
    )
    .await
    .unwrap();
    assert_eq!(
        out["last_pass"],
        json!(null),
        "no GC has run on this node yet"
    );

    // ONE PATH: `series.retention.gc` and the periodic reactor both go through `run_gc`, which owns
    // the record write — so an on-demand pass can never leave the status stale.
    call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.gc",
        &json!({ "now_ms": 424_242 }),
    )
    .await
    .unwrap();
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.status",
        &json!({"series": "cpu"}),
    )
    .await
    .unwrap();
    assert_eq!(
        out["last_pass"]["last_run_ms"].as_u64(),
        Some(424_242),
        "the status reports the `now_ms` the gc verb was called with: {out}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn observability_is_workspace_scoped_over_mcp() {
    let store = Store::memory().await.unwrap();
    let pa = seed_via_mcp(&store, "ws-a", 9).await;
    let pb = seed_via_mcp(&store, "ws-b", 4).await;

    call_ingest_tool(
        &store,
        &pa,
        "ws-a",
        "series.retention.gc",
        &json!({ "now_ms": 111_000 }),
    )
    .await
    .unwrap();
    call_ingest_tool(
        &store,
        &pb,
        "ws-b",
        "series.retention.gc",
        &json!({ "now_ms": 222_000 }),
    )
    .await
    .unwrap();

    for (p, ws, count, now) in [(&pa, "ws-a", 9u64, 111_000u64), (&pb, "ws-b", 4, 222_000)] {
        let out = call_ingest_tool(&store, p, ws, "series.stats", &json!({"series": "cpu"}))
            .await
            .unwrap();
        assert_eq!(
            out["raw_count"].as_u64(),
            Some(count),
            "{ws} counts its own rows"
        );
        let out = call_ingest_tool(
            &store,
            p,
            ws,
            "series.retention.status",
            &json!({"series": "cpu"}),
        )
        .await
        .unwrap();
        assert_eq!(
            out["last_pass"]["last_run_ms"].as_u64(),
            Some(now),
            "{ws} reports its OWN pass, never the other workspace's"
        );
    }

    // And a ws-B token cannot reach INTO ws-a for either verb (gate 1, workspace-first).
    for verb in ["series.stats", "series.retention.status"] {
        let err = call_ingest_tool(&store, &pb, "ws-a", verb, &json!({"series": "cpu"}))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{verb} must deny cross-ws"
        );
    }
}
