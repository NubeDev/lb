//! The LIVE measurement for the federation-read-concurrency scope — the same rig the scope's
//! before-numbers were taken on, so the after-numbers are comparable rather than merely green.
//!
//! `#[ignore]` by design: it needs the seeded `demo-buildings` sqlite (5.8 M-row `point_reading`)
//! that only a dev box carries, and it is a MEASUREMENT, not an assertion — CI runs
//! `read_concurrency_test.rs` (which pins the behavior on a fixture it seeds itself). Run it by hand:
//!
//! ```text
//! LB_BENCH_SQLITE=/path/to/buildings.db \
//!   cargo test -p federation --test read_concurrency_live_bench -- --ignored --nocapture
//! ```
//!
//! It prints the staircase table the scope quotes: one scan alone, then 4 and 10 concurrent, so the
//! "wall ≈ sum" (before) vs "wall ≈ max" (after) shape is visible directly. Flip `READ_SLOTS` to 1 in
//! `source/sqlite.rs` and re-run to reproduce the before-column on this machine.

use std::sync::Arc;
use std::time::Instant;

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

/// A panel-shaped read: a bucketed aggregate over the big table — what a dashboard timeseries cell
/// actually asks for, and the query whose ~240 ms unit cost the scope measured.
const PANEL_SQL: &str = "SELECT point_id, COUNT(*) AS n, AVG(value) AS avg_value \
                         FROM point_reading GROUP BY point_id LIMIT 50";

async fn scan(src: &Arc<SqliteSource>) {
    src.query_direct(PANEL_SQL).await.expect("panel scan");
}

/// Run `n` scans concurrently and report the wall clock.
async fn concurrent(src: &Arc<SqliteSource>, n: usize) -> std::time::Duration {
    let t = Instant::now();
    futures::future::join_all((0..n).map(|_| {
        let src = Arc::clone(src);
        async move { scan(&src).await }
    }))
    .await;
    t.elapsed()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "measurement against a dev-box seeded database; set LB_BENCH_SQLITE"]
async fn measure_the_staircase() {
    let dsn = std::env::var("LB_BENCH_SQLITE")
        .expect("set LB_BENCH_SQLITE to the seeded demo-buildings sqlite file");
    let src = Arc::new(SqliteSource::connect(&dsn).await.expect("connect"));

    // Warm slot 0's provider/schema build so the numbers are steady-state, exactly as a warm board is.
    scan(&src).await;

    let t = Instant::now();
    scan(&src).await;
    let unit = t.elapsed();

    let four = concurrent(&src, 4).await;
    let ten = concurrent(&src, 10).await;

    println!("\n=== federation read concurrency (READ_SLOTS = {READ_SLOTS}) ===");
    println!("  1 scan alone      : {unit:?}");
    println!(
        "  4 concurrent, wall: {four:?}   (serial would be ~{:?})",
        unit * 4
    );
    println!(
        " 10 concurrent, wall: {ten:?}   (serial would be ~{:?})",
        unit * 10
    );
    println!("  slots built       : {}\n", src.built_slots());
}
