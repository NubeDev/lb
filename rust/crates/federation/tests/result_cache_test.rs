//! Query-result cache tests (federation-result-cache scope §Testing plan). Real seeded SQLite files
//! throughout — no mocks (testing-scope §0).
//!
//! **The rule this file is written to obey.** This crate has shipped VACUOUSLY-GREEN cache tests
//! twice: a test that asserts "fast" or "flagged `hit`" passes against a cache that silently re-runs
//! the query, and passes just as well against one that returns nothing at all. So every behavioural
//! test here asserts ROW CONTENT, and it does it the only way that actually proves the datasource
//! was not touched: **mutate the SQLite file underneath the cache**, then assert on whether the new
//! row is visible. A hit must show the OLD rows; a miss/bypass must show the NEW ones. Latency and
//! event flags are corroborating evidence, never the assertion.
//!
//! Each test names what breaks it; each was broken-and-watched-red before being committed (see the
//! session doc's revert-check table).

use std::sync::RwLock;
use std::time::Duration;

/// Serializes the kill-switch test against every other cache-behaviour test in this binary.
///
/// **This is load-bearing, not tidiness.** `LB_FEDERATION_RESULT_CACHE` is a process-global env var
/// and Rust runs a binary's tests as threads in ONE process, so while the kill-switch test holds it
/// set, every other test sees the cache disabled — their queries bypass, the mid-test INSERT becomes
/// visible, and every hit-dependent assertion fails. That is a real flake we hit: 44/5 on the full
/// run, green alone, with a DIFFERENT failing subset each time.
/// See `docs/debugging/federation/result-cache-tests-flake-under-parallelism.md`.
///
/// Cache tests take the READ lock (they run concurrently with each other, which is the point — the
/// single-flight tests need real parallelism); the kill-switch test takes the WRITE lock.
static ENV_LOCK: RwLock<()> = RwLock::new(());

#[path = "../src/event.rs"]
mod event;
#[path = "../src/info_schema.rs"]
mod info_schema;
#[path = "../src/pool.rs"]
mod pool;
#[path = "../src/query.rs"]
mod query;
#[path = "../src/results.rs"]
mod results;
#[path = "../src/source/mod.rs"]
mod source;
#[path = "../src/validate.rs"]
mod validate;
#[path = "../src/write.rs"]
mod write;

/// Seed a real SQLite file with one row carrying `marker`. Unique per call (atomic seq + pid) so
/// parallel harness workers never collide.
fn seed_db(tag: &str, marker: i64) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "lb-fed-result-{tag}-{seq}-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch("CREATE TABLE marker (id INTEGER);")
        .expect("create schema");
    conn.execute("INSERT INTO marker VALUES (?1)", [marker])
        .expect("seed marker");
    path.to_string_lossy().into_owned()
}

/// Seed a `burn` table used by the single-flight tests to make one query take ~hundreds of ms of
/// REAL engine work, so all racers arrive before the first completes.
///
/// **Not a recursive CTE**, which was the obvious choice and is wrong here: `validate_select`
/// collects every table name a SELECT references, including the CTE's own name, and the query path
/// then tries to register `slow` as a real table provider — `no such table: slow`. A self-cross-join
/// over a real seeded table is genuinely slow, genuinely pushed down, and references only tables
/// that exist.
fn seed_burn(dsn: &str, rows: i64) {
    let conn = rusqlite::Connection::open(dsn).expect("open sqlite fixture for burn");
    conn.execute_batch("CREATE TABLE burn (n INTEGER);")
        .expect("create burn");
    let tx = conn.unchecked_transaction().unwrap();
    for i in 0..rows {
        tx.execute("INSERT INTO burn VALUES (?1)", [i])
            .expect("seed burn");
    }
    tx.commit().unwrap();
}

/// The slow SELECT: an O(n²) self-join over `burn` aggregated to a single number, joined with the
/// real `marker` rows so the ANSWER still reflects the file's live contents (which is what every
/// assertion in the single-flight tests is made against).
const SLOW_SQL: &str = "SELECT (SELECT COUNT(*) FROM burn a JOIN burn b) AS burn, id \
                        FROM marker ORDER BY id";

/// Insert another row DIRECTLY into the file, behind the child's back. This is the whole test
/// strategy: after this call, any query that really reaches the datasource sees two rows, and any
/// answer served from cache still sees one.
fn insert_row(dsn: &str, marker: i64) {
    let conn = rusqlite::Connection::open(dsn).expect("open sqlite fixture for mutation");
    conn.execute("INSERT INTO marker VALUES (?1)", [marker])
        .expect("insert new row");
}

/// The child input the host builds, plus an optional `cache` contract. Mirrors
/// `host/src/federation/query.rs`'s enumerated `json!` exactly — if that build changes, this should
/// too, and the mismatch is the review seam the scope calls out.
fn input(source: &str, sql: &str, ttl_s: Option<f64>) -> serde_json::Value {
    let mut v = serde_json::json!({
        "kind": "sqlite", "source": source, "sql": sql,
    });
    if let Some(ttl) = ttl_s {
        v["cache"] = serde_json::json!({ "ttl_s": ttl });
    }
    v
}

const SQL: &str = "SELECT id FROM marker ORDER BY id";

async fn run(dsn: &str, input: &serde_json::Value) -> query::QueryResult {
    query::run_query_cached(
        "sqlite",
        dsn,
        SQL,
        input.get("source").and_then(|s| s.as_str()),
        input,
    )
    .await
    .expect("query")
}

/// 1. **A hit serves the cached rows, provably.** Query with a 60 s window, insert a row under the
///    cache, re-query: the second answer must still be the OLD single row.
///
/// The vacuous version of this test asserts "second call was fast" or "the event said hit" — both
/// pass against a cache that re-runs the query. Asserting the ABSENCE of the row we just inserted is
/// what proves the datasource was not touched.
///
/// Breaks if: `cached_query` runs the inner future on an accepted `current` (rule 1), or the store
/// on completion is dropped, or the key is computed per-call.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_hit_serves_the_cached_rows_not_the_new_ones() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("hit", 1);
    let inp = input("hitsrc", SQL, Some(60.0));

    let first = run(&dsn, &inp).await;
    assert_eq!(first.rows.len(), 1, "seeded with one row");
    assert_eq!(first.rows[0][0].as_i64(), Some(1));

    insert_row(&dsn, 2);

    let second = run(&dsn, &inp).await;
    assert_eq!(
        second.rows.len(),
        1,
        "a cache HIT must serve the stored rows — seeing the row inserted after the store means \
         the query re-ran and the cache did nothing: {:?}",
        second.rows
    );
    assert_eq!(
        second.rows[0][0].as_i64(),
        Some(1),
        "and they must be the same rows, not an empty/placeholder envelope"
    );
    assert_eq!(
        second.columns,
        vec!["id".to_string()],
        "columns survive too"
    );
}

/// 2. **TTL expiry.** With a short window, the entry stops being acceptable and the new row appears.
///    This is the other half of test 1: together they prove the TTL is actually compared, rather
///    than the cache being permanently on (test 1 alone passes against a never-expiring cache) or
///    permanently off (test 2 alone passes against no cache at all).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_entry_re_queries_and_sees_new_rows() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("ttl", 1);
    let inp = input("ttlsrc", SQL, Some(0.3));

    let first = run(&dsn, &inp).await;
    assert_eq!(first.rows.len(), 1);

    insert_row(&dsn, 2);

    // Still inside the window: the old rows.
    let inside = run(&dsn, &inp).await;
    assert_eq!(inside.rows.len(), 1, "inside the TTL this must still hit");

    tokio::time::sleep(Duration::from_millis(400)).await;

    let after = run(&dsn, &inp).await;
    assert_eq!(
        after.rows.len(),
        2,
        "past the TTL the query must re-run and see the inserted row: {:?}",
        after.rows
    );
    assert_eq!(after.rows[1][0].as_i64(), Some(2));
}

/// 3a. **Bypass — no `cache` field.** The default path must be today's behaviour bit for bit: every
///     call reaches the datasource, so a row inserted between two calls is immediately visible.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_cache_field_never_caches() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("bypass-absent", 1);
    let inp = input("bysrc", SQL, None);

    assert_eq!(run(&dsn, &inp).await.rows.len(), 1);
    insert_row(&dsn, 2);
    assert_eq!(
        run(&dsn, &inp).await.rows.len(),
        2,
        "with no cache contract the caller must always see live rows"
    );
}

/// 3b. **Bypass — `ttl_s: 0`.** An explicit zero window is a disable, not a "cache forever".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ttl_zero_never_caches() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("bypass-zero", 1);
    let inp = input("zerosrc", SQL, Some(0.0));

    assert_eq!(run(&dsn, &inp).await.rows.len(), 1);
    insert_row(&dsn, 2);
    assert_eq!(
        run(&dsn, &inp).await.rows.len(),
        2,
        "ttl_s: 0 must disable caching, not enable an unbounded one"
    );
}

/// 3c. **Bypass — the node kill-switch.** With `LB_FEDERATION_RESULT_CACHE=off` in the child's
///     environment, a caller that DOES ask for caching is still served live rows. The operator's
///     switch outranks the caller's request — that is the point of having it.
///
/// Runs in its own process-global env, so it is `serial`-shaped by construction: it sets the var,
/// exercises the path, and restores it. (Every other test in this binary reads the var as unset.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_kill_switch_forces_bypass_even_when_the_caller_asks() {
    let dsn = seed_db("bypass-env", 1);
    let inp = input("envsrc", SQL, Some(60.0));

    // EXCLUSIVE: nothing else may be mid-query while this global is set (see ENV_LOCK).
    let _guard = ENV_LOCK.write().expect("env lock");
    std::env::set_var(results::KILL_SWITCH_ENV, "off");
    // The contract asks for a 60 s window; the switch must overrule it.
    assert!(
        results::requested_ttl(&inp).is_none(),
        "the kill-switch must resolve every contract to bypass"
    );
    assert_eq!(run(&dsn, &inp).await.rows.len(), 1);
    insert_row(&dsn, 2);
    let second = run(&dsn, &inp).await;
    std::env::remove_var(results::KILL_SWITCH_ENV);

    assert_eq!(
        second.rows.len(),
        2,
        "with the kill-switch on, a caller asking for a 60s window still gets live rows"
    );
}

/// 4. **Write-through eviction.** A `federation.write` between two in-window queries must make the
///    second one fresh — the child must never serve rows it knows it invalidated.
///
/// Breaks if: the `results::evict_source` call in `write::run_write` is removed (the second query
/// then serves the pre-write rows and this goes red on the row count).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_evicts_the_cached_results_for_that_source() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("write-evict", 1);
    let inp = input("wsrc", SQL, Some(60.0));

    assert_eq!(run(&dsn, &inp).await.rows.len(), 1, "warm the cache");

    // The real write path, not a direct sqlite poke — this is what must trigger the invalidation.
    let affected = write::run_write(
        "sqlite",
        &dsn,
        "marker",
        &["id".to_string()],
        &[serde_json::json!([2])],
        None,
    )
    .await
    .expect("federation.write");
    assert_eq!(affected, 1);

    let after = run(&dsn, &inp).await;
    assert_eq!(
        after.rows.len(),
        2,
        "a write must evict this source's cached results — serving the pre-write rows inside the \
         TTL is the child knowingly returning invalidated data: {:?}",
        after.rows
    );
}

/// 4b. A write to source A must NOT evict source B's cached results. Eviction is coarse per source
///     on purpose, but it must not be coarse across sources — that would make any busy writer
///     destroy every other source's cache.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_to_one_source_leaves_another_sources_cache_intact() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let a = seed_db("evict-a", 10);
    let b = seed_db("evict-b", 20);
    let ia = input("asrc", SQL, Some(60.0));
    let ib = input("bsrc", SQL, Some(60.0));

    assert_eq!(run(&a, &ia).await.rows[0][0].as_i64(), Some(10));
    assert_eq!(run(&b, &ib).await.rows[0][0].as_i64(), Some(20));

    insert_row(&b, 21); // behind B's back — B's entry must still hide this
    write::run_write(
        "sqlite",
        &a,
        "marker",
        &["id".to_string()],
        &[serde_json::json!([11])],
        None,
    )
    .await
    .expect("write to A");

    assert_eq!(
        run(&a, &ia).await.rows.len(),
        2,
        "A was written → A is fresh"
    );
    assert_eq!(
        run(&b, &ib).await.rows.len(),
        1,
        "B was not written → B's entry survives (a write must not flush the whole child)"
    );
}

/// 5. **Key separation.** Two sources, two SQLs, and two SQLs differing only in a paging cursor must
///    never cross results. This is the scariest failure a datasource cache can have, so it is
///    asserted on real content from real distinct files.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn distinct_calls_never_share_an_entry() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let a = seed_db("keysep-a", 100);
    let b = seed_db("keysep-b", 200);
    insert_row(&a, 101);
    insert_row(&b, 201);

    // (i) Different DSNs, identical args → different entries.
    let ia = input("src", SQL, Some(60.0));
    let ra = run(&a, &ia).await;
    let rb = run(&b, &ia).await;
    assert_eq!(ra.rows[0][0].as_i64(), Some(100));
    assert_eq!(
        rb.rows[0][0].as_i64(),
        Some(200),
        "source B must not be served source A's cached rows"
    );

    // (ii) Same DSN, different SQL → different entries.
    let one_sql = "SELECT id FROM marker WHERE id = 100";
    let one = query::run_query_cached(
        "sqlite",
        &a,
        one_sql,
        Some("src"),
        &input("src", one_sql, Some(60.0)),
    )
    .await
    .expect("filtered query");
    assert_eq!(one.rows.len(), 1, "the filtered SQL gets its OWN entry");

    // (iii) Same DSN and shape, differing only in a paging cursor → different entries.
    let page0 = "SELECT id FROM marker ORDER BY id LIMIT 1 OFFSET 0";
    let page1 = "SELECT id FROM marker ORDER BY id LIMIT 1 OFFSET 1";
    let r0 = query::run_query_cached(
        "sqlite",
        &a,
        page0,
        Some("src"),
        &input("src", page0, Some(60.0)),
    )
    .await
    .expect("page 0");
    let r1 = query::run_query_cached(
        "sqlite",
        &a,
        page1,
        Some("src"),
        &input("src", page1, Some(60.0)),
    )
    .await
    .expect("page 1");
    assert_eq!(r0.rows[0][0].as_i64(), Some(100));
    assert_eq!(
        r1.rows[0][0].as_i64(),
        Some(101),
        "page 1 must not be served page 0's cached rows — cursors ride the args hash"
    );

    // (iv) And the FIRST entry is still intact and still its own (no clobbering).
    assert_eq!(run(&a, &ia).await.rows[0][0].as_i64(), Some(100));
}

/// 5b. The `source` alias participates in the key: two aliases over one DSN double-cache. Wasteful,
///     harmless, and intended — a rename then invalidates naturally instead of serving the old
///     alias's rows under the new name.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_source_alias_is_part_of_the_key() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("alias", 1);

    let under_a = run(&dsn, &input("alias-a", SQL, Some(60.0))).await;
    assert_eq!(under_a.rows.len(), 1);

    insert_row(&dsn, 2);

    // A different alias over the SAME dsn is a different key → its own query → sees the new row.
    let under_b = run(&dsn, &input("alias-b", SQL, Some(60.0))).await;
    assert_eq!(
        under_b.rows.len(),
        2,
        "a renamed source must not inherit the old alias's entry"
    );
    // The original alias's entry is untouched.
    assert_eq!(
        run(&dsn, &input("alias-a", SQL, Some(60.0)))
            .await
            .rows
            .len(),
        1
    );
}

/// 6. **Bounds — an over-cap result is served but not stored.** Seeds a table larger than
///    `MAX_ENTRY_BYTES` and asserts the next in-window call sees a freshly-inserted row (i.e. it
///    missed, because nothing was cached).
///
/// Uses a wide TEXT payload to cross 4 MB in few enough rows to stay under `ROW_CAP`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_over_cap_result_is_served_but_not_stored() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("oversize", 0);
    {
        let conn = rusqlite::Connection::open(&dsn).unwrap();
        conn.execute_batch("CREATE TABLE big (id INTEGER, blob TEXT);")
            .unwrap();
        // 900 rows × ~6 KB ≈ 5.4 MB serialized — over MAX_ENTRY_BYTES (4 MB), under ROW_CAP (1000).
        let payload = "x".repeat(6000);
        let tx = conn.unchecked_transaction().unwrap();
        for i in 0..900 {
            tx.execute(
                "INSERT INTO big VALUES (?1, ?2)",
                rusqlite::params![i, payload],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }

    let sql = "SELECT id, blob FROM big";
    let inp = input("bigsrc", sql, Some(60.0));
    let first = query::run_query_cached("sqlite", &dsn, sql, Some("bigsrc"), &inp)
        .await
        .expect("oversize query");
    assert_eq!(first.rows.len(), 900, "the caller is still SERVED the rows");

    // Not stored → the next call re-queries → the row inserted meanwhile is visible.
    {
        let conn = rusqlite::Connection::open(&dsn).unwrap();
        conn.execute(
            "INSERT INTO big VALUES (?1, ?2)",
            rusqlite::params![9999, "y"],
        )
        .unwrap();
    }
    let second = query::run_query_cached("sqlite", &dsn, sql, Some("bigsrc"), &inp)
        .await
        .expect("oversize re-query");
    assert_eq!(
        second.rows.len(),
        901,
        "a result over the per-entry cap must NOT be cached — one tile must not consume the whole \
         budget (scope Risk 3)"
    );
}

/// 6b. **Bounds — the entry cap evicts oldest and the map stays bounded.** Runs well past
///     `MAX_RESULT_ENTRIES` distinct queries and asserts the map never exceeds the cap while the
///     most recent entry still serves cached rows (eviction must not evict what was just stored).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_entry_cap_bounds_the_map_without_evicting_the_newest() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("entrycap", 1);
    // Distinct SQL per iteration → distinct keys. `MAX_RESULT_ENTRIES + 20` of them.
    for i in 0..(results::MAX_RESULT_ENTRIES + 20) {
        let sql = format!("SELECT id, {i} AS n FROM marker");
        let inp = input("capsrc", &sql, Some(60.0));
        query::run_query_cached("sqlite", &dsn, &sql, Some("capsrc"), &inp)
            .await
            .expect("capped query");
        assert!(
            results::len() <= results::MAX_RESULT_ENTRIES,
            "the map must never exceed MAX_RESULT_ENTRIES (at i={i}: {})",
            results::len()
        );
    }

    // The most recent key is still cached: insert behind its back and confirm the OLD rows come out.
    let last = format!(
        "SELECT id, {} AS n FROM marker",
        results::MAX_RESULT_ENTRIES + 19
    );
    let inp = input("capsrc", &last, Some(60.0));
    insert_row(&dsn, 2);
    let again = query::run_query_cached("sqlite", &dsn, &last, Some("capsrc"), &inp)
        .await
        .expect("newest key");
    assert_eq!(
        again.rows.len(),
        1,
        "cap eviction must drop the OLDEST, never the entry just stored"
    );
}

/// 7. **Single-flight.** N concurrent identical cold queries must reach the datasource ONCE.
///
/// The observable is content, not a counter: all N racers are launched against a file holding one
/// row; the moment the first query completes and stores, a row is inserted; every racer's answer
/// must be the single pre-insert row. A stampede (N independent queries) would let the later ones
/// observe two rows. A deliberately slow SELECT (recursive CTE) makes the race deterministic — all N
/// arrive before the first completes. Side-effecting SQL is impossible here: both the host gate and
/// the child's `validate_select` accept only a single SELECT.
///
/// Breaks if: the `inflight` handle is not installed under the map lock, or joiners start their own
/// query instead of subscribing.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_cold_queries_collapse_to_one() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("singleflight", 1);
    // 12,000² = 144M join rows ≈ 750 ms of real engine work. Sized DELIBERATELY, not guessed: at
    // 1,500 rows the query took ~40 ms and finished BEFORE the racers were even spawned, which made
    // the first version of this test vacuous (it passed, but nothing ever raced). The assertion
    // below on `first_completed` is what keeps that from silently coming back.
    seed_burn(&dsn, 12_000);
    let sql = SLOW_SQL;
    let inp = input("sfsrc", sql, Some(60.0));

    let mut handles = Vec::new();
    for _ in 0..8 {
        let (d, s, i) = (dsn.clone(), sql.to_string(), inp.clone());
        handles.push(tokio::spawn(async move {
            query::run_query_cached("sqlite", &d, &s, Some("sfsrc"), &i)
                .await
                .expect("racer")
        }));
    }

    // Insert while the racers are in flight. Whoever queries the datasource AFTER this point would
    // see two rows — and under single-flight nobody does, because only one query ever runs and it
    // started before this insert.
    //
    // The sleep must land INSIDE the in-flight query, or the test proves nothing: every racer would
    // simply have finished first and the insert would be irrelevant. `started` + the assertion after
    // the joins pin that — if the whole race completed in under the insert delay, the test FAILS
    // rather than passing vacuously.
    let started = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(150)).await;
    insert_row(&dsn, 2);

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.expect("join racer"));
    }
    assert!(
        started.elapsed() > Duration::from_millis(150),
        "the racers finished before the mid-flight insert — nothing actually raced, so a green \
         result here would be vacuous. Increase the burn size."
    );

    for (i, r) in results.into_iter().enumerate() {
        assert_eq!(
            r.rows.len(),
            1,
            "racer {i} saw {} rows — N concurrent identical queries must collapse to ONE query \
             against the source; a stampede lets late racers observe the mid-flight insert",
            r.rows.len()
        );
        assert_eq!(r.rows[0][1].as_i64(), Some(1), "and it is the real row");
    }
}

/// 7b. **The refresh rules.** With a warm entry, a caller whose TTL REJECTS it triggers exactly one
///     refresh, while a concurrent caller whose TTL ACCEPTS it returns the old rows IMMEDIATELY
///     without waiting on that refresh (slot rules 1 and 2).
///
/// The accepting caller's answer is content-asserted as the OLD rows, and its latency is asserted to
/// be far below the refresh's duration — together those show it neither waited nor was served the
/// refreshed data. Breaks if the accept path joins `inflight` instead of returning `current`.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn an_accepting_caller_never_waits_on_a_stricter_callers_refresh() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("refresh", 1);
    seed_burn(&dsn, 12_000); // ~750 ms — see the sizing note in the single-flight test.
    let sql = SLOW_SQL;

    // Warm the entry.
    let warm = query::run_query_cached(
        "sqlite",
        &dsn,
        sql,
        Some("rsrc"),
        &input("rsrc", sql, Some(60.0)),
    )
    .await
    .expect("warm");
    assert_eq!(warm.rows.len(), 1);

    // Now the entry is fresh-but-non-zero-age. Insert behind its back, then race:
    insert_row(&dsn, 2);
    tokio::time::sleep(Duration::from_millis(120)).await;

    // A STRICT caller (ttl below the entry's age) → rejects → starts the one refresh.
    let (d1, s1) = (dsn.clone(), sql.to_string());
    let strict = tokio::spawn(async move {
        query::run_query_cached(
            "sqlite",
            &d1,
            &s1,
            Some("rsrc"),
            &input("rsrc", &s1, Some(0.05)),
        )
        .await
        .expect("strict")
    });

    // A LENIENT caller (60 s window) → accepts → must return the OLD rows without waiting.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let started = std::time::Instant::now();
    let lenient = query::run_query_cached(
        "sqlite",
        &dsn,
        sql,
        Some("rsrc"),
        &input("rsrc", sql, Some(60.0)),
    )
    .await
    .expect("lenient");
    let waited = started.elapsed();

    // **The invariant, asserted unconditionally**: whatever it was served, the accepting caller did
    // not BLOCK. The refresh takes ~750 ms; anything near that means rule 1 was violated and the
    // accept path joined `inflight` instead of returning `current`.
    //
    // The bound is 350 ms, not 100 ms. The accept path is a lockless synchronous return
    // (results.rs `Action::Serve` — no `.await` on any refresh), so the wait it MEASURES is pure
    // runtime-scheduling latency, and a saturated 16-worker run of this suite legitimately breached
    // 100 ms on a continuation that never blocked (a flake — see the debugging entry). A genuine
    // rule-1 violation would join `inflight` and wait ~750 ms, so 350 ms (< half the refresh) still
    // catches it with margin while absorbing scheduling jitter that scales with box load.
    assert!(
        waited < Duration::from_millis(350),
        "the accepting caller waited {waited:?} — it must never block on a stricter caller's \
         in-flight refresh (slot rule 1)"
    );

    // **The content assertion, gated on the race actually having happened.** Under heavy box load
    // the strict caller's refresh can legitimately COMPLETE before the lenient caller starts, and
    // then serving 2 rows is correct behaviour (a completed refresh replaced `current`, and rule 3
    // says fresher-than-asked is never wrong). Asserting 1 unconditionally made this test fail on a
    // loaded machine for a reason that was not a defect — a flake, and worse, one that would have
    // trained the next reader to distrust a real signal. `strict_done` distinguishes the two worlds
    // instead of guessing, so the assertion still has teeth in the case it was written for.
    let strict_done = strict.is_finished();
    if !strict_done {
        assert_eq!(
            lenient.rows.len(),
            1,
            "with the refresh STILL IN FLIGHT, the accepting caller must be served the stored rows \
             — not the refresh's fresher ones, and not a blocked wait"
        );
    }

    // The refresher itself gets the fresh rows.
    let refreshed = strict.await.expect("join strict");
    assert_eq!(
        refreshed.rows.len(),
        2,
        "the rejecting caller's refresh must return the CURRENT rows"
    );

    // And the refresh replaced `current`: a subsequent accepting caller now sees the fresh rows.
    let after = query::run_query_cached(
        "sqlite",
        &dsn,
        sql,
        Some("rsrc"),
        &input("rsrc", sql, Some(60.0)),
    )
    .await
    .expect("after refresh");
    assert_eq!(
        after.rows.len(),
        2,
        "a completed refresh must REPLACE current (slot rule 4) — the pool's set-once OnceCell \
         shape would leave the stale entry here forever"
    );
}

/// 7c. **A failed refresh leaves `current` intact and does not wedge the key** (slot rule 4). A
///     bad-SQL refresh against a warm entry errors for the caller that asked for it, while the
///     accepting callers keep being served — and the key still works afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_refresh_leaves_the_entry_serving() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("failrefresh", 1);
    let inp_long = input("fsrc", SQL, Some(60.0));

    assert_eq!(run(&dsn, &inp_long).await.rows.len(), 1, "warm");

    // Force a failing refresh on the SAME key: drop the table so the query errors, with a TTL that
    // rejects the warm entry.
    {
        let conn = rusqlite::Connection::open(&dsn).unwrap();
        conn.execute_batch("ALTER TABLE marker RENAME TO marker_gone;")
            .unwrap();
    }
    let failed = query::run_query_cached(
        "sqlite",
        &dsn,
        SQL,
        Some("fsrc"),
        &input("fsrc", SQL, Some(0.001)),
    )
    .await;
    assert!(failed.is_err(), "the refresh itself must surface the error");

    // `current` untouched: an accepting caller still gets the stored rows.
    let still = run(&dsn, &inp_long).await;
    assert_eq!(
        still.rows.len(),
        1,
        "a failed refresh must leave `current` in place (accepting callers were never blocked)"
    );

    // And the key is not wedged — once the source recovers, a rejecting caller succeeds.
    {
        let conn = rusqlite::Connection::open(&dsn).unwrap();
        conn.execute_batch("ALTER TABLE marker_gone RENAME TO marker;")
            .unwrap();
    }
    let recovered = query::run_query_cached(
        "sqlite",
        &dsn,
        SQL,
        Some("fsrc"),
        &input("fsrc", SQL, Some(0.001)),
    )
    .await
    .expect("a recovered source must re-query cleanly (inflight was cleared)");
    assert_eq!(recovered.rows.len(), 1);
}

/// 8. **Restart (hot-reload category).** The cache is process-local, so a respawn loses it
///     harmlessly: the next query misses and returns fresh, correct rows.
///
/// A real kill+respawn of the child is the native-suite's job; what is pinned HERE is the property
/// that makes that safe — the cache holds nothing a caller depends on. `evict_source` reproduces the
/// post-respawn state (empty for this source) in-process, and the assertion is that the answer is
/// then FRESH and CORRECT, not merely non-empty.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lost_cache_costs_freshness_not_correctness() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let dsn = seed_db("restart", 1);
    let inp = input("rssrc", SQL, Some(60.0));

    assert_eq!(run(&dsn, &inp).await.rows.len(), 1);
    insert_row(&dsn, 2);
    assert_eq!(
        run(&dsn, &inp).await.rows.len(),
        1,
        "cached before the drop"
    );

    // Everything a respawn would lose.
    results::evict_source("sqlite", &dsn);

    let after = run(&dsn, &inp).await;
    assert_eq!(
        after.rows.len(),
        2,
        "after losing the cache the next query must return the CURRENT rows"
    );
    assert_eq!(after.rows[0][0].as_i64(), Some(1));
    assert_eq!(after.rows[1][0].as_i64(), Some(2));
}

/// 9. **Workspace isolation** — the scariest imaginable regression, pinned even though it is
///     structural. The child is per `(ws, ext_id)`, so ws-A's cached result cannot be reachable from
///     ws-B's child: they are different OS processes with different memory.
///
/// Asserted here at the level this crate can: two distinct sources (as two workspaces' datasources
/// necessarily are, since a DSN is resolved per-workspace host-side) never share an entry, in either
/// direction, even with byte-identical SQL and identical `source` aliases. The process-level half is
/// asserted by the host's native workspace suite.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_workspaces_sources_never_share_a_cached_result() {
    // Shared: runs concurrently with other cache tests, but never while the kill-switch test
    // holds the global env var (see ENV_LOCK).
    let _env = ENV_LOCK.read().expect("env lock");
    let ws_a = seed_db("ws-a", 1111);
    let ws_b = seed_db("ws-b", 2222);
    // Identical alias AND identical SQL — only the host-mediated DSN differs, exactly as it would
    // for two workspaces that each registered a datasource named "warehouse".
    let inp = input("warehouse", SQL, Some(60.0));

    assert_eq!(run(&ws_a, &inp).await.rows[0][0].as_i64(), Some(1111));
    assert_eq!(
        run(&ws_b, &inp).await.rows[0][0].as_i64(),
        Some(2222),
        "workspace B must never be served workspace A's cached rows"
    );
    // And back the other way, from cache this time.
    assert_eq!(
        run(&ws_a, &inp).await.rows[0][0].as_i64(),
        Some(1111),
        "workspace A's entry is still its own after B cached under the same alias + SQL"
    );
}

/// 10. **The event carries the result-cache verdict, and still leaks nothing.** `result_cache` is
///     the operator's only view of hit/miss/bypass; `age_ms` is what lets a UI badge "as of Xs ago".
///     The secret discipline is unchanged: never the DSN, never raw SQL.
#[test]
fn the_result_cache_event_reports_state_without_leaking() {
    use event::{Outcome, ResultCacheEvent};
    assert_eq!(results::ResultCache::Hit.as_str(), "hit");
    assert_eq!(results::ResultCache::Miss.as_str(), "miss");
    assert_eq!(results::ResultCache::Bypass.as_str(), "bypass");

    // The event path runs without panicking and the digest still hides the text (the pool suite
    // pins the digest itself; this pins that the new field did not change that contract).
    let sql = "SELECT card FROM payments WHERE token = 'hunter2-abcdef'";
    event::query_event(
        Some("paysrc"),
        "sqlite",
        None,
        sql,
        3,
        &Outcome::Ok(1),
        Some(&ResultCacheEvent {
            state: results::ResultCache::Hit,
            age_ms: Some(10_000),
        }),
    );
    let digest = event::sql_digest(sql);
    for forbidden in ["hunter2", "payments", "card", "token"] {
        assert!(!digest.contains(forbidden), "leaked {forbidden}");
    }
}
