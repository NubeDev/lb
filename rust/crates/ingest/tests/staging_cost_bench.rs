//! **What does the staging hop actually cost?** `#[ignore]`d — ~2 min, and it measures rather than
//! asserts. Run with `cargo test -p lb-ingest --test staging_cost_bench -- --ignored --nocapture`.
//!
//! Staging exists to make a producer's write CHEAP: land it in a table with no secondary index, no
//! rollup-view upkeep and no tag edges, and pay the indexed write once per batch at the commit. This
//! measures whether that trade pays. Three arms, same 20,000 samples, same batch size, same engine:
//!
//! | arm | time | on disc |
//! |---|---|---|
//! | staged, as shipped | 110,502 ms | 11,873,746 B |
//! | staged, appends batched | 8,837 ms | 9,838,675 B |
//! | direct into `series` | 3,163 ms | 4,009,581 B |
//!
//! Two separate findings, and they want different fixes:
//!
//! 1. **`write()` commits once per sample.** `append_one` issues its own `query_ws` per sample, so
//!    20,000 samples are 20,000 transactions. That alone is **12.5x** (110.5s → 8.8s). It is a bug in
//!    the staging path, not a property of staging, and the split timing says where: `write()`=103.3s
//!    against `drain`=7.2s, so 93% of the cost is the append loop.
//! 2. **Even batched, the hop still costs 2.8x the time and 2.5x the disc.** Staging does not avoid
//!    the indexed write; it adds a write and a tombstone in front of it. The disc figure is the one
//!    that matters for an edge node buffering through a partition — 2.5x the bytes is 2.5x less
//!    survivable time on the same card.
//!
//! **Caveat, so the numbers are not over-read.** The `direct` arm is deliberately minimal: it skips
//! cap enforcement, dead-letter diversion, `series_latest` maintenance and `series_meta`
//! registration, all of which the real commit does. Its true cost is therefore higher than 3,163 ms
//! and the gap narrower than 2.8x. Single producer, one series, no concurrency, a fast SSD — not an
//! SD card. The direction and order of magnitude are solid; the exact ratio is not.

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

/// DIRECT: the same rows, same batching, straight into `series` — no staging hop.
async fn direct(dir: &str) -> (u128, u64) {
    let store = Store::open(dir).await.unwrap();
    ensure_series_schema(&store, "w").await.unwrap();
    let t = Instant::now();
    for chunk in 0..(N / BATCH as u64) {
        let s = samples("bench", chunk * BATCH as u64, BATCH as u64);
        let mut sql = String::from("BEGIN TRANSACTION;");
        let mut binds: Vec<(String, serde_json::Value)> = Vec::new();
        for (i, smp) in s.iter().enumerate() {
            sql.push_str(&format!(
                " UPSERT type::record('series', [$se{i}, $pr{i}, $sq{i}]) CONTENT \
                 {{ series: $se{i}, producer: $pr{i}, seq: $sq{i}, \
                    ts: time::from_millis($ts{i}), payload: $pl{i} }} RETURN NONE;"
            ));
            binds.push((format!("se{i}"), json!(smp.series)));
            binds.push((format!("pr{i}"), json!(smp.producer)));
            binds.push((format!("sq{i}"), json!(smp.seq)));
            binds.push((format!("ts{i}"), json!(smp.ts)));
            binds.push((format!("pl{i}"), smp.payload.clone()));
        }
        sql.push_str(" COMMIT TRANSACTION;");
        store.query_ws("w", &sql, binds).await.unwrap();
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
