//! Read-concurrency tests (federation-read-concurrency scope §Testing plan). Real seeded SQLite
//! files, real engine, no mocks (testing-scope §0).
//!
//! The bug these pin: the upstream `SqliteConnectionPool` is a SINGLE tokio-rusqlite connection and
//! every cached `TableProvider` is bound to it, so N concurrent reads on one source funnelled into
//! one connection and collapsed into a serial staircase (measured live: 4 identical scans =
//! 228/444/654/865 ms, wall ≈ the serial sum; a 10-panel `viz.query_batch` measured ~3.5 s and the
//! host's 16-permit semaphore bought nothing). `SqliteSource` now holds `READ_SLOTS` independent
//! pools, each with its own provider cache, round-robined per read.
//!
//! What each test pins, and what breaks it:
//!   1. `concurrent_reads_do_not_serialize` — **the headline.** Set `READ_SLOTS = 1` in
//!      `source/sqlite.rs` and it goes red (that is the mutation check; the serial behavior measures
//!      ≈N× the single-query cost, and the assertion allows 2×, so the gap is wide and honest).
//!   2. `concurrent_reads_return_their_own_rows` — no cross-talk between slots' provider caches.
//!      Share one cache across slots (or key it wrongly) and predicates start returning each other's
//!      rows.
//!   3. `connect_builds_exactly_one_slot` — slots are LAZY. Build all K eagerly in `connect` and
//!      this goes red: a source serving serial traffic must not pay for K connections.
//!
//! Test 1 is the only timing-based assertion in the file, and it is written to be robust on a loaded
//! CI box: it compares CONCURRENT wall against a MEASURED serial baseline from the same process, so
//! it scales with whatever the machine is doing rather than pinning an absolute millisecond budget.

use std::sync::Arc;
use std::time::Instant;

// The crate is a binary, so the test binary composes the modules it needs directly (same pattern as
// `pool_cache_test.rs`).
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/event.rs"]
mod event;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/info_schema.rs"]
mod info_schema;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/pool.rs"]
mod pool;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/query.rs"]
mod query;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/results.rs"]
mod results;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/source/mod.rs"]
mod source;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/validate.rs"]
mod validate;

use source::{Source, SqliteSource, READ_SLOTS};

/// How many rows the scan table carries. Large enough that a full scan + aggregate costs real,
/// measurable time (the staircase is only observable when the unit cost dominates scheduling
/// noise), small enough to seed in a second or two.
const ROWS: i64 = 120_000;

/// Seed a real SQLite file with a table big enough that a full scan measurably costs. `bucket`
/// partitions the rows so concurrent readers can each select a distinct slice (test 2).
fn seed_scan_db(tag: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("lb-fed-conc-{tag}-{seq}-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE reading (id INTEGER, bucket INTEGER, value REAL);",
    )
    .expect("create schema");
    {
        let tx = conn.unchecked_transaction().expect("tx");
        let mut stmt = tx
            .prepare("INSERT INTO reading (id, bucket, value) VALUES (?1, ?2, ?3)")
            .expect("prepare seed");
        for i in 0..ROWS {
            // 4 buckets so each of the 4 concurrent readers can own one.
            stmt.execute(rusqlite::params![i, i % 4, (i as f64) * 1.5])
                .expect("seed row");
        }
        drop(stmt);
        tx.commit().expect("commit seed");
    }
    path.to_string_lossy().into_owned()
}

/// One full-scan aggregate — the unit of work whose serialization this scope removes. Deliberately
/// an aggregate over every row so the cost lands in the sqlite scan, not in row marshalling.
const SCAN_SQL: &str = "SELECT SUM(value) AS total FROM reading";

/// Run one scan through the source's own read path (`query_direct` → the slot's pool → sqlite).
/// Both read entries — `query_direct` and the `table_provider` a panel query drives — round-robin
/// over the same slots, so this measures the serialization the scope removes without dragging a
/// DataFusion `SessionContext` into the timing.
async fn scan(src: &Arc<SqliteSource>) -> f64 {
    let batches = src.query_direct(SCAN_SQL).await.expect("scan must succeed");
    let b = batches.first().expect("one batch");
    let col = b.column(0);
    let arr = col
        .as_any()
        .downcast_ref::<arrow::array::Float64Array>()
        .expect("SUM(value) is f64");
    arr.value(0)
}

/// 1. THE HEADLINE. N concurrent reads on one source cost ≈ the slowest one, not the sum.
///
/// Method: measure a serial baseline (2 sequential scans, warm) in this same process, then run 4
/// scans concurrently and assert the concurrent wall is well under what a serial staircase would
/// cost. With `READ_SLOTS = 1` the 4 scans serialize to ≈4× the unit cost; the assertion's ceiling
/// is 2× the unit cost, so the honest gap between pass and fail is large.
///
/// **Mutation check:** set `READ_SLOTS = 1` and watch this go red.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reads_do_not_serialize() {
    let dsn = seed_scan_db("staircase");
    let src = Arc::new(SqliteSource::connect(&dsn).await.expect("connect"));

    // Warm: the first scan pays provider/schema build on slot 0. Measure AFTER that, so the baseline
    // is pure scan cost and the comparison is apples-to-apples.
    let _ = scan(&src).await;

    let t = Instant::now();
    let _ = scan(&src).await;
    let _ = scan(&src).await;
    let serial_two = t.elapsed();
    let unit = serial_two / 2;

    // 4 concurrent scans. Each lands on its own slot (round-robin over K=4).
    let t = Instant::now();
    let results = futures::future::join_all((0..4).map(|_| {
        let src = Arc::clone(&src);
        async move { scan(&src).await }
    }))
    .await;
    let concurrent_wall = t.elapsed();

    // Every scan returned the real aggregate — concurrency must not corrupt results.
    let expected: f64 = (0..ROWS).map(|i| (i as f64) * 1.5).sum();
    for r in &results {
        assert!(
            (r - expected).abs() < 1.0,
            "a concurrent scan returned {r}, expected {expected} — results corrupted under concurrency"
        );
    }

    // The assertion. Serial would be ≈4×unit; we allow 2×unit for scheduling noise + the K-th
    // slot's first-touch provider build, which is paid inside this very measurement.
    let ceiling = unit * 2;
    assert!(
        concurrent_wall < ceiling,
        "4 concurrent scans took {concurrent_wall:?}, which is not meaningfully better than \
         serializing them (unit={unit:?}, ceiling={ceiling:?}). Reads are funnelling onto one \
         connection — the read-slot round-robin is not working."
    );
}

/// 2. Correctness under concurrency: N concurrent queries with DISTINCT predicates each return
///    exactly their own rows. A shared/miskeyed provider cache across slots would let one reader's
///    plan serve another's rows — the worst failure mode for this change.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reads_return_their_own_rows() {
    let dsn = seed_scan_db("distinct");
    let src = Arc::new(SqliteSource::connect(&dsn).await.expect("connect"));

    // Each reader owns one bucket; the expected count per bucket is ROWS/4.
    let results = futures::future::join_all((0..4i64).map(|bucket| {
        let src = Arc::clone(&src);
        async move {
            let sql = format!("SELECT COUNT(*) AS n FROM reading WHERE bucket = {bucket}");
            let batches = src.query_direct(&sql).await.expect("bucket scan");
            let b = batches.first().expect("one batch");
            let arr = b
                .column(0)
                .as_any()
                .downcast_ref::<arrow::array::Int64Array>()
                .expect("COUNT(*) is i64");
            (bucket, arr.value(0))
        }
    }))
    .await;

    for (bucket, n) in results {
        assert_eq!(
            n,
            ROWS / 4,
            "bucket {bucket} returned {n} rows, expected {} — slots are crossing results",
            ROWS / 4
        );
    }
}

/// 3. Slots are LAZY — `connect` builds exactly ONE connection.
///
/// This is the cost half of the trade: K connections must be paid for only when real concurrency
/// exists to pay them off. Build all K eagerly in `connect` and this goes red immediately.
///
/// Note this asserts the state right after `connect`, not after N calls: the round-robin cursor
/// advances per read regardless of concurrency, so a source that serves K serial reads does
/// eventually build K slots. That is intended (they are then warm and reusable), and asserting
/// otherwise would pin the round-robin policy rather than the laziness this test is about.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_builds_exactly_one_slot() {
    let dsn = seed_scan_db("lazy");
    let src = SqliteSource::connect(&dsn).await.expect("connect");

    assert_eq!(
        src.built_slots(),
        1,
        "connect must build exactly slot 0 and leave the other {} lazy — a source that never sees \
         concurrent traffic must not pay for {} connections",
        READ_SLOTS - 1,
        READ_SLOTS
    );

    // And a single read still only needs the one slot it lands on (it lands on slot 0 first).
    let _ = src.query_direct(SCAN_SQL).await.expect("one scan");
    assert_eq!(
        src.built_slots(),
        1,
        "one serial read must not build a second slot"
    );
}
