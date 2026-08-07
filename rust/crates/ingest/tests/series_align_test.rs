//! Rollup bucket ALIGNMENT — the policy half (series-observability scope, Decision 21; issue #111).
//!
//! A tier can declare where its buckets START. This file holds the shape questions: does an aligned
//! tier survive the store round-trip, does a policy WITHOUT one still mean exactly what it always
//! meant, and which tier's anchor governs a read at some other width. The grid arithmetic itself is
//! unit-tested in `align.rs`; the fold-vs-read agreement — the failure this feature had to be
//! designed around — is `series_align_grid_test.rs`.

use lb_ingest::{
    bucket_start, list_policies, resolve_policy, set_policy, Align, Policy, Tier, RETENTION_TABLE,
};
use lb_store::Store;

const MIN: u64 = 60_000;
const HOUR: u64 = 3_600_000;
const DAY: u64 = 86_400_000;
/// 2026-07-27T00:00:00Z — a real instant, so a "local midnight" assertion means something.
const DAY0: u64 = 1_785_110_400_000;

fn tier(width_ms: u64, align: Option<Align>) -> Tier {
    Tier {
        width_ms,
        keep_for_ms: 7 * DAY,
        align,
        ..Default::default()
    }
}

/// The revert-check `retention.rs` names: `list_policies` projects `tiers` as a whole column, so a
/// field added INSIDE a tier rides back with it. Assumed once; asserted here, because the closed
/// projection above it is exactly the trap that has bitten this file before.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_aligned_tier_round_trips() {
    let store = Store::memory().await.unwrap();
    let written = Policy {
        prefix: "align.".into(),
        raw_for_ms: HOUR,
        tiers: vec![
            tier(15 * MIN, None),
            tier(
                DAY,
                Some(Align {
                    origin_ms: -10 * HOUR as i64,
                }),
            ),
        ],
        ..Default::default()
    };
    set_policy(&store, "nube", &written).await.unwrap();

    let read = list_policies(&store, "nube").await.unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(
        read[0].tiers, written.tiers,
        "the tiers must survive verbatim"
    );
    assert_eq!(
        read[0].tiers[0].align, None,
        "an unaligned tier stays unaligned"
    );
    assert_eq!(
        read[0].tiers[1].align,
        Some(Align {
            origin_ms: -36_000_000
        })
    );
}

/// A policy row written BEFORE this field existed must keep its exact meaning. Written as raw JSON
/// with no `align` key at all — the only faithful way to represent "an older node wrote this".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_policy_written_before_alignment_reads_back_unaligned() {
    let store = Store::memory().await.unwrap();
    store
        .query_ws(
            "nube",
            &format!(
                "UPSERT type::thing('{RETENTION_TABLE}', 'legacy.') CONTENT {{ \
                   prefix: 'legacy.', raw_for_ms: 3600000, max_samples: 0, \
                   tiers: [{{ width_ms: 900000, keep_for_ms: 604800000, method: 'avg' }}] }}"
            ),
            vec![],
        )
        .await
        .unwrap();

    let read = list_policies(&store, "nube").await.unwrap();
    assert_eq!(
        read[0].tiers[0].align, None,
        "absent must not become a zero anchor"
    );
    assert_eq!(read[0].tiers[0].method.map(|m| m.as_str()), Some("avg"));
    // ...and "unaligned" is the epoch grid, byte-for-byte the floor that shipped.
    let w = read[0].tiers[0].width_ms;
    assert_eq!(
        bucket_start(DAY0 + 1234, w, read[0].tiers[0].align),
        (DAY0 + 1234) / w * w
    );
}

/// Absent alignment is not "an anchor of 0" wearing a different hat — they describe the same grid,
/// and they must still be distinguishable on the wire so an untouched policy round-trips untouched.
#[test]
fn absent_and_zero_are_the_same_grid_but_not_the_same_value() {
    let w = 15 * MIN;
    assert_eq!(
        bucket_start(DAY0 + 999, w, None),
        bucket_start(DAY0 + 999, w, Some(Align { origin_ms: 0 })),
    );
    assert_ne!(tier(w, None), tier(w, Some(Align { origin_ms: 0 })));
    // And that difference survives serialization, which is what a UI round-trip actually rides on.
    let unaligned = serde_json::to_value(tier(w, None)).unwrap();
    assert!(
        unaligned.get("align").is_none(),
        "an unaligned tier writes no align key"
    );
    let zeroed = serde_json::to_value(tier(w, Some(Align { origin_ms: 0 }))).unwrap();
    assert_eq!(zeroed["align"]["origin_ms"], 0);
}

/// `align_for` resolves like `method_for` — exact tier first, else the finest tier that declares
/// one. Deliberately the same rule: an anchor describes what the SERIES means, not one stored width.
#[test]
fn a_read_inherits_the_finest_declared_anchor_when_its_own_width_has_none() {
    let six_thirty = Align {
        origin_ms: 6 * HOUR as i64 + 30 * MIN as i64,
    };
    let local_midnight = Align {
        origin_ms: -10 * HOUR as i64,
    };
    let p = Policy {
        prefix: "align.".into(),
        tiers: vec![
            tier(15 * MIN, None),
            tier(90 * MIN, Some(six_thirty)),
            tier(DAY, Some(local_midnight)),
        ],
        ..Default::default()
    };

    // Exact match wins, even when a finer tier also declares one.
    assert_eq!(p.align_for(DAY), Some(local_midnight));
    assert_eq!(p.align_for(90 * MIN), Some(six_thirty));
    // A width with no tier — and a tier with no anchor — both inherit the FINEST declared one.
    assert_eq!(p.align_for(15 * MIN), Some(six_thirty));
    assert_eq!(p.align_for(5 * MIN), Some(six_thirty));

    // A policy that declares no anchor anywhere resolves none: the epoch grid, unchanged.
    let plain = Policy {
        prefix: "plain.".into(),
        tiers: vec![tier(15 * MIN, None)],
        ..Default::default()
    };
    assert_eq!(plain.align_for(15 * MIN), None);
}

/// Inheriting is SAFE as well as consistent: an anchor declared for a coarse tier collapses to the
/// epoch grid at any width that divides it, so a finer read is unaffected — which is why applying
/// the same precedence as `method_for` cannot surprise anyone.
#[test]
fn an_inherited_anchor_is_inert_at_a_width_that_divides_it() {
    let shift_start = Some(Align {
        origin_ms: 6 * HOUR as i64,
    });
    let ts = DAY0 + 7 * HOUR + 1234;
    for w in [MIN, 5 * MIN, 15 * MIN, HOUR] {
        assert_eq!(
            bucket_start(ts, w, shift_start),
            bucket_start(ts, w, None),
            "a 06:00 anchor must not move a {w}ms bucket — 6 h is a whole number of them"
        );
    }
    // At a width it does NOT divide, it is exactly the knob it was declared to be.
    assert_ne!(
        bucket_start(ts, 7 * MIN, shift_start),
        bucket_start(ts, 7 * MIN, None)
    );
}

/// Resolution runs through the SAME longest-prefix rule as the method and the GC, so a narrower
/// policy re-grids its own series without touching its neighbours'.
#[test]
fn the_longest_matching_prefix_owns_the_grid() {
    let policies = vec![
        Policy {
            prefix: "plant.".into(),
            tiers: vec![tier(DAY, Some(Align { origin_ms: 0 }))],
            ..Default::default()
        },
        Policy {
            prefix: "plant.night-shift.".into(),
            tiers: vec![tier(
                DAY,
                Some(Align {
                    origin_ms: 22 * HOUR as i64,
                }),
            )],
            ..Default::default()
        },
    ];
    let day = resolve_policy(&policies, "plant.line-1.kw").unwrap();
    let night = resolve_policy(&policies, "plant.night-shift.line-1.kw").unwrap();
    assert_eq!(day.align_for(DAY), Some(Align { origin_ms: 0 }));
    assert_eq!(
        night.align_for(DAY),
        Some(Align {
            origin_ms: 22 * HOUR as i64
        })
    );
    assert_eq!(
        bucket_start(DAY0 + 23 * HOUR, DAY, night.align_for(DAY)),
        DAY0 + 22 * HOUR
    );
    assert_eq!(
        bucket_start(DAY0 + 23 * HOUR, DAY, day.align_for(DAY)),
        DAY0
    );
}
