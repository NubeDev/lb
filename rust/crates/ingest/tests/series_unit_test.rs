//! Source-unit provenance, proven against the REAL store (no mocks): a producer's `unit` label
//! becomes the series' registered unit, an unparseable one is refused rather than recorded, and a
//! series that declares nothing keeps working exactly as before.
//!
//! This is the provenance the conversion engine was missing. `lb_prefs` could always convert
//! correctly; it had no `from_unit` to convert FROM, because the registry carried two columns and
//! `series.list` returned bare strings. These tests pin the column that closes that gap.

use lb_ingest::{commit_batch, series_units, set_unit, unit, write, Qos, Sample};
use lb_prefs::axis::Unit;
use lb_store::Store;
use serde_json::json;

fn sample(series: &str, seq: u64, labels: serde_json::Value) -> Sample {
    Sample {
        series: series.into(),
        producer: "prod-a".into(),
        ts: seq * 1000,
        seq,
        payload: json!(seq),
        labels,
        qos: Qos::BestEffort,
    }
}

async fn seed(store: &Store, ws: &str, samples: Vec<Sample>) {
    write(store, ws, &samples, 0).await.unwrap();
    while commit_batch(store, ws, 256).await.unwrap().drained() != 0 {}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declared_unit_becomes_the_series_provenance() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        vec![sample(
            "meter.main.energy",
            1,
            json!({"unit": "kilowatt_hour"}),
        )],
    )
    .await;

    assert_eq!(
        unit(&store, "nube", "meter.main.energy").await.unwrap(),
        Some(Unit::KilowattHour),
        "the producer's declared unit is the series' registered unit"
    );
}

/// The ordinary case for every series that predates this column: no declaration, no unit, and the
/// series still ingests and reads exactly as it always did. An absent unit means "unknown" — the
/// caller renders the canonical value unconverted, which is today's behaviour.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_series_that_declares_nothing_has_no_unit() {
    let store = Store::memory().await.unwrap();
    seed(&store, "nube", vec![sample("legacy.cpu", 1, json!({}))]).await;

    assert_eq!(unit(&store, "nube", "legacy.cpu").await.unwrap(), None);
}

/// **The refusal that matters.** Free text like `"degrees celsius"` LOOKS like provenance and
/// converts to nothing. Recording it would make the registry claim a unit the converter cannot
/// honour — strictly worse than an absent unit, which at least tells the truth.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unparseable_unit_is_refused_not_recorded() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        vec![
            sample("bad.text", 1, json!({"unit": "degrees celsius"})),
            sample("bad.type", 1, json!({"unit": 42})),
            sample("bad.empty", 1, json!({"unit": ""})),
        ],
    )
    .await;

    for series in ["bad.text", "bad.type", "bad.empty"] {
        assert_eq!(
            unit(&store, "nube", series).await.unwrap(),
            None,
            "{series} declared a unit outside the closed vocabulary; it must not be recorded"
        );
    }
}

/// A unit label must NOT be latched the way label→tag conversion is: a rescaled sensor or a
/// corrected declaration is a real event, and a first-write-wins latch would make a typo permanent.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_later_declaration_replaces_an_earlier_one() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        vec![sample("sensor.tank", 1, json!({"unit": "liter"}))],
    )
    .await;
    assert_eq!(
        unit(&store, "nube", "sensor.tank").await.unwrap(),
        Some(Unit::Liter)
    );

    // The producer corrects itself: the tank is actually metered in kilolitres.
    seed(
        &store,
        "nube",
        vec![sample("sensor.tank", 2, json!({"unit": "kiloliter"}))],
    )
    .await;
    assert_eq!(
        unit(&store, "nube", "sensor.tank").await.unwrap(),
        Some(Unit::Kiloliter),
        "the newest declaration wins; a unit is not latched"
    );
}

/// The unit rides alongside the labels a producer already declares — it must not disturb them, and
/// the series must still commit normally.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_unit_coexists_with_ordinary_labels() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        vec![sample(
            "ahu.1.supply_temp",
            1,
            json!({"unit": "celsius", "host": "pi-7", "site": "moree"}),
        )],
    )
    .await;

    assert_eq!(
        unit(&store, "nube", "ahu.1.supply_temp").await.unwrap(),
        Some(Unit::Celsius)
    );
}

/// `series_units` is the listing shape a metadata-bearing `series.list` serves: every registered
/// series, with its unit where it has one — in ONE query, not one probe per series.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_listing_carries_units_and_absences_together() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        vec![
            sample("bas.power", 1, json!({"unit": "kilowatt"})),
            sample("bas.volts", 1, json!({"unit": "volt"})),
            sample("bas.unknown", 1, json!({})),
            sample("other.thing", 1, json!({"unit": "lux"})),
        ],
    )
    .await;

    let listed = series_units(&store, "nube", "bas.").await.unwrap();
    assert_eq!(
        listed,
        vec![
            ("bas.power".to_string(), Some(Unit::Kilowatt)),
            ("bas.unknown".to_string(), None),
            ("bas.volts".to_string(), Some(Unit::Volt)),
        ],
        "prefix-filtered, ascending, units and absences together"
    );
}

/// The direct setter is the seam a non-ingest declaration (an operator, an importer) uses. It
/// registers the series if it is not yet known, so declare-then-write works in either order.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_unit_registers_an_unknown_series() {
    let store = Store::memory().await.unwrap();
    set_unit(&store, "nube", "declared.first", Unit::PartsPerMillion)
        .await
        .unwrap();

    assert_eq!(
        unit(&store, "nube", "declared.first").await.unwrap(),
        Some(Unit::PartsPerMillion)
    );

    // And a later sample on that series commits normally, keeping the declared unit.
    seed(&store, "nube", vec![sample("declared.first", 1, json!({}))]).await;
    assert_eq!(
        unit(&store, "nube", "declared.first").await.unwrap(),
        Some(Unit::PartsPerMillion),
        "an undeclared sample must not erase an existing unit"
    );
}
