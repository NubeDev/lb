//! The persisted last-GC-pass record (series-observability scope), against the real store: `run_gc`
//! stamps it on every pass, the row is UPSERTED (never appended), and it is workspace-scoped.
//!
//! **Load-bearing:** "GC ran and there was nothing to do" and "GC has not run" are different facts.
//! An idle pass that skipped the write would freeze `last_run_ms` and make a healthy node
//! indistinguishable from one whose retention reactor died — see
//! `idle_pass_still_stamps_last_run_ms`.

use lb_ingest::{
    commit_batch, last_pass, record_pass, run_gc, set_policy, write, GcPass, GcPassRecord, Policy,
    Qos, Sample, Tier, GC_PASS_TABLE, MAX_STORED_WARNINGS,
};
use lb_store::Store;
use serde_json::json;

fn sample(series: &str, producer: &str, seq: u64, ts: u64, payload: serde_json::Value) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts,
        seq,
        payload,
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

async fn seed(store: &Store, ws: &str, samples: Vec<Sample>) {
    write(store, ws, &samples, 0).await.unwrap();
    loop {
        // `drained()`, not `committed` — see `series_retention_test.rs`.
        if commit_batch(store, ws, 256).await.unwrap().drained() == 0 {
            break;
        }
    }
}

/// Seed 300 1s-cadence samples on `hist` under a policy that keeps 100s raw and rolls into 10s.
async fn seed_policed(store: &Store, ws: &str) {
    seed(
        store,
        ws,
        (0..300u64)
            .map(|i| sample("hist", "p", i + 1, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;
    set_policy(
        store,
        ws,
        &Policy {
            prefix: "hist".into(),
            raw_for_ms: 100_000,
            max_samples: 0,
            tiers: vec![Tier {
                width_ms: 10_000,
                keep_for_ms: 0,
                method: None,
                ..Default::default()
            }],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

/// How many rows the pass table actually holds in `ws` — the "never an append" probe.
async fn pass_row_count(store: &Store, ws: &str) -> i64 {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {GC_PASS_TABLE} GROUP ALL"),
            vec![],
        )
        .await
        .unwrap();
    let n: Option<i64> = resp.take("count").unwrap();
    n.unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_gc_records_the_pass_it_just_ran() {
    let store = Store::memory().await.unwrap();
    seed_policed(&store, "acme").await;

    assert_eq!(
        last_pass(&store, "acme").await.unwrap(),
        None,
        "a node that has never run GC honestly reports no pass — not a fabricated zero row"
    );

    let pass = run_gc(&store, "acme", 300_000).await.unwrap();
    let rec = last_pass(&store, "acme")
        .await
        .unwrap()
        .expect("one pass ran, so a record exists");
    assert_eq!(
        rec.last_run_ms, 300_000,
        "`last_run_ms` is the caller's `now_ms`, not wall-clock"
    );
    assert_eq!(rec.evicted_raw, pass.evicted_raw);
    assert_eq!(rec.rollup_rows, pass.rollup_rows);
    assert_eq!(rec.capped_raw, pass.capped_raw);
    assert_eq!(rec.evicted_rollup, pass.evicted_rollup);
    // `last_pass` projects BY NAME, so a counter added to `GcPassRecord` and forgotten here reads
    // back as its serde default forever while the row on disc is correct. That happened to
    // `capped_rollup` (rubix-ai#84): the pass returned 15, the operator's status read said 0.
    assert_eq!(rec.capped_rollup, pass.capped_rollup);
    assert!(
        rec.evicted_raw > 0 && rec.rollup_rows > 0,
        "this pass really did work: {rec:?}"
    );
}

/// UPGRADE, not fresh install: a pass row written by a build that predates `capped_rollup` must
/// still read back — and the whole retention pass must still run.
///
/// Found live on RC-6 (2026-08-06) immediately after deploying the `max_rows` build onto a node
/// with existing data. `last_pass` names its columns, so a pre-upgrade row returns `capped_rollup`
/// as a PRESENT `NONE`, which `#[serde(default)]` never sees and `usize` refuses:
///
/// ```text
/// expected a 64-bit unsigned integer, found None
/// ```
///
/// `run_gc` opens by reading `last_pass`, so that ONE stale row broke `series.retention.gc` AND
/// `series.retention.status` outright on the upgraded node — the identical failure
/// `Policy::max_samples` carries `none_as_default` to prevent. Every other test here writes its row
/// with the current struct, so none of them can see this.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pass_row_predating_capped_rollup_still_reads_and_gc_still_runs() {
    let store = Store::memory().await.unwrap();

    // EXACTLY what an older build left on disc: every column the old struct had, and no
    // `capped_rollup`. Written as raw SQL because the current struct cannot express its own absence.
    store
        .query_ws(
            "acme",
            "UPSERT type::thing('series_gc_pass', 'last') CONTENT {
               last_run_ms: 1000, duration_ms: 5, evicted_raw: 0, capped_raw: 0,
               rollup_rows: 0, evicted_rollup: 0, warnings: [], warnings_total: 0
             }",
            vec![],
        )
        .await
        .expect("seed a pre-upgrade pass row");

    let rec = last_pass(&store, "acme")
        .await
        .expect("a pre-upgrade row must not fail to deserialize")
        .expect("the row exists");
    assert_eq!(rec.last_run_ms, 1000, "the old row's data survives");
    assert_eq!(
        rec.capped_rollup, 0,
        "a column the old build never wrote reads as the unbounded default"
    );

    // The failure that actually bit: run_gc reads last_pass first, so a stale row broke the pass.
    run_gc(&store, "acme", 2000)
        .await
        .expect("GC must run on a workspace holding a pre-upgrade pass row");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_record_is_upserted_last_pass_only_never_appended() {
    let store = Store::memory().await.unwrap();
    seed_policed(&store, "acme").await;

    run_gc(&store, "acme", 300_000).await.unwrap();
    assert_eq!(
        last_pass(&store, "acme")
            .await
            .unwrap()
            .unwrap()
            .last_run_ms,
        300_000
    );

    run_gc(&store, "acme", 999_000).await.unwrap();
    let rec = last_pass(&store, "acme").await.unwrap().unwrap();
    assert_eq!(
        rec.last_run_ms, 999_000,
        "the SECOND pass is what `last_pass` reports"
    );

    // The guarantee that makes this bounded: one row per workspace, forever. A per-pass append would
    // grow ~10k rows/ws/year at the reactor's 300s cadence — an unbounded table in the subsystem
    // whose whole job is bounding growth.
    assert_eq!(
        pass_row_count(&store, "acme").await,
        1,
        "exactly ONE row in {GC_PASS_TABLE} after two passes"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn idle_pass_still_stamps_last_run_ms() {
    // making the `record_pass` call in `gc.rs` conditional on `evicted > 0` must turn THIS test red.
    let store = Store::memory().await.unwrap();
    // No policies, no samples: the pass has literally nothing to do.
    let pass = run_gc(&store, "idle-ws", 12_345).await.unwrap();
    assert_eq!(pass.evicted_raw, 0);
    assert_eq!(pass.rollup_rows, 0);

    let rec = last_pass(&store, "idle-ws")
        .await
        .unwrap()
        .expect("an idle pass IS a pass — a frozen last_run_ms reads as a dead reactor");
    assert_eq!(rec.last_run_ms, 12_345);
    assert_eq!(rec.evicted_raw, 0);
    assert_eq!(rec.capped_raw, 0);
    assert_eq!(rec.rollup_rows, 0);
    assert!(rec.warnings.is_empty());
    assert_eq!(rec.warnings_total, 0);

    // …and it keeps advancing while idle, which is the whole signal: an operator watching this field
    // learns the reactor is alive, not that data is being deleted.
    run_gc(&store, "idle-ws", 67_890).await.unwrap();
    assert_eq!(
        last_pass(&store, "idle-ws")
            .await
            .unwrap()
            .unwrap()
            .last_run_ms,
        67_890,
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pass_with_samples_but_no_policies_still_records() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        (1..=30u64)
            .map(|i| sample("unpoliced", "p", i, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;

    run_gc(&store, "acme", 500_000).await.unwrap();
    let rec = last_pass(&store, "acme").await.unwrap().unwrap();
    assert_eq!(rec.last_run_ms, 500_000);
    assert_eq!(rec.evicted_raw, 0, "no policy governs anything");
    assert_eq!(
        rec.warnings_total, 0,
        "30 samples is far under the advisory cap"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn warnings_are_clipped_but_the_total_stays_honest() {
    // The clip threshold is exercised through the REAL `GcPassRecord::new` + a REAL store round-trip.
    // It is not reachable via `run_gc` at test scale: a warning needs a series past
    // DEFAULT_MAX_SAMPLES (100k rows), so tripping MAX_STORED_WARNINGS would mean seeding >2M rows.
    let store = Store::memory().await.unwrap();
    let over = MAX_STORED_WARNINGS + 5;
    let pass = GcPass {
        warnings: (0..over)
            .map(|i| format!("series s{i} is unbounded"))
            .collect(),
        ..GcPass::default()
    };
    let rec = GcPassRecord::new(&pass, 7_000, 3);
    record_pass(&store, "acme", &rec).await.unwrap();

    let read_back = last_pass(&store, "acme").await.unwrap().unwrap();
    assert_eq!(
        read_back.warnings.len(),
        MAX_STORED_WARNINGS,
        "the stored list is clipped — the row is rewritten every tick and must not grow unboundedly wide"
    );
    assert_eq!(
        read_back.warnings_total, over,
        "…but the COUNT stays honest, so the clip is visible rather than a silent truncation"
    );
    assert_eq!(read_back.warnings[0], "series s0 is unbounded");
    assert_eq!(read_back, rec, "the record round-trips through the store");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_pass_record_is_workspace_scoped() {
    let store = Store::memory().await.unwrap();
    seed_policed(&store, "ws-a").await;
    seed_policed(&store, "ws-b").await;

    run_gc(&store, "ws-a", 300_000).await.unwrap();
    assert_eq!(
        last_pass(&store, "ws-a")
            .await
            .unwrap()
            .unwrap()
            .last_run_ms,
        300_000
    );
    assert_eq!(
        last_pass(&store, "ws-b").await.unwrap(),
        None,
        "ws-b has run no pass and must not read ws-a's"
    );

    run_gc(&store, "ws-b", 800_000).await.unwrap();
    assert_eq!(
        last_pass(&store, "ws-a")
            .await
            .unwrap()
            .unwrap()
            .last_run_ms,
        300_000,
        "ws-a's record is untouched by ws-b's pass"
    );
    assert_eq!(
        last_pass(&store, "ws-b")
            .await
            .unwrap()
            .unwrap()
            .last_run_ms,
        800_000
    );
    assert_eq!(pass_row_count(&store, "ws-a").await, 1);
    assert_eq!(pass_row_count(&store, "ws-b").await, 1);
}
