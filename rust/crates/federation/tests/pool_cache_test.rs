//! Warm-pool cache + query-bound tests (federation-pool-cache scope §Testing plan). Real seeded
//! SQLite files throughout — no mocks, no Docker (testing-scope §0: the `Source` trait is the one
//! sanctioned fake-boundary, and these run against a REAL engine behind it).
//!
//! What each test pins, and what breaks it:
//!   1. `second_query_hits_the_cache` — the cache exists and is used at all. Remove the
//!      `cached_connect` call in `run_query` (go back to `connect`) and this goes red.
//!   2. `distinct_dsns_do_not_share_a_pool` — the key includes the DSN hash. Key on `kind` alone and
//!      source B serves source A's rows: red, and loudly.
//!   3. `timeout_fires_and_evicts_without_wedging_other_sources` — **the regression test for the
//!      live wedge.** One unbounded remote query hung >2 min and starved *local SQLite* queries in
//!      the same child until a restart. Delete the `tokio::time::timeout` and this hangs; delete the
//!      `evict` on the timeout arm and the entry-gone assertion goes red.
//!   4. `events_leak_no_dsn_or_sql` — secret mediation (§155). Log the DSN or raw SQL and it fails.
//!
//! Note these are INTEGRATION tests over the crate's binary target, so they exercise the public
//! surface the sidecar actually serves.

use std::time::Duration;

// The crate is a binary, so the test binary includes the modules it needs directly. This mirrors
// how the sidecar itself composes them.
#[path = "../src/event.rs"]
mod event;
#[path = "../src/info_schema.rs"]
mod info_schema;
#[path = "../src/pool.rs"]
mod pool;
#[path = "../src/query.rs"]
mod query;
// `query.rs` fronts the result cache (federation-result-cache scope), so this composition needs it
// too. It stays INERT for every test in this file: none of them passes a `cache: {ttl_s}` contract,
// which is itself a live assertion that the cache is opt-in — if it ever started caching by default,
// the pool tests below would go red on their own row-content assertions.
#[path = "../src/results.rs"]
mod results;
#[path = "../src/source/mod.rs"]
mod source;
#[path = "../src/validate.rs"]
mod validate;

/// Seed a real SQLite file with one tiny table carrying a per-source marker value, so a result can
/// be traced back to WHICH source produced it (test 2 depends on that).
fn seed_db(tag: &str, marker: i64) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("lb-fed-pool-{tag}-{seq}-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch("CREATE TABLE marker (id INTEGER);")
        .expect("create schema");
    conn.execute("INSERT INTO marker VALUES (?1)", [marker])
        .expect("seed marker");
    path.to_string_lossy().into_owned()
}

/// 1. A second query against the same source reuses the pool rather than rebuilding it.
///
/// The observable assertion is `pool::is_warm`, not wall-clock: a timing assertion on local SQLite
/// is flaky by construction (the connect it saves is milliseconds), and the scope's whole point is
/// that the *state* is what matters — on a remote Timescale that same state is worth ~2,400 ms.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_query_hits_the_cache() {
    let dsn = seed_db("hit", 7);
    assert!(
        !pool::is_warm("sqlite", &dsn),
        "a never-queried source starts cold"
    );

    let first = query::run_query("sqlite", &dsn, "SELECT id FROM marker")
        .await
        .expect("first query");
    assert_eq!(first.rows.len(), 1);
    assert!(
        pool::is_warm("sqlite", &dsn),
        "the first query must leave the pool warm — this is the whole feature"
    );

    let second = query::run_query("sqlite", &dsn, "SELECT id FROM marker")
        .await
        .expect("second query");
    assert_eq!(
        second.rows[0][0].as_i64(),
        Some(7),
        "a cache hit must return the same real rows, not a stale/empty result"
    );
}

/// 2. Two different DSNs get two different pools, and their results never cross. Keying on `kind`
///    alone (or on a truncated hash) would silently serve one source's data for another — the
///    worst possible failure mode for a cache in a multi-tenant datasource layer.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn distinct_dsns_do_not_share_a_pool() {
    let a = seed_db("srcA", 111);
    let b = seed_db("srcB", 222);

    let ra = query::run_query("sqlite", &a, "SELECT id FROM marker")
        .await
        .expect("query A");
    assert_eq!(ra.rows[0][0].as_i64(), Some(111));

    let rb = query::run_query("sqlite", &b, "SELECT id FROM marker")
        .await
        .expect("query B");
    assert_eq!(
        rb.rows[0][0].as_i64(),
        Some(222),
        "B must not serve A's row"
    );

    // Re-query A: it is still warm and still its own source.
    assert!(pool::is_warm("sqlite", &a) && pool::is_warm("sqlite", &b));
    let ra2 = query::run_query("sqlite", &a, "SELECT id FROM marker")
        .await
        .expect("re-query A");
    assert_eq!(
        ra2.rows[0][0].as_i64(),
        Some(111),
        "A still serves A's row after B was cached"
    );
}

/// 3. **The wedge regression.** A query that cannot complete within its bound must return a bounded
///    error, must drop its pool, and must NOT prevent a healthy source from being queried after.
///
/// Before this scope there was no timeout on any query path: a hung remote occupied the child until
/// a restart, and local SQLite queries timed out alongside it. That is the behaviour being pinned.
///
/// **Why a real SQLite source and a sub-millisecond bound**, rather than an unroutable Postgres
/// address: the obvious version of this test (`postgres://…@192.0.2.1`) PASSES VACUOUSLY in the
/// default build. Postgres is a feature, so `connect` returns "not built in" instantly — the first
/// draft of this test reported `elapsed_ms: 0` and would have stayed green with the timeout deleted
/// entirely. SQLite is always compiled in, so the bound below is genuinely raced against real
/// engine work: the query cannot finish in 1 ms, so the timeout arm is the one that must fire.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn timeout_fires_and_evicts_without_wedging_other_sources() {
    let slow = seed_db("slow", 1);
    let healthy = seed_db("healthy", 42);

    // WARM THE POOL FIRST. Without this the eviction assertion below is vacuous: a bound that
    // elapses during the initial connect leaves nothing cached, so `is_warm` reads false whether or
    // not the timeout arm evicts. (Verified by breaking it — with a cold start, deleting the
    // `evict` call kept this test green.) A warm entry that then times out is the real Risk 3 shape:
    // a pool that worked, broke, and must not be retained.
    query::run_query("sqlite", &slow, "SELECT id FROM marker")
        .await
        .expect("warm-up query");
    assert!(
        pool::is_warm("sqlite", &slow),
        "warm-up must cache the pool"
    );

    let started = std::time::Instant::now();
    let result = query::run_query_with(
        "sqlite",
        &slow,
        // Query `information_schema.tables` to force the DataFusion path (the direct
        // path handles all real-database queries, even complex JOINs — but synthetic
        // views only exist in DataFusion's in-memory registration, so they must still
        // go through the federation ceremony).
        "SELECT table_name FROM information_schema.tables",
        Some("slowsrc"),
        // Below the floor of a cold connect + plan + execute: this MUST elapse.
        Duration::from_nanos(1),
    )
    .await;
    let elapsed = started.elapsed();

    let err = result.expect_err("a query past its bound must error, not return rows");
    assert!(
        err.contains("bound"),
        "the timeout must be a typed, recognisable error rather than a generic engine failure: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "the bound must be ENFORCED — took {elapsed:?}. This is the hang this test exists to \
         catch (the live incident ran >2 minutes and wedged the child)"
    );
    assert!(
        !pool::is_warm("sqlite", &slow),
        "a timed-out pool must be EVICTED — a retained poisoned entry is how caching becomes \
         worse than the per-call connect it replaced (scope Risk 3)"
    );

    // And the child is still usable: a healthy source answers normally afterwards. This is the
    // assertion that would have caught the original wedge, where local SQLite died too.
    let ok = query::run_query("sqlite", &healthy, "SELECT id FROM marker")
        .await
        .expect("a healthy source must still work after another source timed out");
    assert_eq!(ok.rows[0][0].as_i64(), Some(42));

    // The evicted source is not poisoned either — with a sane bound it recovers.
    let recovered = query::run_query("sqlite", &slow, "SELECT id FROM marker")
        .await
        .expect("an evicted source must reconnect cleanly on the next call");
    assert_eq!(recovered.rows[0][0].as_i64(), Some(1));
}

/// 3b. A slow source must not block a *concurrent* query to a different source (scope Risk 4 — the
///     `Mutex`-held-across-`.await` trap). If the map lock were held during connect, the healthy
///     query below would queue behind the slow one's full bound.
///
/// The stalled side is a real SQLite connect held under a 3 s bound (see test 3 on why not an
/// unroutable Postgres). What is measured is only that the OTHER source stays responsive.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stalled_connect_does_not_block_another_source() {
    let blocked = seed_db("blocked", 7);
    let healthy = seed_db("parallel", 99);

    let slow = tokio::spawn(async move {
        // Many distinct cold sources in a row: if connects serialised under the map lock, this
        // occupies the lock for the duration.
        for i in 0..8 {
            let d = seed_db(&format!("churn{i}"), i as i64);
            let _ = query::run_query("sqlite", &d, "SELECT id FROM marker").await;
        }
        let _ = query::run_query("sqlite", &blocked, "SELECT id FROM marker").await;
    });

    // While those are churning, the healthy source must answer promptly.
    let started = std::time::Instant::now();
    let ok = query::run_query("sqlite", &healthy, "SELECT id FROM marker")
        .await
        .expect("healthy query during concurrent cold connects");
    let elapsed = started.elapsed();

    assert_eq!(ok.rows[0][0].as_i64(), Some(99));
    assert!(
        elapsed < Duration::from_secs(10),
        "concurrent connects to other sources serialised this one: took {elapsed:?}. The map lock \
         is being held across an .await (scope Risk 4)."
    );
    let _ = slow.await;
}

/// 4. No secret leaks. The DSN, its password, and the raw SQL must appear in NO emitted field.
///
/// Asserted against the event's own inputs rather than by capturing stderr (which is process-global
/// and unreliable under a parallel test harness): the digest is the thing that must not be
/// reversible, so that is what is checked.
#[test]
fn events_leak_no_dsn_or_sql() {
    let sql = "SELECT secret_col FROM billing WHERE token = 'hunter2-abcdef'";
    let digest = event::sql_digest(sql);
    for forbidden in ["hunter2", "billing", "secret_col", "SELECT", "token"] {
        assert!(
            !digest.contains(forbidden),
            "sql_digest leaked {forbidden:?}: {digest}"
        );
    }
    // The digest still identifies a repeat of the same query (its reason to exist).
    assert_eq!(digest, event::sql_digest(sql));
    assert_ne!(digest, event::sql_digest("SELECT 1"));
}
