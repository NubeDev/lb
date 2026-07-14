//! The series-plane readiness slices at the MCP surface: paged + bucketed `series.read` and the
//! `series.retention.*` admin verbs through `call_ingest_tool`, with the MANDATORY capability-deny
//! and workspace-isolation tests (a cursor is a bookmark, never a grant; a ws-B token replaying a
//! ws-A cursor sees nothing).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::call_ingest_tool;
use lb_mcp::ToolError;
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
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

fn sample(series: &str, seq: u64, payload: serde_json::Value) -> serde_json::Value {
    json!({ "series": series, "producer": "ignored", "ts": seq * 1000, "seq": seq,
            "payload": payload, "qos": "best-effort" })
}

async fn seed_via_mcp(store: &Store, ws: &str, n: u64) -> Principal {
    let p = principal(
        "client:pi-7",
        ws,
        &[
            "mcp:ingest.write:call",
            "mcp:series.read:call",
            "mcp:series.retention.set:call",
            "mcp:series.retention.list:call",
            "mcp:series.retention.gc:call",
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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn paged_read_walks_chain_via_mcp() {
    let store = Store::memory().await.unwrap();
    let p = seed_via_mcp(&store, "acme", 25).await;

    let mut total = 0;
    let mut cursor: Option<String> = None;
    loop {
        let mut input = json!({ "series": "cpu", "limit": 10 });
        if let Some(c) = &cursor {
            input["cursor"] = json!(c);
        }
        let out = call_ingest_tool(&store, &p, "acme", "series.read", &input)
            .await
            .unwrap();
        total += out["samples"].as_array().unwrap().len();
        match out["next_cursor"].as_str() {
            Some(c) => cursor = Some(c.to_string()),
            None => break,
        }
    }
    assert_eq!(total, 25, "the chain returns every sample exactly once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn windowed_read_is_half_open_via_mcp() {
    let store = Store::memory().await.unwrap();
    // seed_via_mcp stamps ts = seq * 1000, seq 1..=10 -> ts 1000..=10000.
    let p = seed_via_mcp(&store, "acme", 10).await;

    // [3000, 7000): seq 3,4,5,6 (seq 7 has ts 7000, excluded — `to` is exclusive).
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "cpu", "from": 3000, "to": 7000 }),
    )
    .await
    .unwrap();
    let rows = out["samples"].as_array().unwrap();
    let seqs: Vec<u64> = rows.iter().map(|r| r["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, vec![3, 4, 5, 6], "from is inclusive, to is exclusive");

    // Row is the full canonical Sample envelope, not a {ts, value} projection.
    assert_eq!(rows[0]["payload"], json!(3.0), "value field is `payload`");
    assert_eq!(rows[0]["ts"], json!(3000), "ts is epoch ms, not a datetime string");
    assert!(rows[0].get("producer").is_some() && rows[0].get("seq").is_some());

    // from_seq/to_seq are inclusive on BOTH ends (contrast with the half-open wall-clock window).
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "cpu", "from_seq": 3, "to_seq": 6 }),
    )
    .await
    .unwrap();
    let seqs: Vec<u64> = out["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(seqs, vec![3, 4, 5, 6], "from_seq/to_seq bound inclusively");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bucketed_read_via_mcp_and_deny_without_cap() {
    let store = Store::memory().await.unwrap();
    let p = seed_via_mcp(&store, "acme", 60).await;

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "cpu", "mode": "buckets", "from": 0, "to": 61_000, "budget": 6 }),
    )
    .await
    .unwrap();
    let buckets = out["buckets"].as_array().unwrap();
    assert!(
        buckets.len() <= 6,
        "budget is a ceiling, got {}",
        buckets.len()
    );
    assert!(buckets[0].get("min").is_some() && buckets[0].get("max").is_some());
    assert!(buckets[0].get("avg").is_some() && buckets[0].get("last").is_some());

    // MANDATORY deny: no `mcp:series.read:call` → opaque denial, for BOTH modes.
    let no_cap = principal("client:intruder", "acme", &["mcp:series.latest:call"]);
    for input in [
        json!({ "series": "cpu" }),
        json!({ "series": "cpu", "from": 0, "to": 1000 }),
        json!({ "series": "cpu", "mode": "buckets", "from": 0, "to": 1000, "budget": 1 }),
    ] {
        let err = call_ingest_tool(&store, &no_cap, "acme", "series.read", &input)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "expected Denied, got {err:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_replaying_ws_a_cursor_sees_nothing() {
    let store = Store::memory().await.unwrap();
    let pa = seed_via_mcp(&store, "ws-a", 20).await;

    let out = call_ingest_tool(
        &store,
        &pa,
        "ws-a",
        "series.read",
        &json!({ "series": "cpu", "limit": 5 }),
    )
    .await
    .unwrap();
    let stolen = out["next_cursor"].as_str().unwrap().to_string();

    // A ws-B principal WITH the read cap replays ws-A's cursor: the seek runs in ws-B's namespace
    // and resolves nothing — the hard wall holds; the cursor is a bookmark, not a key.
    let pb = principal("client:b", "ws-b", &["mcp:series.read:call"]);
    let out = call_ingest_tool(
        &store,
        &pb,
        "ws-b",
        "series.read",
        &json!({ "series": "cpu", "cursor": stolen }),
    )
    .await
    .unwrap();
    assert_eq!(out["samples"].as_array().unwrap().len(), 0);

    // And a ws-B token can never call INTO ws-a at all (gate 1, workspace-first).
    let err = call_ingest_tool(
        &store,
        &pb,
        "ws-a",
        "series.read",
        &json!({ "series": "cpu" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retention_round_trip_and_deny_via_mcp() {
    let store = Store::memory().await.unwrap();
    let p = seed_via_mcp(&store, "acme", 200).await;

    // set → list → gc, all over the MCP bridge.
    call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.set",
        &json!({ "prefix": "cpu", "raw_for_ms": 100_000,
                 "tiers": [{ "width_ms": 10_000, "keep_for_ms": 0 }] }),
    )
    .await
    .unwrap();
    let out = call_ingest_tool(&store, &p, "acme", "series.retention.list", &json!({}))
        .await
        .unwrap();
    assert_eq!(out["policies"].as_array().unwrap().len(), 1);

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.retention.gc",
        &json!({ "now_ms": 200_000 }),
    )
    .await
    .unwrap();
    assert!(
        out["evicted_raw"].as_u64().unwrap() > 0,
        "gc evicted raw history: {out}"
    );
    assert!(
        out["rollup_rows"].as_u64().unwrap() > 0,
        "gc stored rollup tiers: {out}"
    );

    // MANDATORY deny: retention admin without the caps is refused, opaquely — for every verb.
    let no_cap = principal("client:intruder", "acme", &["mcp:series.read:call"]);
    for (verb, input) in [
        (
            "series.retention.set",
            json!({ "prefix": "cpu", "raw_for_ms": 1 }),
        ),
        ("series.retention.list", json!({})),
        ("series.retention.delete", json!({ "prefix": "cpu" })),
        ("series.retention.gc", json!({ "now_ms": 1 })),
    ] {
        let err = call_ingest_tool(&store, &no_cap, "acme", verb, &input)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Denied), "{verb} must deny");
    }

    // ISOLATION: ws-B cannot see or run ws-A's policies.
    let pb = principal(
        "client:b",
        "ws-b",
        &[
            "mcp:series.retention.list:call",
            "mcp:series.retention.gc:call",
        ],
    );
    let out = call_ingest_tool(&store, &pb, "ws-b", "series.retention.list", &json!({}))
        .await
        .unwrap();
    assert_eq!(
        out["policies"].as_array().unwrap().len(),
        0,
        "policies are ws-scoped"
    );
    let out = call_ingest_tool(
        &store,
        &pb,
        "ws-b",
        "series.retention.gc",
        &json!({ "now_ms": 200_000 }),
    )
    .await
    .unwrap();
    assert_eq!(
        out["evicted_raw"].as_u64(),
        Some(0),
        "a ws-B gc touches nothing of ws-A"
    );
}
