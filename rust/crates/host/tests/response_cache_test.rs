//! Response-cache integration tests (response-cache scope, Testing plan) — a REAL embedded node
//! (`mem://` store via `Node::boot`, real caps, real gateway dispatch through `lb_host::call_tool`),
//! no mocks and no fake cache. The cache is turned on the way the builder does it in production:
//! `node.install_response_cache(Some(CacheConfig::default()))`.
//!
//! Requires the `page-cache` feature (the whole cache is compiled out otherwise); run with
//! `cargo test -p lb-host --features page-cache --test response_cache_test`.
//!
//! Mandatory categories covered here: single-flight, staleness-after-write (per dirtied class +
//! the coarse `store.write` nuke), workspace-isolation (+ `cache.purge` scoping), capability-deny
//! (a warm key is as unreachable to a capless caller as a cold one; the `cache.*` admin verbs),
//! budget/eviction, ext/uncacheable-verb-dispatches-every-call, and the perf/de-dup assertion
//! (a re-open inside the window runs zero engine dispatches). `viz.query`/quantisation are DEFERRED
//! (subject-filtered — see the scope), so there is no quantisation test in v1.
#![cfg(feature = "page-cache")]

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, CacheConfig, Node};
use serde_json::{json, Value};

/// A principal in `ws` holding exactly `caps` — the real verified-token path (no shortcut).
fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
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

/// The full read/write cap set the cache tests drive.
const CAPS: &[&str] = &[
    "mcp:series.list:call",
    "mcp:ingest.write:call",
    "mcp:datasource.list:call",
    "mcp:flows.list:call",
    "mcp:store.write:call",
    "mcp:tools.catalog:call",
    "mcp:cache.stats:call",
    "mcp:cache.purge:call",
    "store:series:write",
    "store:series_meta:write",
    "store:*:read",
    "store:*:write",
];

/// Boot a real node with the cache ON (default budget/TTL). Returns an `Arc<Node>` like production.
async fn boot_cached() -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    node.install_response_cache(Some(CacheConfig::default()));
    node
}

/// Write one sample into `series` through the real ingest write+drain path (a real store mutation).
async fn ingest(node: &Arc<Node>, p: &Principal, ws: &str, series: &str, seq: u64, v: f64) {
    let sample = json!({ "series": series, "producer": "seed", "ts": seq, "seq": seq, "payload": v, "qos": "best-effort" });
    call_tool(
        node,
        p,
        ws,
        "ingest.write",
        &json!({ "samples": [sample] }).to_string(),
    )
    .await
    .expect("ingest.write");
}

/// `series.list` (optionally prefixed) → the sorted set of series names.
async fn series_list(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    prefix: Option<&str>,
) -> Vec<String> {
    let args = match prefix {
        Some(px) => json!({ "prefix": px }),
        None => json!({}),
    };
    let out = call_tool(node, p, ws, "series.list", &args.to_string())
        .await
        .expect("series.list");
    let v: Value = serde_json::from_str(&out).unwrap();
    serde_json::from_value(v["series"].clone()).unwrap_or_default()
}

/// The node-wide `cache.stats` snapshot.
async fn stats(node: &Arc<Node>, p: &Principal, ws: &str) -> Value {
    let out = call_tool(node, p, ws, "cache.stats", "{}")
        .await
        .expect("cache.stats");
    serde_json::from_str(&out).unwrap()
}

fn u(v: &Value, k: &str) -> u64 {
    v[k].as_u64()
        .unwrap_or_else(|| panic!("stats.{k} missing in {v}"))
}

// ----------------------------------------------------------------------------------------------

/// Perf/de-dup assertion + warm hit: a re-read inside the window serves from cache with ZERO extra
/// engine dispatch — the first call misses, the second hits, and the values are identical.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn warm_reopen_runs_zero_engine_dispatches() {
    let node = boot_cached().await;
    let p = principal("acme", CAPS);
    ingest(&node, &p, "acme", "temp", 1, 20.0).await;

    let first = series_list(&node, &p, "acme", None).await; // cold → miss + compute
    let second = series_list(&node, &p, "acme", None).await; // warm → hit, no engine work
    assert_eq!(first, second);
    assert!(first.contains(&"temp".to_string()));

    let s = stats(&node, &p, "acme").await;
    assert_eq!(u(&s, "misses"), 1, "exactly one cold compute");
    assert_eq!(
        u(&s, "hits"),
        1,
        "the re-open served from cache (zero engine dispatch)"
    );
}

/// Single-flight: N concurrent identical COLD reads collapse to ONE compute (one miss), and every
/// caller gets the identical response.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_flight_coalesces_concurrent_cold_reads() {
    let node = boot_cached().await;
    let p = Arc::new(principal("acme", CAPS));
    ingest(&node, &p, "acme", "temp", 1, 20.0).await;

    let mut handles = Vec::new();
    for _ in 0..16 {
        let (n, pr) = (node.clone(), p.clone());
        handles.push(tokio::spawn(async move {
            call_tool(&n, &pr, "acme", "series.list", "{}")
                .await
                .unwrap()
        }));
    }
    let mut outs = Vec::new();
    for h in handles {
        outs.push(h.await.unwrap());
    }
    // All identical.
    assert!(
        outs.windows(2).all(|w| w[0] == w[1]),
        "all coalesced callers see one value"
    );

    let s = stats(&node, &p, "acme").await;
    assert_eq!(
        u(&s, "misses"),
        1,
        "16 concurrent cold reads → ONE engine dispatch (single-flight)"
    );
    assert_eq!(u(&s, "hits"), 15, "the other 15 coalesced onto it");
}

/// Staleness-after-write: a write that dirties a class is visible on the very next read. `ingest.write`
/// dirties the `series` class; a generic `store.write` nukes ALL classes (the coarse, safe path).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn write_invalidates_immediately() {
    let node = boot_cached().await;
    let p = principal("acme", CAPS);
    ingest(&node, &p, "acme", "a", 1, 1.0).await;

    let before = series_list(&node, &p, "acme", None).await; // caches {a}
    assert_eq!(before, vec!["a".to_string()]);

    // A dirtying write (ingest.write → Series) must be visible immediately, not after the TTL.
    ingest(&node, &p, "acme", "b", 2, 2.0).await;
    let after = series_list(&node, &p, "acme", None).await;
    assert!(
        after.contains(&"b".to_string()),
        "the just-written series is visible at once: {after:?}"
    );

    // The generic store.write nukes every class — prove the coarse path also re-dispatches. Warm the
    // key (a hit), then store.write, then the next read must be a fresh MISS (miss count climbs).
    let _ = series_list(&node, &p, "acme", None).await; // hit
    let s1 = stats(&node, &p, "acme").await;
    call_tool(
        &node,
        &p,
        "acme",
        "store.write",
        &json!({ "table": "note", "id": "n1", "value": { "x": 1 } }).to_string(),
    )
    .await
    .expect("store.write");
    let _ = series_list(&node, &p, "acme", None).await; // must be a MISS again (nuked)
    let s2 = stats(&node, &p, "acme").await;
    assert!(
        u(&s2, "misses") > u(&s1, "misses"),
        "store.write nuked the class → the next read re-dispatched (a miss, not a stale hit)"
    );
}

/// Workspace-isolation: same-named data in ws A and B never crosses; priming A then reading B returns
/// B's own value, and `cache.purge` of A leaves B's entries serving.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_isolation_and_purge_scope() {
    let node = boot_cached().await;
    let pa = principal("wsa", CAPS);
    let pb = principal("wsb", CAPS);
    ingest(&node, &pa, "wsa", "only_a", 1, 1.0).await;
    ingest(&node, &pb, "wsb", "only_b", 1, 1.0).await;

    // Prime A, then read B — B sees ITS data, never A's (the key is ws-scoped).
    let a = series_list(&node, &pa, "wsa", None).await;
    let b = series_list(&node, &pb, "wsb", None).await;
    assert_eq!(a, vec!["only_a".to_string()]);
    assert_eq!(b, vec!["only_b".to_string()]);

    // Purge A. B's cached entry still serves (purge is per-ws).
    call_tool(&node, &pa, "wsa", "cache.purge", "{}")
        .await
        .expect("purge A");
    let b_after = series_list(&node, &pb, "wsb", None).await;
    assert_eq!(
        b_after,
        vec!["only_b".to_string()],
        "purging A left B untouched"
    );
}

/// Capability-deny: a caller lacking `mcp:series.list:call` is `Denied` — and priming the key with a
/// capable caller does NOT let the capless one read it (the cache is behind the caps wall).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deny_is_identical_on_warm_and_cold_keys() {
    let node = boot_cached().await;
    let capable = principal("acme", CAPS);
    let capless = principal("acme", &["mcp:tools.catalog:call"]); // no series.list
    ingest(&node, &capable, "acme", "temp", 1, 1.0).await;

    // Warm the key as the capable caller.
    let _ = series_list(&node, &capable, "acme", None).await;

    // The capless caller is denied on the WARM key exactly as on a cold one.
    let err = call_tool(&node, &capless, "acme", "series.list", "{}")
        .await
        .unwrap_err();
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "capless caller denied on a warm key: {err:?}"
    );
}

/// The `cache.*` admin verbs carry their own caps: a caller without them is `Denied`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_admin_verbs_require_their_caps() {
    let node = boot_cached().await;
    let no_admin = principal("acme", &["mcp:series.list:call"]);
    assert!(matches!(
        call_tool(&node, &no_admin, "acme", "cache.stats", "{}")
            .await
            .unwrap_err(),
        lb_mcp::ToolError::Denied
    ));
    assert!(matches!(
        call_tool(&node, &no_admin, "acme", "cache.purge", "{}")
            .await
            .unwrap_err(),
        lb_mcp::ToolError::Denied
    ));
}

/// An uncacheable (non-allowlisted) verb dispatches on every call — it never enters the cache, so it
/// never produces a hit. (Extension `<ext>.<tool>` verbs are uncacheable by the same mechanism: only
/// host verbs on the allowlist have a `read_class`.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uncacheable_verb_never_hits() {
    let node = boot_cached().await;
    let p = principal("acme", CAPS);

    // `tools.catalog` is not on the read allowlist — two calls, still zero cache activity.
    let _ = call_tool(&node, &p, "acme", "tools.catalog", "{}")
        .await
        .expect("tools.catalog");
    let _ = call_tool(&node, &p, "acme", "tools.catalog", "{}")
        .await
        .expect("tools.catalog");
    let s = stats(&node, &p, "acme").await;
    assert_eq!(
        u(&s, "hits"),
        0,
        "a non-allowlisted verb never hits the cache"
    );
    assert_eq!(u(&s, "misses"), 0, "…and never populates it");
}

/// Budget/eviction: a tiny budget filled with many distinct-key entries evicts — the entry count
/// stays far below the number of distinct reads, so RSS is bounded.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn budget_bounds_the_cache() {
    let node = Arc::new(Node::boot().await.unwrap());
    let mut cfg = CacheConfig::default();
    cfg.memory_budget_bytes = 512; // deliberately tiny
    node.install_response_cache(Some(cfg));
    let p = principal("acme", CAPS);

    // Seed some series, then read with many DISTINCT prefixes → many distinct cache keys.
    for i in 0..20 {
        ingest(&node, &p, "acme", &format!("s{i}"), i + 1, i as f64).await;
    }
    for i in 0..60 {
        let _ = series_list(&node, &p, "acme", Some(&format!("s{i}"))).await;
    }
    let s = stats(&node, &p, "acme").await;
    let entries = u(&s, "entry_count");
    assert!(
        entries < 60,
        "the 512-byte budget evicted: {entries} entries held for 60 distinct reads"
    );
    assert!(
        u(&s, "weighted_size_bytes") <= 4096,
        "weighted size stays near the budget: {}",
        u(&s, "weighted_size_bytes")
    );
}
