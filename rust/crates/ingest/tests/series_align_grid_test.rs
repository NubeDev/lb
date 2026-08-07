//! **The test this feature exists to make possible.** `bucket_start` (reads) and `rollup_series`
//! (the GC fold) must agree on the grid. If a read floors differently from the way the GC folded, a
//! bucket read silently mixes two griddings — stored rows land in buckets whose boundaries the read
//! invented — and NOTHING errors: no exception, no warning, just quietly wrong history.
//!
//! So this file never asserts on the arithmetic. It seeds raw through the real write path, runs a
//! REAL `run_gc`, and then reads the buckets back through BOTH read paths (the pushed-down `GROUP
//! BY` and the in-Rust fold oracle), asserting the two agree with each other AND with the `t` values
//! actually on disc.
//!
//! Every test here also carries its own revert-check: reading the same data on the epoch grid must
//! produce DIFFERENT boundaries. Without that, a read that ignored alignment entirely would pass
//! every assertion below — which is precisely the bug being guarded.

use lb_ingest::{
    bucket_start, commit_batch, read_buckets, read_buckets_fold, read_rollups, run_gc, set_policy,
    write, Align, Bucket, BucketQuery, Policy, Qos, Sample, Tier,
};
use lb_store::Store;
use serde_json::json;

const MIN: u64 = 60_000;
const HOUR: u64 = 3_600_000;
const DAY: u64 = 86_400_000;
/// 2026-07-27T00:00:00Z. A real instant, because "local midnight" is meaningless at epoch 0.
const DAY0: u64 = 1_785_110_400_000;
const WS: &str = "nube";

fn sample(series: &str, seq: u64, ts: u64, v: f64) -> Sample {
    Sample {
        series: series.into(),
        producer: "p".into(),
        ts,
        seq,
        payload: json!(v),
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

/// Seed through the REAL write path (staging → drained commit), never a direct row insert.
async fn seed(store: &Store, series: &str, count: u64, step_ms: u64) {
    let samples: Vec<Sample> = (0..count)
        .map(|i| sample(series, i + 1, DAY0 + i * step_ms, i as f64))
        .collect();
    write(store, WS, &samples, 0).await.unwrap();
    while commit_batch(store, WS, 256).await.unwrap().drained() != 0 {}
}

async fn read_both(store: &Store, series: &str, q: &BucketQuery, width: u64) -> Vec<Bucket> {
    let pushed = read_buckets(store, WS, series, q, width).await.unwrap();
    let folded = read_buckets_fold(store, WS, series, q, width)
        .await
        .unwrap();
    assert_eq!(
        starts(&pushed),
        starts(&folded),
        "the pushdown and the fold oracle disagree about where buckets start"
    );
    for (p, f) in pushed.iter().zip(folded.iter()) {
        assert_eq!(
            (p.count, p.min, p.max),
            (f.count, f.min, f.max),
            "at t={}",
            p.t
        );
    }
    pushed
}

fn starts(b: &[Bucket]) -> Vec<u64> {
    b.iter().map(|x| x.t).collect()
}

/// A 90-minute tier anchored at 06:30 — a width that does NOT divide a day, at an offset that is not
/// a whole number of widths from the epoch. Nothing about this grid is reachable by epoch anchoring,
/// so every boundary below is evidence.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_fold_and_a_read_land_on_the_same_declared_grid() {
    let store = Store::memory().await.unwrap();
    let series = "align.grid";
    // 6 hours of raw at 1-minute cadence.
    seed(&store, series, 360, MIN).await;

    let width = 90 * MIN;
    let align = Align {
        origin_ms: 6 * HOUR as i64 + 30 * MIN as i64,
    };
    set_policy(
        &store,
        WS,
        &Policy {
            prefix: "align.".into(),
            raw_for_ms: 2 * HOUR,
            tiers: vec![Tier {
                width_ms: width,
                keep_for_ms: 30 * DAY,
                align: Some(align),
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // A real pass at a real clock: the horizon lands at 04:00Z, which is NOT on the tier's grid, so
    // the fold must snap back to 03:30Z of its own accord.
    let now_ms = DAY0 + 6 * HOUR;
    let pass = run_gc(&store, WS, now_ms).await.unwrap();
    assert!(
        pass.rollup_rows > 0,
        "the pass folded nothing — the rest proves nothing"
    );
    assert!(
        pass.evicted_raw > 0,
        "the pass evicted no raw — the rest proves nothing"
    );

    // 1. What is ON DISC is on the declared grid, not the epoch one.
    let stored = read_rollups(&store, WS, series, 0, DAY0 + 90 * DAY)
        .await
        .unwrap();
    assert!(!stored.is_empty());
    for row in &stored {
        assert_eq!(
            row.t,
            bucket_start(row.t, width, Some(align)),
            "stored bucket {} is not a boundary of the tier's own grid",
            row.t
        );
        assert_ne!(
            row.t % width,
            0,
            "bucket {} is ALSO an epoch boundary — this fixture cannot tell the two grids apart",
            row.t
        );
    }

    // 2. A read on the tier's grid agrees with the fold, through BOTH read paths.
    let q = BucketQuery {
        from_ts: DAY0 - width,
        to_ts: now_ms,
        width_ms: Some(width),
        align: Some(align),
        ..Default::default()
    };
    let buckets = read_both(&store, series, &q, width).await;
    for b in &buckets {
        assert_eq!(
            b.t,
            bucket_start(b.t, width, Some(align)),
            "read bucket {} off-grid",
            b.t
        );
    }
    // Every stored row's bucket appears in the read at exactly its own `t` — the join that silently
    // fails when the two paths floor differently.
    for row in &stored {
        assert!(
            buckets.iter().any(|b| b.t == row.t),
            "stored bucket {} has no bucket at that start in the read — the grids diverged",
            row.t
        );
    }
    // No sample was lost or double-counted across the rollup/raw seam.
    assert_eq!(
        buckets.iter().map(|b| b.count).sum::<u64>(),
        360,
        "the merged read must account for every seeded sample exactly once"
    );

    // 3. REVERT-CHECK. On the epoch grid the same data buckets differently — so a read that ignored
    // the tier's alignment would NOT have passed the assertions above.
    let epoch_read = read_buckets(
        &store,
        WS,
        series,
        &BucketQuery {
            align: None,
            ..q.clone()
        },
        width,
    )
    .await
    .unwrap();
    assert_ne!(
        starts(&epoch_read),
        starts(&buckets),
        "the epoch grid produced identical boundaries — this test cannot detect a read that \
         ignores alignment"
    );
}

/// The flagship case: a DAILY tier at local midnight for a site at UTC+10. Epoch anchoring cannot
/// express it at all — a "daily" bucket would run 10:00→10:00 local.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_daily_tier_can_start_at_local_midnight() {
    let store = Store::memory().await.unwrap();
    let series = "align.daily";
    // 3 days of raw at 30-minute cadence, starting 2026-07-27T00:00:00Z.
    seed(&store, series, 144, 30 * MIN).await;

    let align = Align {
        origin_ms: -10 * HOUR as i64,
    }; // UTC+10 → the local day starts 14:00Z
    set_policy(
        &store,
        WS,
        &Policy {
            prefix: "align.".into(),
            raw_for_ms: 6 * HOUR,
            tiers: vec![Tier {
                width_ms: DAY,
                keep_for_ms: 90 * DAY,
                align: Some(align),
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    run_gc(&store, WS, DAY0 + 3 * DAY).await.unwrap();

    let stored = read_rollups(&store, WS, series, 0, DAY0 + 90 * DAY)
        .await
        .unwrap();
    assert!(!stored.is_empty(), "nothing folded");
    for row in &stored {
        assert_eq!(
            (row.t % DAY) / HOUR,
            14,
            "a local-midnight daily bucket must start at 14:00Z, not {row}",
            row = row.t
        );
    }

    // And the read agrees — through both paths, on the same boundaries.
    let q = BucketQuery {
        from_ts: DAY0 - DAY,
        to_ts: DAY0 + 3 * DAY,
        width_ms: Some(DAY),
        align: Some(align),
        ..Default::default()
    };
    let buckets = read_both(&store, series, &q, DAY).await;
    for b in &buckets {
        assert_eq!(
            (b.t % DAY) / HOUR,
            14,
            "read bucket {} is not local midnight",
            b.t
        );
    }
    assert_eq!(buckets.iter().map(|b| b.count).sum::<u64>(), 144);
}

/// A policy with NO alignment must behave byte-identically to the way it did before this slice —
/// the whole feature is additive or it is a regression.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unaligned_policy_still_buckets_on_the_epoch_grid() {
    let store = Store::memory().await.unwrap();
    let series = "plain.grid";
    seed(&store, series, 360, MIN).await;

    let width = 15 * MIN;
    set_policy(
        &store,
        WS,
        &Policy {
            prefix: "plain.".into(),
            raw_for_ms: 2 * HOUR,
            tiers: vec![Tier {
                width_ms: width,
                keep_for_ms: 30 * DAY,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    run_gc(&store, WS, DAY0 + 6 * HOUR).await.unwrap();

    for row in read_rollups(&store, WS, series, 0, DAY0 + 90 * DAY)
        .await
        .unwrap()
    {
        assert_eq!(
            row.t % width,
            0,
            "an unaligned tier must fold on the epoch grid"
        );
    }
    let q = BucketQuery {
        from_ts: DAY0,
        to_ts: DAY0 + 6 * HOUR,
        width_ms: Some(width),
        ..Default::default()
    };
    for b in read_both(&store, series, &q, width).await {
        assert_eq!(b.t % width, 0);
    }
}

/// Two tiers on DIFFERENT grids, folded in one pass. Each must snap to its own boundary, and raw
/// must not be evicted past the least-advanced of them — the reason `snap_cutoff` could not stay a
/// single floor by the widest width.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_tiers_on_different_grids_each_fold_complete_buckets() {
    let store = Store::memory().await.unwrap();
    let series = "align.mixed";
    seed(&store, series, 720, MIN).await; // 12 h at 1-minute cadence

    let fine = Align {
        origin_ms: 7 * MIN as i64,
    }; // deliberately awkward
    let coarse = Align {
        origin_ms: 6 * HOUR as i64 + 30 * MIN as i64,
    };
    set_policy(
        &store,
        WS,
        &Policy {
            prefix: "align.".into(),
            raw_for_ms: 5 * HOUR,
            tiers: vec![
                Tier {
                    width_ms: 10 * MIN,
                    keep_for_ms: 7 * DAY,
                    align: Some(fine),
                    ..Default::default()
                },
                Tier {
                    width_ms: 90 * MIN,
                    keep_for_ms: 90 * DAY,
                    align: Some(coarse),
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let now_ms = DAY0 + 12 * HOUR;
    run_gc(&store, WS, now_ms).await.unwrap();

    let stored = read_rollups(&store, WS, series, 0, DAY0 + 90 * DAY)
        .await
        .unwrap();
    let (fine_rows, coarse_rows): (Vec<_>, Vec<_>) =
        stored.iter().partition(|r| r.width_ms == 10 * MIN);
    assert!(
        !fine_rows.is_empty() && !coarse_rows.is_empty(),
        "both tiers must have folded"
    );
    for r in &fine_rows {
        assert_eq!(r.t, bucket_start(r.t, 10 * MIN, Some(fine)));
    }
    for r in &coarse_rows {
        assert_eq!(r.t, bucket_start(r.t, 90 * MIN, Some(coarse)));
    }

    // Raw is evicted no further than the LEAST-advanced tier folded. The horizon is 07:00Z; the
    // 90-minute grid's last boundary at or before it is 06:30Z, the 10-minute grid's is 06:57Z. So
    // the surviving raw must reach back to 06:30Z — if raw had been evicted to the finer tier's
    // boundary, the coarse tier's 06:30–08:00 bucket could never be completed.
    let surviving = read_buckets(
        &store,
        WS,
        series,
        &BucketQuery {
            from_ts: DAY0,
            to_ts: now_ms,
            width_ms: Some(MIN),
            ..Default::default()
        },
        MIN,
    )
    .await
    .unwrap();
    let oldest_raw_bucket = surviving
        .iter()
        .filter(|b| b.count > 0)
        .map(|b| b.t)
        .min()
        .unwrap();
    assert!(
        oldest_raw_bucket <= bucket_start(DAY0 + 7 * HOUR, 90 * MIN, Some(coarse)),
        "raw was evicted past the coarse tier's boundary — its next bucket can never be complete"
    );

    // A second pass over the same data is idempotent: the deterministic row ids re-upsert identical
    // rows rather than double-counting.
    let before: Vec<(u64, u64, u64)> = stored.iter().map(|r| (r.width_ms, r.t, r.count)).collect();
    run_gc(&store, WS, now_ms).await.unwrap();
    let after: Vec<(u64, u64, u64)> = read_rollups(&store, WS, series, 0, DAY0 + 90 * DAY)
        .await
        .unwrap()
        .iter()
        .map(|r| (r.width_ms, r.t, r.count))
        .collect();
    assert_eq!(before, after, "a repeated pass changed the folded rows");
}
