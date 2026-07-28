//! **The width AXIS, not one width.** Everything the series plane resolves from configuration by an
//! EXACT match on a bucket width is asserted here across a swept axis, because that is the only shape
//! of test that can see the bug class:
//!
//! > a behaviour keyed on an exact match works at the tested value and silently degrades everywhere
//! > else — often falling back to something plausible and wrong
//! > (`docs/scope/testing/testing-scope.md` §3.2).
//!
//! It shipped once already: `Policy::method_for` resolved a tier's `method` only at that tier's own
//! `width_ms`, so a coil configured `last` read as a step chart at exactly 15 min and silently
//! averaged the moment a dashboard zoomed (`series-normalize-session.md`). The exact-match lookups
//! still on the axis, each covered below:
//!
//!   - `Policy::tier_at` / `Policy::method_for` — `.find(|t| t.width_ms == width_ms)`
//!     (`retention.rs`), including the MULTI-tier policies a single-tier test can't express.
//!   - `merge_rollups`' `.filter(|r| r.width_ms == finest)` (`bucket.rs`) — the stored tier the read
//!     merges, exact-matched against the finest width present.
//!   - `apply_method` over rollup-backed buckets — the `value` column at every zoom, not just at the
//!     tier's own width.
//!
//! Real embedded store, real committed samples, real GC pass — no mocks (testing §0).

use lb_ingest::{
    apply_method, commit_batch, read_buckets, run_gc, set_policy, write, BucketQuery, Method,
    Policy, Qos, Sample, Tier,
};
use lb_store::Store;
use serde_json::json;

/// The zoom axis: finer than the stored tier, at it, and coarser — including widths that are NOT
/// tier widths at all (a dashboard's derived width rarely is).
const WIDTHS: &[u64] = &[
    1_000, 5_000, 10_000, 20_000, 25_000, 60_000, 100_000, 300_000, 900_000,
];

const SAMPLES: u64 = 600;
const WINDOW_MS: u64 = 600_000;
/// Raw older than this is evicted and lives only as rollup rows.
const RAW_FOR_MS: u64 = 100_000;
const TIER_MS: u64 = 10_000;

fn sample(series: &str, seq: u64, ts: u64, payload: serde_json::Value) -> Sample {
    Sample {
        series: series.into(),
        producer: "p".into(),
        ts,
        seq,
        payload,
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

/// Seed `SAMPLES` at 1 s cadence (value = index) and GC so the OLDER five sixths of the history is
/// rollup-backed while the tail is still live raw — the state a real node is in, and the only one in
/// which the rollup merge is on the read path at all.
async fn seeded_store(method: Option<Method>) -> Store {
    let store = Store::memory().await.unwrap();
    let samples: Vec<Sample> = (0..SAMPLES)
        .map(|i| sample("ax.v", i + 1, i * 1_000, json!(i as f64)))
        .collect();
    write(&store, "acme", &samples, 0).await.unwrap();
    while commit_batch(&store, "acme", 256).await.unwrap().drained() > 0 {}

    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "ax.".into(),
            raw_for_ms: RAW_FOR_MS,
            max_samples: 0,
            tiers: vec![Tier {
                width_ms: TIER_MS,
                keep_for_ms: 0,
                method,
                ..Default::default()
            }],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let pass = run_gc(&store, "acme", WINDOW_MS).await.unwrap();
    assert_eq!(pass.evicted_raw, 500, "history is now rollup-backed");
    assert_eq!(pass.rollup_rows, 50);
    store
}

fn q(width: u64) -> BucketQuery {
    BucketQuery {
        from_ts: 0,
        to_ts: WINDOW_MS,
        width_ms: Some(width),
        budget: None,
        ..Default::default()
    }
}

/// A bucketed read must account for EVERY sample at EVERY width — the rollup merge exact-matches the
/// finest stored tier, and a width that is not the tier's own must still see that tier's rows.
///
/// The failure this would catch: a read at any width other than the stored 10 s tier quietly
/// returning only the surviving raw tail (100 of 600 samples) while looking perfectly well-formed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bucketed_read_covers_the_whole_history_at_every_width() {
    let store = seeded_store(None).await;
    for &w in WIDTHS {
        let buckets = read_buckets(&store, "acme", "ax.v", &q(w), w)
            .await
            .unwrap();
        let total: u64 = buckets.iter().map(|b| b.count).sum();
        assert_eq!(
            total, SAMPLES,
            "width {w} lost history: {total} of {SAMPLES} samples — the rollup merge is \
             width-keyed and this width is not the tier's own"
        );
        let lo = buckets
            .iter()
            .filter_map(|b| b.min)
            .fold(f64::MAX, f64::min);
        let hi = buckets
            .iter()
            .filter_map(|b| b.max)
            .fold(f64::MIN, f64::max);
        assert_eq!((lo, hi), (0.0, (SAMPLES - 1) as f64), "width {w} range");
        assert!(
            buckets.iter().all(|b| b.t % w == 0),
            "width {w}: buckets must stay on the absolute width grid"
        );
        assert!(
            buckets.len() as u64 <= WINDOW_MS.div_ceil(w),
            "width {w}: more buckets than the window holds"
        );
    }
}

/// THE REGRESSION SHAPE. A tier's `method` describes how the SERIES reads, so it must govern at
/// every width — including widths finer than the tier, which is exactly what a zoom produces.
///
/// Asserted for every method in the closed set, at every width on the axis: the `value` column is
/// present and equals the column that method names.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_method_governs_a_read_at_every_width_not_just_the_tiers_own() {
    for method in [
        Method::Avg,
        Method::Min,
        Method::Max,
        Method::Sum,
        Method::Count,
        Method::Last,
        Method::First,
        Method::Nearest,
    ] {
        let store = seeded_store(Some(method)).await;
        let policy = lb_ingest::list_policies(&store, "acme").await.unwrap();
        for &w in WIDTHS {
            let resolved = lb_ingest::resolve_policy(&policy, "ax.v")
                .and_then(|p| p.method_for(w))
                .unwrap_or_else(|| {
                    panic!("width {w} resolved NO method for a tier configured {method:?}")
                });
            assert_eq!(resolved, method, "width {w} resolved the wrong method");

            let mut buckets = read_buckets(&store, "acme", "ax.v", &q(w), w)
                .await
                .unwrap();
            apply_method(&mut buckets, resolved).unwrap_or_else(|e| {
                panic!(
                    "{method:?} failed at width {w}: {e} — the tier stored no representative \
                        for it at this zoom, so the method is width-keyed after all"
                )
            });
            assert!(
                buckets.iter().all(|b| b.value.is_some()),
                "{method:?} left a bucket without a value at width {w}"
            );
        }
        // The first bucket always starts at t=0, so `first`/`nearest` name sample 0 at every width.
        if matches!(method, Method::First | Method::Nearest) {
            for &w in WIDTHS {
                let mut buckets = read_buckets(&store, "acme", "ax.v", &q(w), w)
                    .await
                    .unwrap();
                apply_method(&mut buckets, method).unwrap();
                assert_eq!(
                    buckets[0].value,
                    Some(json!(0.0)),
                    "{method:?} at width {w} must name the earliest sample"
                );
            }
        }
    }
}

/// A MULTI-tier policy, which a single-tier test cannot express: the method must resolve at every
/// width even when the tier declaring it is neither the finest nor the one the read asked for, and
/// even when a tier at the read's exact width declares NO method of its own (the `and_then` fall
/// through — an exact hit with an empty `method` must not shadow the configured one).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_multi_tier_policy_resolves_its_method_at_every_width() {
    let coarse_only = Policy {
        prefix: "z.".into(),
        raw_for_ms: 0,
        max_samples: 0,
        tiers: vec![
            Tier {
                width_ms: 10_000,
                keep_for_ms: 0,
                method: None, // an EXACT hit that declares nothing
                ..Default::default()
            },
            Tier {
                width_ms: 60_000,
                keep_for_ms: 0,
                method: None,
                ..Default::default()
            },
            Tier {
                width_ms: 900_000,
                keep_for_ms: 0,
                method: Some(Method::Last), // only the coarsest tier says how it reads
                ..Default::default()
            },
        ],
        filter: None,
        ..Default::default()
    };
    for &w in WIDTHS {
        assert_eq!(
            coarse_only.method_for(w),
            Some(Method::Last),
            "width {w}: a tier at this exact width with no method must fall through to the \
             configured one, not resolve None"
        );
    }

    // And when two tiers DISAGREE, the rule is deterministic at every point on the axis: a tier at
    // the read's exact width wins there; everywhere else the FINEST declaring tier governs. The
    // point of sweeping is that "everywhere else" is asserted, not assumed — and that no width
    // resolves `None`.
    let mut split = coarse_only.clone();
    split.tiers[1].method = Some(Method::Avg); // 60 s says avg, 900 s still says last
    for &w in WIDTHS {
        let want = match w {
            60_000 => Method::Avg,   // exact tier, declares avg
            900_000 => Method::Last, // exact tier, declares last
            _ => Method::Avg,        // finest DECLARING tier is the 60 s one
        };
        assert_eq!(
            split.method_for(w),
            Some(want),
            "width {w}: a disagreeing multi-tier policy must still resolve one method, \
             deterministically — never None and never the coarser tier off its own point"
        );
    }
}
