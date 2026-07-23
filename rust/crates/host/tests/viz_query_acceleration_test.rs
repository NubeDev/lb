//! Integration tests for the dashboard-query-acceleration scope (lb half: slices 1–3) — a REAL
//! embedded node (`mem://` store via `Node::boot`, real caps, real gateway dispatch through
//! `lb_host::call_tool`), no mocks and no fake cache. The cache is turned on exactly as the builder
//! does in production: `node.install_response_cache(Some(CacheConfig::default()))`.
//!
//! Requires the `page-cache` feature (the gateway cache is compiled out otherwise); run with
//! `cargo test -p lb-host --features page-cache --test viz_query_acceleration_test`.
//!
//! Panels here use `store.query` targets over the real `mem://` store (seeded through the real
//! `ingest.write` path) — the cache mechanisms under test (the `subject_scoped` gateway class, the
//! capability fingerprint, the quantiser, the batch fan-in, single-flight) are SOURCE-BLIND, so they
//! are exercised end to end without a federated engine. The federation-specific result-cache hit that
//! slice-1 wires a caller to is covered by the federation suite + the live UI walk (rubix-ai).
//!
//! Mandatory categories: capability-deny, workspace-isolation, the NEW cross-grant leak test
//! (mutation-checked), passthrough/bypass, quantiser, single-flight, batch partial-failure + cap, and
//! the perf assertion (a warm batched re-open runs ZERO resolver dispatches — the instrumented ≥10×).
#![cfg(feature = "page-cache")]

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, CacheConfig, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const VIZ: &str = "mcp:viz.query:call";
const STOREQ: &str = "mcp:store.query:call";
const WRITE: &str = "mcp:ingest.write:call";
const STATS: &str = "mcp:cache.stats:call";
const PURGE: &str = "mcp:cache.purge:call";

/// A principal in `ws` holding exactly `caps` — the real verified-token path.
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
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// The full read/write set a "privileged" caller holds (viz + its store target + seed + cache admin).
fn high_caps() -> Vec<&'static str> {
    vec![
        VIZ,
        STOREQ,
        WRITE,
        STATS,
        PURGE,
        "store:*:read",
        "store:*:write",
        "store:series:write",
        "store:series_meta:write",
    ]
}

/// Boot a real node with the cache ON (default budget/TTL), like production.
async fn boot_cached() -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    node.install_response_cache(Some(CacheConfig::default()));
    node
}

/// Seed `payloads` into `series` in `ws` through the real ingest write+drain path.
async fn seed(node: &Arc<Node>, p: &Principal, ws: &str, series: &str, payloads: &[f64]) {
    let samples: Vec<_> = payloads
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let seq = (i + 1) as u64;
            json!({ "series": series, "producer": "seed", "ts": seq, "seq": seq, "payload": v, "qos": "best-effort" })
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
    .expect("seed ingest");
}

/// A one-target `store.query` panel over the `series` table, filtered to one series name (the name is
/// a COLUMN, the table is `series`). Distinct names → distinct rows AND distinct cache keys.
fn panel(series: &str) -> Value {
    json!({
        "sources": [{
            "refId": "A",
            "tool": "store.query",
            "args": { "sql": format!("SELECT seq, payload FROM series WHERE series = '{series}' ORDER BY seq") }
        }]
    })
}

/// Call `viz.query` with a panel + optional `cache.ttl_s` + `now`; return the parsed `{frames, rows}`.
async fn viz(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    panel: Value,
    ttl_s: Option<u64>,
    now: u64,
) -> Result<Value, ToolError> {
    let mut input = json!({ "panel": panel, "now": now });
    if let Some(ttl) = ttl_s {
        input["cache"] = json!({ "ttl_s": ttl });
    }
    let out = call_tool(node, p, ws, "viz.query", &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// Call `viz.query_batch` with `panels`, returning the parsed `{results:[…]}`.
async fn viz_batch(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    panels: Vec<Value>,
    ttl_s: Option<u64>,
    now: u64,
) -> Result<Value, ToolError> {
    let mut input = json!({ "panels": panels, "now": now });
    if let Some(ttl) = ttl_s {
        input["cache"] = json!({ "ttl_s": ttl });
    }
    let out = call_tool(node, p, ws, "viz.query_batch", &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// The `(hits, misses)` for the `viz` (subject-scoped) class from `cache.stats`.
async fn viz_stats(node: &Arc<Node>, p: &Principal, ws: &str) -> (u64, u64) {
    let out = call_tool(node, p, ws, "cache.stats", "{}").await.unwrap();
    let s: Value = serde_json::from_str(&out).unwrap();
    let per_class = s["per_class"].as_array().cloned().unwrap_or_default();
    per_class
        .iter()
        .find(|c| c["class"] == json!("viz"))
        .map(|c| {
            (
                c["hits"].as_u64().unwrap_or(0),
                c["misses"].as_u64().unwrap_or(0),
            )
        })
        .unwrap_or((0, 0))
}

/// The primary-frame rows a viz result flattened to.
fn rows(v: &Value) -> Vec<Value> {
    v["rows"].as_array().cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// Slice 2 — subject-scoped gateway cache: warm hit, quantiser, single-flight, isolation, cross-grant
// ---------------------------------------------------------------------------------------------

/// Perf/de-dup: a warm re-open inside the bucket serves from the gateway cache with ZERO resolver
/// dispatch — one cold compute (miss), one warm hit, identical rows. The instrumented ≥10× floor.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn warm_reopen_runs_zero_resolver_dispatch() {
    let node = boot_cached().await;
    let p = principal("user:hi", "acme", &high_caps());
    seed(&node, &p, "acme", "temp", &[10.0, 20.0, 30.0]).await;

    let first = viz(&node, &p, "acme", panel("temp"), Some(60), 1_000)
        .await
        .unwrap();
    let second = viz(&node, &p, "acme", panel("temp"), Some(60), 1_010)
        .await
        .unwrap(); // same 60s bucket
    assert_eq!(rows(&first), rows(&second), "warm result equals cold");
    assert_eq!(rows(&first).len(), 3);

    let (hits, misses) = viz_stats(&node, &p, "acme").await;
    assert_eq!(misses, 1, "exactly one cold resolve");
    assert_eq!(
        hits, 1,
        "the re-open served from cache — zero resolver dispatch"
    );
}

/// Quantiser: two opens whose only difference is `now`, but inside ONE TTL bucket, share the key (one
/// compute + one hit). A `now` in the NEXT bucket is a fresh compute (a second miss).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quantiser_collapses_opens_within_a_bucket() {
    let node = boot_cached().await;
    let p = principal("user:hi", "acme", &high_caps());
    seed(&node, &p, "acme", "temp", &[1.0, 2.0]).await;

    // now=1000 and now=1010 both floor to the 960 bucket (60 s) → one key.
    viz(&node, &p, "acme", panel("temp"), Some(60), 1_000)
        .await
        .unwrap();
    viz(&node, &p, "acme", panel("temp"), Some(60), 1_010)
        .await
        .unwrap();
    let (hits, misses) = viz_stats(&node, &p, "acme").await;
    assert_eq!((hits, misses), (1, 1), "same bucket → one compute, one hit");

    // now=1100 floors to a DIFFERENT bucket (1080) → a fresh compute.
    viz(&node, &p, "acme", panel("temp"), Some(60), 1_100)
        .await
        .unwrap();
    let (_, misses2) = viz_stats(&node, &p, "acme").await;
    assert_eq!(misses2, 2, "the next bucket resolves fresh");
}

/// Single-flight: N concurrent identical COLD `viz.query`s collapse to ONE resolve; all identical.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_flight_collapses_concurrent_cold_opens() {
    let node = boot_cached().await;
    let p = Arc::new(principal("user:hi", "acme", &high_caps()));
    seed(&node, &p, "acme", "temp", &[7.0]).await;

    let mut handles = Vec::new();
    for _ in 0..12 {
        let (n, pr) = (node.clone(), p.clone());
        handles.push(tokio::spawn(async move {
            viz(&n, &pr, "acme", panel("temp"), Some(60), 1_000)
                .await
                .unwrap()
        }));
    }
    let mut outs = Vec::new();
    for h in handles {
        outs.push(h.await.unwrap());
    }
    assert!(
        outs.windows(2).all(|w| rows(&w[0]) == rows(&w[1])),
        "all coalesced callers agree"
    );
    let (_, misses) = viz_stats(&node, &p, "acme").await;
    assert_eq!(
        misses, 1,
        "12 concurrent cold opens → ONE resolve (single-flight)"
    );
}

/// Workspace isolation: the same board in ws A and ws B (different seeded data) never crosses. Priming
/// A serves A's rows; B resolves its own. `cache.purge` A leaves B serving.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_isolation_and_purge_scoping() {
    let node = boot_cached().await;
    let a = principal("user:a", "ws-a", &high_caps());
    let b = principal("user:b", "ws-b", &high_caps());
    seed(&node, &a, "ws-a", "temp", &[1.0, 2.0]).await; // A: 2 rows
    seed(&node, &b, "ws-b", "temp", &[9.0]).await; // B: 1 row

    let ra = viz(&node, &a, "ws-a", panel("temp"), Some(60), 1_000)
        .await
        .unwrap();
    let rb = viz(&node, &b, "ws-b", panel("temp"), Some(60), 1_000)
        .await
        .unwrap();
    assert_eq!(rows(&ra).len(), 2, "A sees A's data");
    assert_eq!(rows(&rb).len(), 1, "B sees B's data — no cross-ws bleed");

    // Purge A; B's warm entry is untouched (a hit), A re-resolves (a fresh miss).
    call_tool(&node, &a, "ws-a", "cache.purge", "{}")
        .await
        .unwrap();
    let rb2 = viz(&node, &b, "ws-b", panel("temp"), Some(60), 1_000)
        .await
        .unwrap();
    assert_eq!(rows(&rb2).len(), 1);
    let (hits_b, _) = viz_stats(&node, &b, "ws-b").await;
    assert!(hits_b >= 1, "purging A left B serving from cache");
}

/// THE cross-grant leak test (mutation-checked): a caller WITH the target cap primes a warm frame of
/// REAL rows; a co-workspace caller WITHOUT the target cap (but holding `viz.query`) must NEVER receive
/// that warm frame — its capability fingerprint differs, so it misses, resolves, and gets a denied,
/// EMPTY frame. Disabling the fingerprint fold would turn this red (a leak).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_grant_caller_never_receives_warm_privileged_frame() {
    let node = boot_cached().await;
    let ws = "acme";
    let high = principal("user:hi", ws, &high_caps());
    // LOW holds viz.query (+ stats to observe) but NOT mcp:store.query:call → the target is denied.
    let low = principal("user:lo", ws, &[VIZ, STATS]);
    seed(&node, &high, ws, "temp", &[11.0, 22.0, 33.0]).await;

    // Prime a warm frame of real rows under HIGH's fingerprint.
    let hi_out = viz(&node, &high, ws, panel("temp"), Some(60), 1_000)
        .await
        .unwrap();
    assert_eq!(
        rows(&hi_out).len(),
        3,
        "privileged caller sees the real rows"
    );

    // LOW opens the SAME board in the SAME bucket. Different fingerprint → miss → its own resolve →
    // the store.query target is DENIED → an honest EMPTY frame. It must not be HIGH's warm rows.
    let lo_out = viz(&node, &low, ws, panel("temp"), Some(60), 1_010)
        .await
        .unwrap();
    assert!(
        rows(&lo_out).is_empty(),
        "low-grant caller gets an empty (denied) frame — NO leak"
    );
    assert_ne!(
        rows(&lo_out),
        rows(&hi_out),
        "the warm privileged frame never crossed the grant wall"
    );
    // And the denied target is opaque (no leaked message on its frame status).
    let status = &lo_out["frames"][0]["status"];
    assert_eq!(
        status["state"],
        json!("denied"),
        "denied target stays opaque"
    );
}

/// Capability-deny: `viz.query_batch` without `mcp:viz.query:call` is opaquely `Denied` (same as the
/// single verb) — the batch grants no new privilege.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_denied_without_viz_cap() {
    let node = boot_cached().await;
    let capless = principal("user:none", "acme", &[STATS]);
    let err = viz_batch(
        &node,
        &capless,
        "acme",
        vec![panel("temp")],
        Some(60),
        1_000,
    )
    .await;
    assert!(
        matches!(err, Err(ToolError::Denied)),
        "no viz cap → opaque Denied, got {err:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Slice 1 — passthrough / bypass
// ---------------------------------------------------------------------------------------------

/// Bypass parity: a top-level `cache` directive is SOURCE-BLIND — a `store.query` target ignores the
/// unknown field, so rows are identical with/without it; and `ttl_s:0` (and absent) bypass the gateway
/// cache entirely (every open resolves fresh — no hit accrues).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn directive_is_source_blind_and_zero_ttl_bypasses() {
    let node = boot_cached().await;
    let p = principal("user:hi", "acme", &high_caps());
    seed(&node, &p, "acme", "temp", &[3.0, 4.0]).await;

    let without = viz(&node, &p, "acme", panel("temp"), None, 1_000)
        .await
        .unwrap();
    let with_zero = viz(&node, &p, "acme", panel("temp"), Some(0), 1_000)
        .await
        .unwrap();
    let with_ttl = viz(&node, &p, "acme", panel("temp"), Some(60), 1_000)
        .await
        .unwrap();
    assert_eq!(
        rows(&without),
        rows(&with_ttl),
        "the directive never changes the rows"
    );
    assert_eq!(
        rows(&without),
        rows(&with_zero),
        "ttl_s:0 is a harmless no-op on the rows"
    );

    // Two opens with ttl_s:0 and two with absent → the gateway cache never populated (live bypass).
    viz(&node, &p, "acme", panel("temp"), Some(0), 1_000)
        .await
        .unwrap();
    viz(&node, &p, "acme", panel("temp"), None, 1_000)
        .await
        .unwrap();
    let (hits, _) = viz_stats(&node, &p, "acme").await;
    assert_eq!(hits, 0, "ttl_s:0 / absent never serve a warm gateway frame");
}

// ---------------------------------------------------------------------------------------------
// Slice 3 — batch fan-in: perf, parity, partial failure, cap
// ---------------------------------------------------------------------------------------------

/// Perf assertion (the instrumented ≥10× / "1 batch not N"): ONE `viz.query_batch` resolves N distinct
/// panels (N cold resolves); a warm re-open of the SAME batch runs ZERO resolves (N hits, misses
/// unchanged) — the whole page from memory.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn batch_warm_reopen_runs_zero_db_queries() {
    let node = boot_cached().await;
    let p = principal("user:hi", "acme", &high_caps());
    let n = 8u64;
    for i in 0..n {
        seed(&node, &p, "acme", &format!("s{i}"), &[i as f64 + 1.0]).await;
    }
    let panels: Vec<Value> = (0..n).map(|i| panel(&format!("s{i}"))).collect();

    // Cold: one batch call, N distinct resolves.
    let cold = viz_batch(&node, &p, "acme", panels.clone(), Some(60), 1_000)
        .await
        .unwrap();
    assert_eq!(
        cold["results"].as_array().unwrap().len(),
        n as usize,
        "one call resolved N panels"
    );
    let (_, cold_misses) = viz_stats(&node, &p, "acme").await;
    assert_eq!(
        cold_misses, n,
        "cold batch = N resolves (one concurrent wave)"
    );

    // Warm: the same batch, same bucket → every panel a hit, ZERO new resolves.
    let warm = viz_batch(&node, &p, "acme", panels, Some(60), 1_010)
        .await
        .unwrap();
    let (warm_hits, warm_misses) = viz_stats(&node, &p, "acme").await;
    assert_eq!(
        warm_misses, n,
        "warm re-open added ZERO resolver dispatches"
    );
    assert_eq!(
        warm_hits, n,
        "warm re-open served all N from the gateway cache"
    );
    // Warm rows equal cold rows, panel for panel.
    for i in 0..n as usize {
        assert_eq!(
            cold["results"][i]["rows"], warm["results"][i]["rows"],
            "panel {i} warm==cold"
        );
    }
}

/// Batch parity: a batch result for a panel equals the single `viz.query` result for that same panel.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_result_equals_single_query_per_panel() {
    let node = boot_cached().await;
    let p = principal("user:hi", "acme", &high_caps());
    seed(&node, &p, "acme", "a", &[1.0, 2.0]).await;
    seed(&node, &p, "acme", "b", &[3.0]).await;

    let single_a = viz(&node, &p, "acme", panel("a"), None, 1_000)
        .await
        .unwrap();
    let single_b = viz(&node, &p, "acme", panel("b"), None, 1_000)
        .await
        .unwrap();
    let batch = viz_batch(&node, &p, "acme", vec![panel("a"), panel("b")], None, 1_000)
        .await
        .unwrap();
    assert_eq!(
        batch["results"][0]["rows"], single_a["rows"],
        "panel A parity"
    );
    assert_eq!(
        batch["results"][1]["rows"], single_b["rows"],
        "panel B parity"
    );
}

/// Per-item partial failure: one bad-SQL panel returns its own `{status:"error"}`; the siblings still
/// resolve. The board never blanks on one bad tile.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_partial_failure_isolates_the_bad_panel() {
    let node = boot_cached().await;
    let p = principal("user:hi", "acme", &high_caps());
    seed(&node, &p, "acme", "ok", &[5.0, 6.0]).await;

    // A syntactically invalid SELECT → the store's parse error → `BadInput` → an `error` frame.
    let bad = json!({ "sources": [{ "refId": "A", "tool": "store.query", "args": { "sql": "SELECT payload FROM" } }] });
    let batch = viz_batch(&node, &p, "acme", vec![panel("ok"), bad], None, 1_000)
        .await
        .unwrap();
    let results = batch["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    // The good tile resolved…
    assert_eq!(
        results[0]["rows"].as_array().unwrap().len(),
        2,
        "good panel resolved"
    );
    // …and the bad tile carries a per-item error/empty frame WITHOUT failing the batch. A bad SQL
    // surfaces on the frame's own status (query-diagnostics), never as a whole-batch failure.
    let bad_frame_status = &results[1]["frames"][0]["status"];
    assert_eq!(
        bad_frame_status["state"],
        json!("error"),
        "bad panel isolated as an error frame"
    );
}

/// The hard 64-panel cap: an over-cap batch is a clean `BadInput` (the UI chunks) — never a silent
/// truncation.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_over_cap_is_bad_input() {
    let node = boot_cached().await;
    let p = principal("user:hi", "acme", &high_caps());
    let panels: Vec<Value> = (0..65).map(|_| panel("temp")).collect();
    let err = viz_batch(&node, &p, "acme", panels, Some(60), 1_000).await;
    assert!(
        matches!(err, Err(ToolError::BadInput(_))),
        "65 panels → BadInput, got {err:?}"
    );
}
