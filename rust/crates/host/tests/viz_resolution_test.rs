//! Panel-resolution negotiation end to end through `viz.query` (viz panel-resolution scope, issue
//! #101, slice 1). A mode-less `series.read` chart target is upgraded IN THE RESOLVER to the shipped
//! `mode:"buckets"` decimation path — the width derived from `(range, budget, minInterval)` — so a
//! wide window returns ≤ budget spike-safe buckets instead of raw rows capped at 10k.
//!
//! NO mocks (CLAUDE §9): a real embedded SurrealDB seeded through the real `ingest.write` path, the
//! real capability gate, the real `series.read` bucket engine. What each test proves:
//!   - `wide_window_returns_bounded_buckets_not_raw_rows` — a seeded series over a wide window returns
//!     ≤ budget buckets through `viz.query` (was: raw rows capped); O(buckets) scaling to 1M is proven
//!     in `lb_ingest`'s decimation tests — this layer proves the injection + the bounds.
//!   - `spike_survives_in_bucket_max_at_the_dashboard_layer` — a seeded in-bucket spike shows in a
//!     bucket `max` while `avg` stays low (the decimation headline, re-asserted at the viz layer).
//!   - `explicit_rows_mode_still_returns_raw_rows` — an explicit `mode:"rows"` target is left alone.
//!   - `derivation_is_deterministic_and_zoom_refines` — same range/budget → byte-identical buckets
//!     (cache-key stability); a zoom to a narrower range → finer buckets (more, smaller).
//!   - `bucket_injection_denied_without_series_cap` — MANDATORY: a viewer without `mcp:series.read:call`
//!     gets an honest empty frame for the injected bucket target (no render-path bypass).
//!   - `bucket_injection_workspace_isolation` — MANDATORY: a ws-B panel resolves NO buckets from a
//!     ws-A series, regardless of the injected mode. The hard wall, from the token.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use serde_json::{json, Value};
use std::sync::Arc;

const VIZ: &str = "mcp:viz.query:call";
const READ: &str = "mcp:series.read:call";
const WRITE: &str = "mcp:ingest.write:call";

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// Seed `n` real samples into `series` (ts = `i*1000` epoch ms, i in `1..=n`) through the real ingest
/// write+drain path, in chunks so each write drains its own batch. `spike_at`/`spike_val` inject one
/// out-of-band value at that index (1-based) so a decimation test can prove it survives in `max`.
async fn seed_series(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    series: &str,
    n: u64,
    spike_at: Option<(u64, f64)>,
) {
    const CHUNK: u64 = 20_000;
    let mut i = 1u64;
    while i <= n {
        let end = (i + CHUNK - 1).min(n);
        let samples: Vec<Value> = (i..=end)
            .map(|seq| {
                let v = match spike_at {
                    Some((at, sv)) if at == seq => sv,
                    _ => 20.0,
                };
                json!({ "series": series, "producer": "seed", "ts": seq * 1000, "seq": seq,
                        "payload": v, "qos": "best-effort" })
            })
            .collect();
        call_tool(
            node,
            p,
            ws,
            "ingest.write",
            &json!({ "samples": samples }).to_string(),
        )
        .await
        .expect("seed ingest.write");
        i = end + 1;
    }
}

/// A one-target series panel with the given args + a default budget (no queryOptions unless supplied).
fn series_panel(args: Value, query_options: Value) -> Value {
    json!({
        "sources": [{ "refId": "A", "tool": "series.read", "args": args }],
        "queryOptions": query_options,
    })
}

async fn viz_rows(node: &Arc<Node>, p: &Principal, ws: &str, panel: Value) -> Vec<Value> {
    let out = call_tool(
        node,
        p,
        ws,
        "viz.query",
        &json!({ "panel": panel, "now": 1 }).to_string(),
    )
    .await
    .expect("viz.query runs");
    let out: Value = serde_json::from_str(&out).expect("json");
    out["rows"].as_array().cloned().unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn wide_window_returns_bounded_buckets_not_raw_rows() {
    let ws = "viz-res-budget";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, READ, WRITE]);
    // 2k samples over ~33min (ts 1_000 .. 2_000_000) at a default budget of 1000 → ~400 buckets:
    // enough to prove the injection decimates (≪ budget buckets, never the raw rows) and stays fast
    // against mem-SurrealDB. The O(buckets) scaling to 1M is proven in `lb_ingest`'s decimation tests.
    seed_series(&node, &p, ws, "cpu", 2_000, None).await;

    // Mode-less series.read + a wide window + default budget (1000) → bucket injection.
    let rows = viz_rows(
        &node,
        &p,
        ws,
        series_panel(
            json!({ "series": "cpu", "from": 0u64, "to": 2_000_001u64 }),
            json!({}),
        ),
    )
    .await;

    // Decimated, not the raw 2k (and not the 10k frame cap): ≤ budget buckets, but clearly bucketed.
    assert!(!rows.is_empty(), "some buckets returned");
    assert!(
        rows.len() <= 1_000,
        "budget is a ceiling: got {} buckets > 1000",
        rows.len()
    );
    assert!(
        rows.len() < 2_000,
        "the read decimated — not raw rows ({} rows)",
        rows.len()
    );
    // The bucket record shape survived to the frame rows.
    assert!(
        rows[0].get("t").is_some() && rows[0].get("max").is_some() && rows[0].get("avg").is_some(),
        "bucket record shape {{t,min,max,avg,last,count}}: {}",
        rows[0]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn spike_survives_in_bucket_max_at_the_dashboard_layer() {
    let ws = "viz-res-spike";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, READ, WRITE]);
    // Baseline 20.0 everywhere; a single 200.0 spike at seq 1_000 (ts 1_000_000).
    seed_series(&node, &p, ws, "cpu", 2_000, Some((1_000, 200.0))).await;

    let rows = viz_rows(
        &node,
        &p,
        ws,
        series_panel(
            json!({ "series": "cpu", "from": 0u64, "to": 2_000_001u64 }),
            json!({}),
        ),
    )
    .await;

    // The bucket covering ts=1_000_000 carries max=200 even though its avg is ~20 — the spike an
    // avg-only decimation would have hidden is on screen.
    let spike_bucket = rows
        .iter()
        .find(|r| r["max"].as_f64() == Some(200.0))
        .expect("a bucket carries the spike in its max");
    let avg = spike_bucket["avg"].as_f64().expect("avg present");
    assert!(
        avg < 100.0,
        "avg ({avg}) is far below the spike — proving avg alone would have hidden the 200 peak"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn explicit_rows_mode_still_returns_raw_rows() {
    let ws = "viz-res-rows";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, READ, WRITE]);
    seed_series(&node, &p, ws, "cpu", 50, None).await;

    // Explicit mode:"rows" always wins — tables/exports/the raw inspector keep raw rows.
    let rows = viz_rows(
        &node,
        &p,
        ws,
        series_panel(
            json!({ "series": "cpu", "mode": "rows", "from": 0u64, "to": 100_000u64 }),
            json!({}),
        ),
    )
    .await;
    assert_eq!(
        rows.len(),
        50,
        "explicit rows mode returns every raw sample"
    );
    // Raw samples, not buckets — they carry a `payload`, not a bucket `max`.
    assert!(
        rows[0].get("max").is_none(),
        "raw rows, not buckets: {}",
        rows[0]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn derivation_is_deterministic_and_zoom_refines() {
    let ws = "viz-res-zoom";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, READ, WRITE]);
    seed_series(&node, &p, ws, "cpu", 2_000, None).await;

    let wide = json!({ "series": "cpu", "from": 0u64, "to": 2_000_001u64 });
    // Cache-key stability: two identical calls derive byte-identical args → identical bucket frames.
    let a = viz_rows(&node, &p, ws, series_panel(wide.clone(), json!({}))).await;
    let b = viz_rows(&node, &p, ws, series_panel(wide.clone(), json!({}))).await;
    assert_eq!(
        a, b,
        "same range/budget → identical buckets (single-flight collapses them)"
    );

    // Zoom: a narrower window derives a FINER width → more, smaller buckets over the visible span.
    let zoomed = viz_rows(
        &node,
        &p,
        ws,
        series_panel(
            json!({ "series": "cpu", "from": 800_000u64, "to": 1_200_000u64 }),
            json!({}),
        ),
    )
    .await;
    assert!(!zoomed.is_empty(), "zoom returns buckets");
    // The zoomed window is 1/5 the span; at the same budget its buckets are finer. Assert the width
    // strictly decreased (adjacent bucket spacing is smaller than the wide view's).
    let width_of = |rows: &[Value]| -> u64 {
        let ts: Vec<u64> = rows.iter().filter_map(|r| r["t"].as_u64()).collect();
        ts.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(u64::MAX)
    };
    assert!(
        width_of(&zoomed) < width_of(&a),
        "zoom refined the resolution: zoomed width {} < wide width {}",
        width_of(&zoomed),
        width_of(&a)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bucket_injection_denied_without_series_cap() {
    let ws = "viz-res-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // A writer seeds (holds READ+WRITE); the caller holds viz.query but NOT series.read.
    let writer = principal("user:seed", ws, &[READ, WRITE]);
    seed_series(&node, &writer, ws, "cpu", 100, None).await;

    let caller = principal("user:ada", ws, &[VIZ]); // no series.read
    let rows = viz_rows(
        &node,
        &caller,
        ws,
        series_panel(
            json!({ "series": "cpu", "from": 0u64, "to": 200_000u64 }),
            json!({}),
        ),
    )
    .await;
    assert!(
        rows.is_empty(),
        "a denied bucket target → honest empty frame, no bypass ({} rows)",
        rows.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bucket_injection_workspace_isolation() {
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal("user:ada", "ws-a", &[VIZ, READ, WRITE]);
    seed_series(&node, &a, "ws-a", "cpu", 200, None).await;

    let panel = series_panel(
        json!({ "series": "cpu", "from": 0u64, "to": 300_000u64 }),
        json!({}),
    );

    // Sanity: ws-A sees its own series decimated to buckets.
    let rows_a = viz_rows(&node, &a, "ws-a", panel.clone()).await;
    assert!(!rows_a.is_empty(), "ws-A sees its own buckets");

    // ws-B runs the SAME panel — the bucket read is workspace-first → nothing from ws-A.
    let b = principal("user:bob", "ws-b", &[VIZ, READ, WRITE]);
    let rows_b = viz_rows(&node, &b, "ws-b", panel).await;
    assert!(
        rows_b.is_empty(),
        "ws-B resolves NO buckets from ws-A's series ({} rows)",
        rows_b.len()
    );
}
