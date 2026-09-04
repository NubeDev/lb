//! **What does the staging hop cost?** `#[ignore]`d — ~2 min, and it measures rather than asserts.
//! `cargo test -p lb-ingest --test staging_cost_bench -- --ignored --nocapture`
//!
//! Read `direct.rs` first: `commit_direct` ALREADY exists and the host ALREADY prefers it — a batch
//! goes direct whenever staging is empty, and stages only behind a backlog. So this is not "staging
//! vs a hypothetical", it is "the backlog path vs the path production already takes".
//!
//! Same 20,000 samples, same batch size, same engine:
//!
//! | arm | time | on disc |
//! |---|---|---|
//! | `write()` + drain, as shipped | 115,398 ms | 11,873,949 B |
//! | the same, appends batched | 9,025 ms | 9,838,861 B |
//! | `commit_direct` | 3,752 ms | 4,023,639 B |
//!
//! Three findings:
//!
//! 1. **`write()` commits once per sample.** `append_one` issues its own `query_ws`, so 20,000
//!    samples are 20,000 transactions — `write()`=108.1s against `drain`=7.3s, 93% of the cost. At
//!    ~5.4 ms each that is a durability barrier per sample, not a memtable insert. **12.8x**, and a
//!    bug in the staging path rather than a property of staging.
//! 2. **Even batched, the hop costs 2.4x the time and 2.4x the disc.** It cannot be otherwise: a
//!    staged sample is three log appends (staging UPSERT, series UPSERT, staging DELETE tombstone)
//!    where direct is one.
//! 3. **The extra duties are free.** An earlier arm hand-rolled a bare series UPSERT with no
//!    cardinality gate, no filters and no `series_latest` upkeep: 3,163 ms / 4,009,581 B against
//!    `commit_direct`'s 3,752 ms / 4,023,639 B. So the indexed commit is not where the cost is, and
//!    "staging defers the expensive part" does not survive measurement on an LSM — index entries are
//!    just more keys in the same memtable and WAL.
//!
//! **Caveats.** One machine, one producer, one series, no concurrency, an SSD not an SD card. This
//! measures throughput, NOT what staging is actually for on the shipped path: absorbing a burst that
//! arrives faster than the commit can drain, and holding ordering behind an existing backlog.
//! Removing it is a decision about backpressure, not about these numbers.

use std::time::Instant;

use lb_ingest::{commit_batch, ensure_series_schema, write, Qos, Sample};
use lb_store::Store;
use serde_json::json;

const N: u64 = 20_000;
const BATCH: usize = 2_000;

fn samples(series: &str, from: u64, n: u64) -> Vec<Sample> {
    (from..from + n)
        .map(|i| Sample {
            series: series.into(),
            producer: "p".into(),
            seq: i,
            ts: i * 1_000,
            payload: json!(i),
            labels: json!({}),
            qos: Qos::BestEffort,
        })
        .collect()
}

fn dir_bytes(p: &std::path::Path) -> u64 {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                n += if m.is_dir() {
                    dir_bytes(&e.path())
                } else {
                    m.len()
                };
            }
        }
    }
    n
}

/// STAGED: the shipped path — write() into staging, then drain into series.
async fn staged(dir: &str) -> (u128, u64) {
    let store = Store::open(dir).await.unwrap();
    ensure_series_schema(&store, "w").await.unwrap();
    let t = Instant::now();
    let (mut w_ms, mut c_ms, mut passes) = (0u128, 0u128, 0u64);
    for chunk in 0..(N / BATCH as u64) {
        let s = samples("bench", chunk * BATCH as u64, BATCH as u64);
        let tw = Instant::now();
        write(&store, "w", &s, 0).await.unwrap();
        w_ms += tw.elapsed().as_millis();
        let tc = Instant::now();
        while commit_batch(&store, "w", BATCH).await.unwrap().drained() > 0 {
            passes += 1;
        }
        c_ms += tc.elapsed().as_millis();
    }
    println!("BENCH split : write()={w_ms} ms  drain={c_ms} ms  drain_passes={passes}");
    let ms = t.elapsed().as_millis();
    drop(store);
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    (ms, dir_bytes(std::path::Path::new(dir)))
}

/// DIRECT: the path production ALREADY takes when staging is empty — `commit_direct`, which runs
/// the same transaction builder as the drain (same cardinality gate, same filters, same
/// `series_latest` pointer) and differs only in having no staged row to delete.
async fn direct(dir: &str) -> (u128, u64) {
    let store = Store::open(dir).await.unwrap();
    ensure_series_schema(&store, "w").await.unwrap();
    let t = Instant::now();
    for chunk in 0..(N / BATCH as u64) {
        let s = samples("bench", chunk * BATCH as u64, BATCH as u64);
        for part in s.chunks(lb_ingest::DIRECT_COMMIT_BATCH) {
            lb_ingest::commit_direct(&store, "w", part).await.unwrap();
        }
    }
    let ms = t.elapsed().as_millis();
    drop(store);
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    (ms, dir_bytes(std::path::Path::new(dir)))
}

/// STAGED, BATCHED: the same staging hop, but the appends go in ONE transaction per chunk instead
/// of one per sample. Isolates "does the staging hop cost anything" from "is it unbatched".
async fn staged_batched(dir: &str) -> (u128, u64) {
    let store = Store::open(dir).await.unwrap();
    ensure_series_schema(&store, "w").await.unwrap();
    let t = Instant::now();
    for chunk in 0..(N / BATCH as u64) {
        let s = samples("bench", chunk * BATCH as u64, BATCH as u64);
        let mut sql = String::from("BEGIN TRANSACTION;");
        let mut binds: Vec<(String, serde_json::Value)> = Vec::new();
        for (i, smp) in s.iter().enumerate() {
            sql.push_str(&format!(
                " UPSERT type::record('ingest_staging', [$se{i}, $pr{i}, $sq{i}]) CONTENT $rw{i};"
            ));
            binds.push((format!("se{i}"), json!(smp.series)));
            binds.push((format!("pr{i}"), json!(smp.producer)));
            binds.push((format!("sq{i}"), json!(smp.seq)));
            binds.push((format!("rw{i}"), json!({ "sample": smp })));
        }
        sql.push_str(" COMMIT TRANSACTION;");
        store.query_ws("w", &sql, binds).await.unwrap();
        while commit_batch(&store, "w", BATCH).await.unwrap().drained() > 0 {}
    }
    let ms = t.elapsed().as_millis();
    drop(store);
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    (ms, dir_bytes(std::path::Path::new(dir)))
}

#[ignore = "~2 min; a measurement, not an assertion"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn staged_vs_direct() {
    let base = std::env::temp_dir().join(format!("lb-stagebench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let a = base.join("staged");
    let b = base.join("direct");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();

    let c = base.join("staged_batched");
    std::fs::create_dir_all(&c).unwrap();
    let (sm, sb) = staged(a.to_str().unwrap()).await;
    let (bm, bb) = staged_batched(c.to_str().unwrap()).await;
    let (dm, db) = direct(b.to_str().unwrap()).await;

    println!("BENCH N={N} batch={BATCH}");
    println!("BENCH staged : {sm:>6} ms   {sb:>10} bytes on disc");
    println!("BENCH stgbatch:{bm:>6} ms   {bb:>10} bytes on disc  (staging hop, batched appends)");
    println!("BENCH direct : {dm:>6} ms   {db:>10} bytes on disc");
    println!(
        "BENCH direct is {:.2}x the staged TIME, {:.2}x the staged BYTES",
        dm as f64 / sm as f64,
        db as f64 / sb as f64
    );
    let _ = std::fs::remove_dir_all(&base);
}
