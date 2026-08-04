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
    commit_batch, last_pass, run_gc, set_policy, write, Policy, Qos, Sample, Tier,
    SKEW_TOLERANCE_MS,
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
    write(store, ws, &samples, 0).await.unwrap();
    while commit_batch(store, ws, 256).await.unwrap().drained() != 0 {}
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
    seed_an_hour(&store, "acme", now).await;

    // The HEALTHY pass: the clock agrees with the data, so the 30-minute horizon is real and the
    // older half of the hour is evicted.
    let healthy = run_gc(&store, "acme", now).await.unwrap();
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
    seed_an_hour(&store, "acme", now).await;
    let skewed = run_gc(&store, "acme", now - 46 * MIN).await.unwrap();

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
    seed_an_hour(&store, "acme", now).await;

    run_gc(&store, "acme", now - 46 * MIN).await.unwrap();

    let rec = last_pass(&store, "acme")
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
    run_gc(&store, "acme", now).await.unwrap();
    let rec = last_pass(&store, "acme").await.unwrap().unwrap();
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
    seed_an_hour(&store, "acme", now).await;

    // The newest sample sits 1 minute in the future — well inside the 5-minute tolerance.
    seed(
        &store,
        "acme",
        vec![sample("modbus.meter-1.kwh", 9_999, now + MIN)],
    )
    .await;

    let pass = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(
        pass.clock_skew_ms, None,
        "1 minute of producer jitter is not a clock failure"
    );
}

/// An empty workspace carries no lower bound on the true time, so it must say nothing at all —
/// otherwise every freshly-provisioned node alarms on its first pass, before it has any data to be
/// wrong about.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_workspace_never_alarms() {
    let store = Store::memory().await.unwrap();
    set_policy(&store, "acme", &modbus_policy()).await.unwrap();

    // now_ms = 0 is the most hostile clock there is, and an empty store still cannot contradict it.
    let pass = run_gc(&store, "acme", 0).await.unwrap();
    assert_eq!(pass.clock_skew_ms, None);
    assert!(pass.warnings.is_empty(), "{:?}", pass.warnings);
}
