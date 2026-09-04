//! **An inert GC pass must be distinguishable from an idle one** (rubix-ai#84 AC 7), against the
//! real store.
//!
//! The failure this pins, observed live on RC-6 2026-08-04: the unit has no RTC battery and its
//! bench LAN has no NTP route, so its clock drifted 46 minutes BEHIND. Every retention horizon is
//! `now_ms - keep_for`, so every bound landed before the oldest row on disc and
//! `series.retention.gc` evicted nothing while returning `evicted_raw: 0, warnings: 0` — byte-for-
//! byte what a healthy node with nothing to evict returns. After the clock was corrected the same
//! call evicted 702 raw samples.
//!
//! The unit tests in `clock_sanity` pin the RULE. This file pins the thing that actually matters:
//! that running the REAL pass over the REAL store on a skewed clock produces an observable
//! difference, and that a healthy pass does not. A rule that is right but unreachable from `run_gc`
//! would leave the box exactly as blind as it was.

use lb_ingest::{
    commit_direct, last_pass, run_gc, set_policy, Policy, Qos, Sample, Tier, SKEW_TOLERANCE_MS,
};
use lb_store::Store;
use serde_json::json;

const MIN: u64 = 60_000;

fn sample(series: &str, seq: u64, ts: u64) -> Sample {
    Sample {
        series: series.into(),
        producer: "meter-1".into(),
        ts,
        seq,
        payload: json!({ "v": seq }),
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

async fn seed(store: &Store, ws: &str, samples: Vec<Sample>) {
    commit_direct(store, ws, &samples).await.unwrap();
}

/// The shipped modbus 0.1.13 shape, which is what #84 soaks: 30-minute raw window, 15-minute
/// buckets, and a BOUNDED rollup horizon (`keep_for_ms > 0` — under the shipped `0` the rollup tier
/// can never evict, so a zero there proves nothing, per AC 4).
fn modbus_policy() -> Policy {
    Policy {
        prefix: "modbus.".into(),
        raw_for_ms: 30 * MIN,
        tiers: vec![Tier {
            width_ms: 15 * MIN,
            keep_for_ms: 7 * 24 * 60 * MIN,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Seed one hour of 1/min samples ending at `end_ms`, under the modbus policy.
async fn seed_an_hour(store: &Store, ws: &str, end_ms: u64) {
    set_policy(store, ws, &modbus_policy()).await.unwrap();
    let n = 60u64;
    let samples: Vec<Sample> = (0..n)
        .map(|i| sample("modbus.meter-1.kwh", i, end_ms - (n - i) * MIN))
        .collect();
    seed(store, ws, samples).await;
}

/// THE test. Same store, same data, same policy — two passes that differ ONLY in the clock they are
/// handed. Before `clock_sanity` these two were indistinguishable; that is the whole bug.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_skewed_clock_is_distinguishable_from_an_idle_pass() {
    let store = Store::memory().await.unwrap();
    let now = 10_000 * MIN;
    seed_an_hour(&store, "nube", now).await;

    // The HEALTHY pass: the clock agrees with the data, so the 30-minute horizon is real and the
    // older half of the hour is evicted.
    let healthy = run_gc(&store, "nube", now).await.unwrap();
    assert!(
        healthy.evicted_raw > 0,
        "the control must actually evict, or the comparison below is vacuous"
    );
    assert_eq!(
        healthy.clock_skew_ms, None,
        "a clock ahead of its data is not skewed"
    );
    assert!(
        !healthy.warnings.iter().any(|w| w.contains("clock skew")),
        "healthy pass must not cry wolf: {:?}",
        healthy.warnings
    );

    // The SKEWED pass: the RC-6 shape, 46 minutes behind. Fresh store, identical everything else.
    let store = Store::memory().await.unwrap();
    seed_an_hour(&store, "nube", now).await;
    let skewed = run_gc(&store, "nube", now - 46 * MIN).await.unwrap();

    // The old, indistinguishable half — this is what the box used to show and still shows.
    assert_eq!(
        skewed.evicted_raw, 0,
        "a clock behind the data evicts nothing — this is the inert state itself"
    );

    // ...and the new half that tells them apart.
    // The clock is 46m behind, and the NEWEST sample is the last of the seeded hour at `now - 1min`
    // — so the observable skew is 45m. The check measures the gap to the data, which is the only
    // thing the store can actually witness; it does not know what the "true" time was.
    let skew = skewed
        .clock_skew_ms
        .expect("an inert pass MUST report why it was inert");
    assert_eq!(skew, 45 * MIN, "the reported skew must be the real one");
    let warning = skewed
        .warnings
        .iter()
        .find(|w| w.contains("clock skew"))
        .unwrap_or_else(|| panic!("expected a clock-skew warning, got {:?}", skewed.warnings));
    assert!(
        warning.contains("45m"),
        "an operator must be able to read the delta: {warning}"
    );
}

/// The skew must survive the call that found it. On the field hardware the clock resets on every
/// power cycle, so by the time anyone looks the pass is long gone — if this only ever existed as a
/// return value, an unattended node would drift, evict nothing, and leave no trace of why (AC 8's
/// power-cycle half depends on this being readable after the fact).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_skew_is_persisted_for_someone_who_was_not_watching() {
    let store = Store::memory().await.unwrap();
    let now = 10_000 * MIN;
    seed_an_hour(&store, "nube", now).await;

    run_gc(&store, "nube", now - 46 * MIN).await.unwrap();

    let rec = last_pass(&store, "nube")
        .await
        .unwrap()
        .expect("every pass records, including one that evicted nothing");
    assert_eq!(rec.evicted_raw, 0);
    assert!(
        rec.clock_skew_ms.is_some_and(|s| s > SKEW_TOLERANCE_MS),
        "the stored row must carry the skew, not just the return value: {rec:?}"
    );
    assert!(
        rec.warnings.iter().any(|w| w.contains("clock skew")),
        "and the human-readable half too: {:?}",
        rec.warnings
    );

    // A CORRECTED clock must clear it — a warning that latches forever is a warning that gets
    // ignored, and the recovery path is exactly what an operator checks after fixing the clock.
    run_gc(&store, "nube", now).await.unwrap();
    let rec = last_pass(&store, "nube").await.unwrap().unwrap();
    assert_eq!(
        rec.clock_skew_ms, None,
        "correcting the clock must clear the skew on the next pass"
    );
    assert!(rec.evicted_raw > 0, "...and the eviction must then happen");
}

/// Ordinary producer skew — a gateway stamping from its own slightly-fast clock — must stay silent
/// through the REAL pass, not just through the pure rule. A warning that fires on normal field
/// jitter is noise, gets filtered out, and puts us back where we started.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ordinary_producer_skew_does_not_fire() {
    let store = Store::memory().await.unwrap();
    let now = 10_000 * MIN;
    seed_an_hour(&store, "nube", now).await;

    // The newest sample sits 1 minute in the future — well inside the 5-minute tolerance.
    seed(
        &store,
        "nube",
        vec![sample("modbus.meter-1.kwh", 9_999, now + MIN)],
    )
    .await;

    let pass = run_gc(&store, "nube", now).await.unwrap();
    assert_eq!(
        pass.clock_skew_ms, None,
        "1 minute of producer jitter is not a clock failure"
    );
}

/// **A fast clock over-evicts, and only the FLOOR check can see it** — the data comparison cannot,
/// and must not pretend to.
///
/// This test pins the corrected design after a real mistake. A first attempt flagged any clock
/// running ahead of the newest sample as a fault, reasoning that over-eviction is the unrecoverable
/// direction. It is — but "clock ahead of the data" is also the ordinary state of every idle series,
/// so that check fired on healthy nodes (`series_default_cap_test` caught it at once). The two facts
/// are genuinely indistinguishable from the data alone.
///
/// So the coverage is split, and this proves both halves:
///   1. a fast clock really does destroy data that was inside its window (the damage is real);
///   2. `clock_skew_ms` stays silent about it (the data check cannot see it, by design);
///   3. the floor check catches it anyway, because a node cannot run a pass before an earlier one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_fast_clock_over_evicts_and_the_floor_is_what_catches_it() {
    let store = Store::memory().await.unwrap();
    let now = 10_000 * MIN;
    seed_an_hour(&store, "nube", now).await;

    // A control pass on a good clock: the 30-minute horizon keeps the recent half.
    let store_ok = Store::memory().await.unwrap();
    seed_an_hour(&store_ok, "nube", now).await;
    let healthy = run_gc(&store_ok, "nube", now).await.unwrap();

    // (1) The same data, clock a day fast: every sample is now "older than 30 minutes".
    let ahead = run_gc(&store, "nube", now + 24 * 60 * MIN).await.unwrap();
    assert!(
        ahead.evicted_raw > healthy.evicted_raw,
        "a fast clock must over-evict relative to a good one: {} vs {}",
        ahead.evicted_raw,
        healthy.evicted_raw
    );

    // (2) ...and the data comparison is silent, because from the data alone this is
    // indistinguishable from a series that simply stopped reporting a day ago.
    assert_eq!(
        ahead.clock_skew_ms, None,
        "a clock ahead of the data must NOT be reported as skew — that fires on every idle series"
    );

    // (3) The floor is what sees it. The fast pass above wrote its own inflated `last_run_ms`, so
    // when the clock is corrected the next pass is BELOW that floor and says so.
    let corrected = run_gc(&store, "nube", now).await.unwrap();
    let warning = corrected
        .warnings
        .iter()
        .find(|w| w.contains("clock went backwards"))
        .unwrap_or_else(|| {
            panic!(
                "the floor must catch the correction after a fast pass, got {:?}",
                corrected.warnings
            )
        });
    assert!(
        warning.contains("cannot move backwards"),
        "the message must say why this is impossible, not merely odd: {warning}"
    );
}

/// **The power-cycle case (AC 8), which the data comparison is structurally blind to.**
///
/// `skew` compares `now_ms` to the newest sample. That is useless when there are no samples, and
/// equally useless when the producer and the node share one wrong clock (the modbus sidecar stamps
/// `ts` from `SystemTime::now()` on the same box) — the data agrees with the clock and nothing
/// fires.
///
/// `clock_went_backwards` covers it with an INDEPENDENT signal: this node's own last recorded pass
/// is a monotonic floor. A pass demonstrably ran at `last_run_ms`, so the true time is at least
/// that; a clock below it moved backwards between two boots, which a correct clock cannot do. The
/// floor is written on every pass INCLUDING idle ones, so it exists even on a node that has never
/// evicted anything.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_backwards_clock_is_caught_even_with_no_data_at_all() {
    let store = Store::memory().await.unwrap();
    let now = 10_000 * MIN;

    // A pass on a good clock over an EMPTY workspace. Evicts nothing, warns about nothing — and
    // still records the floor, which is the whole point of recording idle passes.
    let first = run_gc(&store, "nube", now).await.unwrap();
    assert!(first.warnings.is_empty(), "{:?}", first.warnings);
    assert_eq!(
        last_pass(&store, "nube")
            .await
            .unwrap()
            .unwrap()
            .last_run_ms,
        now,
        "an idle pass must still stamp the floor, or there is nothing to check against"
    );

    // Power cycle: the box comes back with its clock hours behind. The series table is STILL empty,
    // so `skew` has nothing to say...
    let after_reboot = now - 6 * 60 * MIN;
    let pass = run_gc(&store, "nube", after_reboot).await.unwrap();
    assert_eq!(
        pass.clock_skew_ms, None,
        "no data means the data-comparison check is silent — that is the blind spot"
    );

    // ...and the floor catches it anyway.
    let warning = pass
        .warnings
        .iter()
        .find(|w| w.contains("clock went backwards"))
        .unwrap_or_else(|| {
            panic!(
                "expected a backwards-clock warning, got {:?}",
                pass.warnings
            )
        });
    assert!(
        warning.contains("6h"),
        "the delta must be readable: {warning}"
    );
    assert!(
        warning.contains("cannot move backwards"),
        "the message must say why this is impossible rather than merely odd: {warning}"
    );
}

/// A node that has genuinely never run a pass has no floor, and must not invent one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_node_that_never_ran_a_pass_has_no_floor_to_check() {
    let store = Store::memory().await.unwrap();
    // now_ms = 0 is the most hostile clock there is; with no prior pass there is nothing to compare.
    let pass = run_gc(&store, "nube", 0).await.unwrap();
    assert!(pass.warnings.is_empty(), "{:?}", pass.warnings);
}

/// An empty workspace carries no lower bound on the true time from its DATA, so the skew check must
/// say nothing — otherwise every freshly-provisioned node alarms on its first pass, before it has
/// any data to be wrong about. (The floor check above is what covers this case instead.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_workspace_never_alarms() {
    let store = Store::memory().await.unwrap();
    set_policy(&store, "nube", &modbus_policy()).await.unwrap();

    // now_ms = 0 is the most hostile clock there is, and an empty store still cannot contradict it.
    let pass = run_gc(&store, "nube", 0).await.unwrap();
    assert_eq!(pass.clock_skew_ms, None);
    assert!(pass.warnings.is_empty(), "{:?}", pass.warnings);
}
