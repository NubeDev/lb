//! MEASUREMENT, not an assertion of behaviour: how many bytes ON DISC does one stored rollup row
//! actually cost?
//!
//! `Tier::max_rows` is only useful if an operator can turn a row count into a disc budget, and the
//! sizing tables in `docs/scope/ingest/rollup-row-cap-scope.md` (and rubix-ai#84) rested on a single
//! ~400 B estimate eyeballed from RC-6. This writes a real tier to a real file-backed SurrealKV
//! store, compacts it, and stats the directory, so the number in those tables comes from a
//! measurement on this hardware instead of a guess.
//!
//! It is `#[ignore]`d: it writes ~10k rows and compacts, which is far too slow for the default
//! suite, and a byte count is environment-dependent — asserting on it would make an honest
//! measurement into a flaky test. Run it deliberately:
//!
//! ```text
//! cargo test -p lb-ingest --test series_rollup_row_size_test -- --ignored --nocapture
//! ```

use lb_ingest::{write_rollups, RollupRow};
use lb_store::{compact, Store};
use serde_json::json;

const WIDTH: u64 = 900_000; // 15-minute buckets — the modbus sizing target's grid
const ROWS: u64 = 10_000;

/// Total bytes of every file under `dir`, recursively.
fn dir_bytes(dir: &std::path::Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for e in entries.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        total += if meta.is_dir() {
            dir_bytes(&e.path())
        } else {
            meta.len()
        };
    }
    total
}

/// A row shaped like one `rollup_series` actually writes: every stat column populated, a float
/// payload in `last`/`first`. A row with `None` stats would under-measure the real thing.
fn rollup_row(series: &str, t: u64) -> RollupRow {
    RollupRow {
        series: series.into(),
        width_ms: WIDTH,
        t,
        min: Some(18.5),
        max: Some(23.75),
        sum: 1234.5,
        num_count: 60,
        count: 60,
        last: json!(21.25),
        last_ts: t + WIDTH - 1,
        first: json!(19.0),
        first_ts: Some(t),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "measurement: writes 10k rows + compacts; run with --ignored --nocapture"]
async fn measure_rollup_row_bytes_on_disc() {
    let dir = std::env::temp_dir().join(format!("lb-rollup-size-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.to_str().unwrap().to_string();

    // An empty-but-open store is the BASELINE: SurrealKV writes its own scaffolding at open, and
    // charging that to the rows would inflate the per-row number.
    let store = Store::open(&path).await.unwrap();
    // Touch the workspace namespace so the baseline includes the namespace, not just the engine.
    write_rollups(&store, "acme", &[rollup_row("sizing.warmup", 0)])
        .await
        .unwrap();
    compact(&store).await.unwrap();
    let baseline = dir_bytes(&dir);

    // One series, one tier, ROWS rows on the tier grid — exactly what a capped tier holds.
    let rows: Vec<RollupRow> = (1..=ROWS)
        .map(|i| rollup_row("sizing.point", i * WIDTH))
        .collect();
    // Chunked: one 10k-row statement is a different write shape than a node's incremental passes.
    for chunk in rows.chunks(500) {
        write_rollups(&store, "acme", chunk).await.unwrap();
    }

    let before_compaction = dir_bytes(&dir);
    let record = compact(&store).await.unwrap();
    let after_compaction = dir_bytes(&dir);

    let grew = after_compaction.saturating_sub(baseline);
    let per_row = grew as f64 / ROWS as f64;
    let per_row_uncompacted = before_compaction.saturating_sub(baseline) as f64 / ROWS as f64;

    println!("\n=== rollup row size on disc ({ROWS} rows, {WIDTH} ms buckets) ===");
    println!("baseline (open + 1 row + compact) : {baseline:>12} B");
    println!("after {ROWS} rows, pre-compaction  : {before_compaction:>12} B");
    println!("after compaction                  : {after_compaction:>12} B");
    println!("compaction record                 : {record:?}");
    println!("--> per row, COMPACTED            : {per_row:>12.1} B");
    println!("--> per row, append-log worst case: {per_row_uncompacted:>12.1} B");
    println!("\nworst case for a max_rows tier (compacted):");
    for series in [1u64, 100, 1800] {
        for max_rows in [672u64, 2880] {
            let mb = (per_row * (series * max_rows) as f64) / 1_048_576.0;
            println!("  {series:>4} series x max_rows {max_rows:>5} = {mb:>9.1} MiB");
        }
    }

    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}
